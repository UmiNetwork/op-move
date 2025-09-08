use crate::config::LoadTestConfig;

mod compile;
mod config;
mod run_server;

const CARGO_MANIFEST_DIR: &str = std::env!("CARGO_MANIFEST_DIR");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let server_binary = compile::build_umi_server().await?;
    let config = LoadTestConfig::new()?;

    let mut umi_process = run_server::start(&server_binary, config.to_server_config()?)?;

    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    umi_process.kill().await?;

    Ok(())
}
