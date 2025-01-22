//! Constants and functions used to submit regular transactions to the L1.
//! This heartbeat ensures that the L1 makes regular progress and this is
//! necessary because it is an assumption the proposer makes.

use super::*;

pub const ADDRESS: Address = address!("88f9b82462f6c4bf4a0fb15e5c3971559a316e7f");
const SK: [u8; 32] = [0xbb; 32];
const TARGET: Address = address!("1111111111111111111111222222222222222222");
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(3);

pub fn start_heartbeat() -> tokio::task::JoinHandle<anyhow::Result<()>> {
    tokio::spawn(async {
        let signer = PrivateKeySigner::from_slice(&SK).unwrap();
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(EthereumWallet::from(signer))
            .on_http(Url::parse(&var("L1_RPC_URL")?)?);
        let amount = U256::from(100_u64);
        loop {
            let tx = provider
                .transaction_request()
                .to(TARGET)
                .value(amount)
                .gas_limit(21_000);
            let pending = provider
                .send_transaction(tx)
                .await
                .inspect_err(|e| println!("HEARTBEAT ERROR {e}"))?;
            let _tx_hash = pending
                .watch()
                .await
                .inspect_err(|e| println!("HEARTBEAT ERROR {e}"))?;
            tokio::time::sleep(HEARTBEAT_INTERVAL).await;
        }
    })
}
