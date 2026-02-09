use {
    alloy::{
        primitives::Address, signers::local::PrivateKeySigner, transports::http::reqwest::Url,
    },
    tokio::process::Command,
};

pub async fn resolve_claim(
    game_address: &Address,
    rpc: &Url,
    signer: &PrivateKeySigner,
) -> anyhow::Result<()> {
    let address_str = game_address.to_string();
    let key_str = alloy::hex::encode(signer.to_bytes());

    Command::new("op-challenger")
        .args([
            "resolve-claim",
            "--l1-eth-rpc",
            rpc.as_str(),
            "--claim",
            "0",
            "--game-address",
            &address_str,
            "--private-key",
            &key_str,
        ])
        .output()
        .await?;

    Ok(())
}

pub async fn resolve_game(
    game_address: &Address,
    rpc: &Url,
    signer: &PrivateKeySigner,
) -> anyhow::Result<()> {
    let address_str = game_address.to_string();
    let key_str = alloy::hex::encode(signer.to_bytes());

    Command::new("op-challenger")
        .args([
            "resolve",
            "--l1-eth-rpc",
            rpc.as_str(),
            "--game-address",
            &address_str,
            "--private-key",
            &key_str,
        ])
        .output()
        .await?;

    Ok(())
}
