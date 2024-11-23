use {
    crate::{json_utils, json_utils::access_state_error, jsonrpc::JsonRpcError},
    moved::types::state::{Query, StateMessage},
    tokio::sync::{mpsc, oneshot},
};

pub async fn execute(
    request: serde_json::Value,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<serde_json::Value, JsonRpcError> {
    parse_params(request)?;
    let response = inner_execute(state_channel).await?;

    // Format the block number as a hex string
    Ok(serde_json::to_value(format!("0x{:x}", response))
        .expect("Must be able to JSON-serialize response"))
}

fn parse_params(request: serde_json::Value) -> Result<(), JsonRpcError> {
    let params = json_utils::get_params_list(&request);
    match params {
        [] => Ok(()),
        _ => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Too many params".into(),
        }),
    }
}

async fn inner_execute(state_channel: mpsc::Sender<StateMessage>) -> Result<u64, JsonRpcError> {
    let (tx, rx) = oneshot::channel();
    let msg = Query::BlockNumber {
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
            primitives::{B256, U256},
            state_actor::StatePayloadId,
            storage::InMemoryState,
        },
    };

    #[tokio::test]
    async fn test_execute() {
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
            "method": "eth_blockNumber",
            "params": [],
            "id": 1
        });

        let expected_response: serde_json::Value = serde_json::from_str(r#""0x0""#).unwrap();
        let response = execute(request, state_channel).await.unwrap();

        assert_eq!(response, expected_response);
        state_handle.await.unwrap();
    }
}
