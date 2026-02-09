use {
    alloy::{
        network::EthereumWallet,
        primitives::Address,
        providers::{Provider, ProviderBuilder},
        signers::local::PrivateKeySigner,
        transports::http::reqwest::Url,
    },
    anyhow::Context,
    std::time::Duration,
};

pub struct Config {
    pub signer: PrivateKeySigner,
    pub factory_address: Address,
    pub rpc_url: Url,
    pub interval: Duration,
}

impl Config {
    pub fn get_provider(&self) -> impl Provider + 'static {
        ProviderBuilder::new()
            .wallet(EthereumWallet::from(self.signer.clone()))
            .connect_http(self.rpc_url.clone())
    }
}

impl TryFrom<crate::cli::Args> for Config {
    type Error = anyhow::Error;

    fn try_from(value: crate::cli::Args) -> Result<Self, Self::Error> {
        let signer_bytes = alloy::hex::decode(
            value
                .private_key
                .strip_prefix("0x")
                .unwrap_or(&value.private_key),
        )
        .context("Invalid hex encoding in private key")?;

        let signer =
            PrivateKeySigner::from_slice(&signer_bytes).context("Invalid private key bytes")?;

        let factory_address: Address = value
            .game_factory_address
            .parse()
            .context("Invalid game factory address")?;

        let rpc_url: Url = value.l1_eth_rpc.parse().context("Invalid L1 Eth RPC URL")?;

        let interval = Duration::from_secs(value.proposer_interval_secs);

        Ok(Self {
            signer,
            factory_address,
            rpc_url,
            interval,
        })
    }
}
