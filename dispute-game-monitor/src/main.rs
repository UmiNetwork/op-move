use {
    clap::Parser,
    umi_dispute_game_monitor::{
        cli::Args, config::Config, initialize, monitor_loop, set_global_tracing_subscriber,
    },
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    set_global_tracing_subscriber();

    let args = Args::parse();
    let config: Config = args.try_into()?;

    let (state, provider) = initialize(&config).await?;
    monitor_loop(config, state, provider).await;

    Ok(())
}
