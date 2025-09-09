use crate::{client::UmiClient, config::LoadTestConfig};

mod client;
mod compile;
mod config;
mod run_server;

const CARGO_MANIFEST_DIR: &str = std::env!("CARGO_MANIFEST_DIR");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server_binary = compile::build_umi_server().await?;
    let config = LoadTestConfig::new()?;

    let mut umi_process = run_server::start(&server_binary, config.to_server_config()?)?;
    let client = UmiClient::new(None);

    // Wait for op-move to start
    tokio::time::sleep(std::time::Duration::from_secs(20)).await;

    // Send an example request as a test
    let balance = client
        .eth_get_balance(alloy::primitives::Address::ZERO)
        .await?;
    assert_eq!(balance, alloy::primitives::U256::ZERO);

    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    umi_process.kill().await?;

    Ok(())
}
