use crate::{client::UmiClient, config::LoadTestConfig};

mod client;
mod compile;
mod config;
mod run_server;

const CARGO_MANIFEST_DIR: &str = std::env!("CARGO_MANIFEST_DIR");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = LoadTestConfig::new()?;
    let server_binary = config.binary_path().await?;

    // Start `op-move` and wait for it to be ready.
    let mut umi_process = run_server::start(&server_binary, config.to_server_config()?)?;
    tokio::time::sleep(config.op_move_start_time).await;

    // Create a client that can access the authorized endpoint.
    let client = UmiClient::new(Some(config.jwt_secret()));

    // Send some example requests as a test

    let genesis = client.get_block_by_number(0).await?;
    assert_eq!(genesis.0.header.number, 0);

    let balance = client
        .eth_get_balance(alloy::primitives::Address::ZERO)
        .await?;
    assert_eq!(balance, alloy::primitives::U256::ZERO);

    // Shutdown `op-move`
    umi_process.kill().await?;

    Ok(())
}
