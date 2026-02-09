use clap::Parser;

#[derive(Parser, Debug)]
pub struct Args {
    #[arg(short, long)]
    pub private_key: String,
    #[arg(short, long)]
    pub game_factory_address: String,
    #[arg(short, long)]
    pub l1_eth_rpc: String,
    #[arg(short, long, default_value("3600"))]
    pub proposer_interval_secs: u64,
}
