//! This module is designed to parse the
//! [Superchain Token List](https://github.com/ethereum-optimism/ethereum-optimism.github.io).
//! The idea is that we will automatically bridge the same tokens that exist on Optimism
//! onto our network. We will only do this for standard bridged tokens. Therefore we ignore
//! any tokens marked as `nonstandard` or `nobridge` as well as any tokens that override
//! the Optimism bridge (i.e. did not use the standard bridge).

use {
    alloy::primitives::Address,
    anyhow::Result,
    std::{
        fs::{read_dir, read_to_string},
        path::Path,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgedToken {
    name: String,
    symbol: String,
    decimals: u8,
    ethereum_address: Address,
}

pub fn parse_token_list(path: &Path) -> Result<Vec<BridgedToken>> {
    let mut result = Vec::new();

    // If the path is not a directory then we assume it is a single
    // file containing a JSON list of the token entries.
    if !path.is_dir() {
        let data = read_to_string(path)?;
        let json: serde_json::Value = serde_json::from_str(&data)?;
        let array = json
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Single file must contain an array"))?;
        for value in array {
            if let Some(token) = parse_json(value)? {
                result.push(token);
            }
        }
        return Ok(result);
    }

    for maybe_entry in read_dir(path)? {
        let entry = maybe_entry?;
        if entry.metadata()?.is_dir() {
            let data_path = entry.path().join("data.json");
            if let Some(token) = parse_single_data_file(&data_path)? {
                result.push(token);
            }
        }
    }

    Ok(result)
}

fn parse_single_data_file(path: &Path) -> Result<Option<BridgedToken>> {
    let data = read_to_string(path)?;
    let json: serde_json::Value = serde_json::from_str(&data)?;
    parse_json(&json)
}

fn parse_json(json: &serde_json::Value) -> Result<Option<BridgedToken>> {
    let object = json
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Must be object"))?;

    let bool_or_true = |v: &serde_json::Value| -> bool { v.as_bool().unwrap_or(true) };

    // We only automatically bridge standard tokens bridged to Optimism from Ethereum.
    // If these fields are present then this is not the case.
    let is_non_standard = object.get("nonstandard").map(bool_or_true).unwrap_or(false);
    let is_no_bridge = object.get("nobridge").map(bool_or_true).unwrap_or(false);
    if is_non_standard || is_no_bridge {
        return Ok(None);
    }

    let mut name = object
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Name must be a string"))?;
    let mut symbol = object
        .get("symbol")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Symbol must be a string"))?;
    let mut decimals = object
        .get("decimals")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("Decimals must be a number"))?;

    let tokens = object
        .get("tokens")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow::anyhow!("Tokens must be an object"))?;
    let Some(ethereum) = tokens.get("ethereum").and_then(|v| v.as_object()) else {
        return Ok(None);
    };
    let Some(optimism) = tokens.get("optimism").and_then(|v| v.as_object()) else {
        return Ok(None);
    };

    // If a different bridge than the Optimism standard bridge was used then
    // we do not automatically bridge it ourselves.
    if has_ethereum_bridge_override(ethereum).unwrap_or(false)
        || has_optimism_bridge_override(optimism).unwrap_or(false)
    {
        return Ok(None);
    }

    let ethereum_address = ethereum
        .get("address")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Ethereum address must be a string"))?;
    let ethereum_address: Address = ethereum_address.parse()?;

    // ETH is our base token, so no need to bridge it again
    if ethereum_address.is_zero() {
        return Ok(None);
    }

    // Check if any metadata has overrides.
    if let Some(overrides) = optimism.get("overrides").and_then(|v| v.as_object()) {
        if let Some(alt_name) = overrides.get("name").and_then(|v| v.as_str()) {
            name = alt_name;
        }
        if let Some(alt_symbol) = overrides.get("symbol").and_then(|v| v.as_str()) {
            symbol = alt_symbol;
        }
        if let Some(alt_decimals) = overrides.get("decimals").and_then(|v| v.as_u64()) {
            decimals = alt_decimals;
        }
    }

    if decimals > u8::MAX as u64 {
        anyhow::bail!("Invalid value for token decimals");
    }

    Ok(Some(BridgedToken {
        name: name.into(),
        symbol: symbol.into(),
        decimals: decimals as u8,
        ethereum_address,
    }))
}

fn has_ethereum_bridge_override(
    ethereum: &serde_json::Map<String, serde_json::Value>,
) -> Option<bool> {
    Some(
        ethereum
            .get("overrides")?
            .as_object()?
            .get("bridge")?
            .as_object()?
            .contains_key("optimism"),
    )
}

fn has_optimism_bridge_override(
    optimism: &serde_json::Map<String, serde_json::Value>,
) -> Option<bool> {
    Some(
        optimism
            .get("overrides")?
            .as_object()?
            .contains_key("bridge"),
    )
}

#[test]
fn test_read_dir() {
    let path = Path::new("/home/birchmd/rust/duo/op-stack/ethereum-optimism.github.io/data/");
    let x = parse_token_list(path).unwrap();
    for token in x {
        println!("{}", token.name);
    }
}
