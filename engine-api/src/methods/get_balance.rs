use {
    crate::{json_utils, json_utils::access_state_error, jsonrpc::JsonRpcError},
    alloy::{eips::BlockNumberOrTag, primitives::Address},
    moved::types::state::StateMessage,
    serde_json::Value,
    tokio::sync::{mpsc, oneshot},
};

pub async fn execute(
    request: Value,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<Value, JsonRpcError> {
    let (address, block_number) = parse_params(request)?;
    let response = inner_execute(address, block_number, state_channel).await?;

    // Format the balance as a hex string
    Ok(serde_json::to_value(format!("0x{:x}", response))
        .expect("Must be able to JSON-serialize response"))
}

fn parse_params(request: Value) -> Result<(Address, BlockNumberOrTag), JsonRpcError> {
    let params = json_utils::get_params_list(&request);
    match params {
        [] => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Not enough params".into(),
        }),
        [a, b] => {
            let address: Address = json_utils::deserialize(a)?;
            let block_number: BlockNumberOrTag = json_utils::deserialize(b)?;
            Ok((address, block_number))
        }
        _ => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Too many params".into(),
        }),
    }
}

async fn inner_execute(
    address: Address,
    block_number: BlockNumberOrTag,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<u64, JsonRpcError> {
    let (tx, rx) = oneshot::channel();
    let msg = StateMessage::GetBalance {
        address,
        block_number,
        response_channel: tx,
    };
    state_channel.send(msg).await.map_err(access_state_error)?;
    let response = rx.await.map_err(access_state_error)?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use {super::*, moved::primitives::U64, std::str::FromStr, test_case::test_case};

    #[test_case("0x1")]
    #[test_case("latest")]
    #[test_case("pending")]
    fn test_parse_params_with_block_number(block: &str) {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getBalance",
            "params": [
                "0x742d35Cc6634C0532925a3b844Bc454e4438f44e",
                block,
            ],
            "id": 1
        });

        let (address, block_number) = parse_params(request).unwrap();
        assert_eq!(
            address,
            Address::from_str("0x742d35Cc6634C0532925a3b844Bc454e4438f44e").unwrap()
        );
        match block {
            "latest" => assert_eq!(block_number, BlockNumberOrTag::Latest),
            "pending" => assert_eq!(block_number, BlockNumberOrTag::Pending),
            _ => assert_eq!(
                block_number,
                BlockNumberOrTag::Number(U64::from_str(block).unwrap().into_limbs()[0])
            ),
        }
    }
}
