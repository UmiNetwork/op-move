use {
    crate::{client::UmiClient, config::LoadTestConfig},
    tokio::sync::broadcast,
};

mod client;
mod compile;
mod config;
mod loads;
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

    // Get the genesis block hash
    let genesis = client.get_block_by_number(0).await?;
    let genesis_block_hash = genesis.0.header.hash;

    // Create shutdown channel to control graceful end to load test.
    let (shutdown, shutdown_rx) = broadcast::channel(1);

    // Spawn the block building job
    let block_build = loads::block_production::BlockProduction::new(
        genesis_block_hash,
        config.jwt_secret(),
        shutdown_rx,
    )
    .spawn();

    // Spawn balance check jobs
    let balance_checkers = loads::balance_checker::BalanceChecker::spawn_many(
        config.n_balance_checkers,
        shutdown.subscribe(),
    )
    .await?;

    // Allow test to run for a time
    // TODO: collect metrics?
    tokio::time::sleep(config.load_test_duration).await;

    // Shutdown loads
    shutdown.send(()).ok();
    if let Err(e) = block_build.await {
        println!("WARN: failed to join block build task: {e:?}");
    }
    for handle in balance_checkers {
        if let Err(e) = handle.await {
            println!("WARN: failed to join balance checker task: {e:?}");
        }
    }

    // Shutdown `op-move`
    umi_process.kill().await?;

    Ok(())
}
