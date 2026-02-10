use {
    alloy::{
        primitives::Address, signers::local::PrivateKeySigner, transports::http::reqwest::Url,
    },
    anyhow::Context,
    tokio::process::Command,
};

pub async fn resolve_claim(
    game_address: &Address,
    rpc: &Url,
    signer: &PrivateKeySigner,
) -> anyhow::Result<()> {
    let address_str = game_address.to_string();
    let key_str = alloy::hex::encode(signer.to_bytes());

    let output = Command::new("op-challenger")
        .args([
            "resolve-claim",
            "--log.format",
            "json",
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

    let logs = String::from_utf8(output.stdout).context("Non-utf8 logs from op-challenger")?;
    for line in logs.lines() {
        let Ok(parsed): Result<serde_json::Value, _> = serde_json::from_str(line) else {
            tracing::warn!("Failed to parse op-challenger log {line:?}");
            continue;
        };
        let Some(lvl) = parsed
            .as_object()
            .and_then(|obj| obj.get("lvl"))
            .and_then(|v| v.as_str())
        else {
            continue;
        };
        if lvl == "crit" || lvl == "error" {
            anyhow::bail!("op-challenger execution failed: {line:?}");
        }
    }

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
