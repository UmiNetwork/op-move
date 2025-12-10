use {
    super::*,
    std::{
        sync::{
            atomic::{AtomicBool, AtomicU64, Ordering},
            Arc,
        },
        thread::JoinHandle,
    },
};

const RESOLVE_INTERVAL: Duration = Duration::from_secs(30);

mod op_dgf {
    alloy::sol!(
        #[sol(rpc)]
        DisputeGameFactory,
        "src/tests/res/DisputeGameFactory.json"
    );
}

mod op_pdg {
    alloy::sol!(
        #[sol(rpc)]
        PermissionedDisputeGame,
        "src/tests/res/PermissionedDisputeGame.json"
    );
}

pub struct ChallengerTask {
    should_stop: Arc<AtomicBool>,
    curr_idx: Arc<AtomicU64>,
    inner: JoinHandle<anyhow::Result<()>>,
}

impl ChallengerTask {
    pub fn new() -> Self {
        let should_stop = Arc::new(AtomicBool::new(false));
        let l1_addr = L1Addresses::load().unwrap();
        let games = Command::new("op-challenger")
            .args([
                "list-games",
                "--l1-eth-rpc",
                &var("L1_RPC_URL").unwrap(),
                "--game-factory-address",
                &l1_addr.dispute_game_factory_proxy.to_string(),
            ])
            .output()
            .unwrap();
        let stdout = String::from_utf8(games.stdout).unwrap();
        let last_idx: u64 = stdout
            .lines()
            .rev() // start from the end
            .find(|line| !line.trim().is_empty())
            .and_then(|line| line.split_whitespace().next())
            .ok_or_else(|| panic!("no rows found"))
            .unwrap()
            .parse()
            .unwrap();

        let curr_idx = Arc::new(AtomicU64::new(last_idx));
        let thread_stop = Arc::clone(&should_stop);
        let thread_idx = Arc::clone(&curr_idx);
        let inner = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(async {
                let admin_key = var("ADMIN_PRIVATE_KEY").unwrap();
                let signer = PrivateKeySigner::from_str(&admin_key).unwrap();
                let provider = ProviderBuilder::new()
                    .wallet(EthereumWallet::from(signer))
                    .connect_http(Url::parse(&var("L1_RPC_URL")?)?);

                let game_factory_address = l1_addr.dispute_game_factory_proxy;
                let game_factory = op_dgf::DisputeGameFactory::new(game_factory_address, &provider);
                loop {
                    if thread_stop.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    let curr_idx = thread_idx.load(Ordering::Relaxed);
                    let next_game_addr = game_factory
                        .gameAtIndex(U256::from(curr_idx))
                        .call()
                        .await?
                        .proxy_;
                    eprintln!("Processing game address {next_game_addr}...");
                    for _ in 1..7 {
                        let resolve_claim = Command::new("op-challenger")
                            .args([
                                "resolve-claim",
                                "--l1-eth-rpc",
                                &var("L1_RPC_URL")?,
                                "--game-address",
                                &next_game_addr.to_string(),
                                "--claim",
                                "0",
                                "--private-key",
                                &admin_key,
                            ])
                            .output()?;
                        if resolve_claim.status.code() != Some(0) {
                            eprintln!("Game was not ready yet, retrying...");
                            tokio::time::sleep(Duration::from_secs(10)).await;
                        } else {
                            eprintln!("Resolved a game claim");
                            break;
                        }
                    }

                    let _resolve = Command::new("op-challenger")
                        .args([
                            "resolve",
                            "--l1-eth-rpc",
                            &var("L1_RPC_URL")?,
                            "--game-address",
                            &next_game_addr.to_string(),
                            "--private-key",
                            &admin_key,
                        ])
                        .output()?;

                    thread_idx.store(curr_idx + 1, Ordering::Relaxed);
                    tokio::time::sleep(RESOLVE_INTERVAL).await;
                }
            })
        });

        Self {
            should_stop,
            curr_idx,
            inner,
        }
    }

    pub fn shutdown(self) {
        self.should_stop.store(true, Ordering::Relaxed);
        let join_result = self
            .inner
            .join()
            .expect("Challenger thread should complete");
        if let Err(e) = join_result {
            println!("CHALLENGER ERROR {e:?}");
        }
    }

    pub fn curr_idx(&self) -> u64 {
        self.curr_idx.load(Ordering::Relaxed)
    }
}

impl Default for ChallengerTask {
    fn default() -> Self {
        Self::new()
    }
}
