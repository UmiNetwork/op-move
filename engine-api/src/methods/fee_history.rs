use {
    crate::{json_utils, json_utils::access_state_error, jsonrpc::JsonRpcError},
    alloy::{eips::BlockNumberOrTag, rpc::types::FeeHistory},
    moved::types::state::{Query, StateMessage},
    tokio::sync::{mpsc, oneshot},
};

pub async fn execute(
    request: serde_json::Value,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<serde_json::Value, JsonRpcError> {
    let (block_count, block_number, reward_percentiles) = parse_params(request)?;
    let response =
        inner_execute(block_count, block_number, reward_percentiles, state_channel).await?;
    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

fn parse_params(
    request: serde_json::Value,
) -> Result<(u64, BlockNumberOrTag, Option<Vec<f64>>), JsonRpcError> {
    let params = json_utils::get_params_list(&request);
    match params {
        [] => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Not enough params".into(),
        }),
        [a, b] => {
            let block_count: String = json_utils::deserialize(a)?;
            let block_count = block_count.trim_start_matches("0x");
            let block_count: u64 =
                u64::from_str_radix(block_count, 16).map_err(|_| JsonRpcError {
                    code: -32602,
                    data: 0.into(),
                    message: "Block count parsing error".into(),
                })?;
            let block_number: BlockNumberOrTag = json_utils::deserialize(b)?;
            Ok((block_count, block_number, None))
        }
        [a, b, c] => {
            let block_count: String = json_utils::deserialize(a)?;
            let block_count = block_count.trim_start_matches("0x");
            let block_count: u64 =
                u64::from_str_radix(block_count, 16).map_err(|_| JsonRpcError {
                    code: -32602,
                    data: 0.into(),
                    message: "Block count parsing error".into(),
                })?;
            let reward_percentiles: Vec<f64> = json_utils::deserialize(b)?;
            let block_number: BlockNumberOrTag = json_utils::deserialize(c)?;
            Ok((block_count, block_number, Some(reward_percentiles)))
        }
        _ => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Too many params".into(),
        }),
    }
}

async fn inner_execute(
    block_count: u64,
    block_number: BlockNumberOrTag,
    reward_percentiles: Option<Vec<f64>>,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<FeeHistory, JsonRpcError> {
    let (tx, rx) = oneshot::channel();
    let msg = Query::FeeHistory {
        block_count,
        block_number,
        reward_percentiles,
        response_channel: tx,
    }
    .into();
    state_channel.send(msg).await.map_err(access_state_error)?;
    let response = rx.await.map_err(access_state_error)?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        moved::{
            block::{BlockMemory, Eip1559GasFee, InMemoryBlockQueries, InMemoryBlockRepository},
            genesis::init_state,
            primitives::{B256, U256, U64},
            state_actor::StatePayloadId,
            storage::InMemoryState,
        },
        std::str::FromStr,
        test_case::test_case,
    };

    #[test_case("0x1")]
    #[test_case("latest")]
    #[test_case("pending")]
    fn test_parse_params(block: &str) {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_feeHistory",
            "params": [
                "0x2",
                [
                    20.0
                ],
                block,
            ],
            "id": 1
        });

        let (block_count, block_number, reward_percentiles) =
            parse_params(request.clone()).unwrap();
        assert_eq!(block_count, 2);
        assert_eq!(reward_percentiles, Some(vec![20f64]));
        match block {
            "latest" => assert_eq!(block_number, BlockNumberOrTag::Latest),
            "pending" => assert_eq!(block_number, BlockNumberOrTag::Pending),
            _ => assert_eq!(
                block_number,
                BlockNumberOrTag::Number(U64::from_str(block).unwrap().into_limbs()[0])
            ),
        }
    }

    #[test_case("0x1")]
    #[test_case("latest")]
    #[test_case("pending")]
    #[tokio::test]
    async fn test_execute(block: &str) {
        let genesis_config = moved::genesis::config::GenesisConfig::default();
        let mut state = InMemoryState::new();
        init_state(&genesis_config, &mut state);

        let (state_channel, rx) = mpsc::channel(10);

        let state_actor = moved::state_actor::StateActor::new(
            rx,
            state,
            B256::ZERO,
            genesis_config,
            StatePayloadId,
            B256::ZERO,
            InMemoryBlockRepository::new(),
            Eip1559GasFee::default(),
            U256::ZERO,
            (),
            InMemoryBlockQueries,
            BlockMemory::new(),
        );

        let state_handle = state_actor.spawn();
        let request: serde_json::Value = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_feeHistory",
            "params": [
                "0x2",
                [
                    20.0
                ],
                block,
            ],
            "id": 1
        });

        let expected_response: serde_json::Value =
            serde_json::json!({"gasUsedRatio": [], "oldestBlock": "0x0"});
        let response = execute(request, state_channel).await.unwrap();

        assert_eq!(response, expected_response);
        state_handle.await.unwrap();
    }
}
