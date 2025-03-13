//! Constants and functions used to submit regular transactions to the L1.
//! This heartbeat ensures that the L1 makes regular progress and this is
//! necessary because it is an assumption the proposer makes.

use {
    super::*,
    std::{
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        thread::JoinHandle,
    },
};

pub const ADDRESS: Address = address!("88f9b82462f6c4bf4a0fb15e5c3971559a316e7f");
const SK: [u8; 32] = [0xbb; 32];
const TARGET: Address = address!("1111111111111111111111222222222222222222");
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(3);

pub struct HeartbeatTask {
    should_stop: Arc<AtomicBool>,
    inner: JoinHandle<anyhow::Result<()>>,
}

impl HeartbeatTask {
    pub fn new() -> Self {
        let should_stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&should_stop);
        let inner = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(async {
                let signer = PrivateKeySigner::from_slice(&SK).unwrap();
                let provider = ProviderBuilder::new()
                    .with_recommended_fillers()
                    .wallet(EthereumWallet::from(signer))
                    .on_http(Url::parse(&var("L1_RPC_URL")?)?);
                let amount = U256::from(100_u64);
                loop {
                    if thread_stop.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    let tx = provider
                        .transaction_request()
                        .to(TARGET)
                        .value(amount)
                        .gas_limit(21_000);
                    let pending = provider
                        .send_transaction(tx)
                        .await
                        .inspect_err(|e| println!("HEARTBEAT ERROR {e:?}"))?;
                    let _tx_hash = pending
                        .with_required_confirmations(0)
                        .with_timeout(Some(HEARTBEAT_INTERVAL / 2))
                        .watch()
                        .await
                        .inspect_err(|e| println!("HEARTBEAT ERROR {e:?}"))?;
                    tokio::time::sleep(HEARTBEAT_INTERVAL).await;
                }
            })
        });

        Self { should_stop, inner }
    }

    pub fn shutdown(self) {
        self.should_stop.store(true, Ordering::Relaxed);
        let join_result = self.inner.join().expect("Heartbeat thread should complete");
        if let Err(e) = join_result {
            println!("HEARTBEAT ERROR {e:?}");
        }
    }
}

impl Default for HeartbeatTask {
    fn default() -> Self {
        Self::new()
    }
}
