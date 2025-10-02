//! Contains an actor that will check the balance of a random address once per second.
//! Many of these actors can be spawned at once to put load on the server.

use {
    crate::{client::UmiClient, stats::Datapoint},
    rand::{Rng, SeedableRng, rngs::StdRng},
    std::time::Duration,
    tokio::{
        sync::{broadcast::Receiver, mpsc::UnboundedSender},
        task::JoinHandle,
    },
};

pub const BATCH_SEPARATION: Duration = Duration::from_millis(1);
const INTERVAL: Duration = Duration::from_secs(1);

pub struct BalanceChecker {
    rng: StdRng,
    client: UmiClient,
    shutdown: Receiver<()>,
}

impl BalanceChecker {
    pub fn spawn_many(
        n: usize,
        stats_channel: UnboundedSender<Datapoint>,
        shutdown: Receiver<()>,
    ) -> anyhow::Result<Vec<JoinHandle<()>>> {
        let mut result: Vec<JoinHandle<()>> = Vec::with_capacity(n);
        let mut seed = rand::thread_rng();
        for _ in 0..n {
            let actor = match Self::new(&mut seed, stats_channel.clone(), shutdown.resubscribe()) {
                Ok(actor) => actor,
                Err(e) => {
                    // Cancel all previously created actors before returning
                    for handle in result {
                        handle.abort();
                    }
                    return Err(e);
                }
            };
            result.push(actor.spawn());
        }
        Ok(result)
    }

    pub fn new<R: Rng>(
        seed: &mut R,
        stats_channel: UnboundedSender<Datapoint>,
        shutdown: Receiver<()>,
    ) -> anyhow::Result<Self> {
        let rng = StdRng::from_rng(seed)?;
        let client = UmiClient::new(stats_channel, None);
        Ok(Self {
            rng,
            client,
            shutdown,
        })
    }

    pub fn spawn(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                let address: [u8; 20] = self.rng.r#gen();

                // Race balance check against the shutdown channel to ensure
                // shutdown commands are not blocked by waiting on a response from the server.
                let result = tokio::select! {
                    result = self.client.eth_get_balance(address.into()) => result,
                    _ = self.shutdown.recv() => break,
                };

                if let Err(e) = result {
                    println!("WARN: error in balance query: {e:?}");
                }
                tokio::select! {
                    _ = tokio::time::sleep(INTERVAL) => continue,
                    _ = self.shutdown.recv() => break,
                }
            }
        })
    }
}
