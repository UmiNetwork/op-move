use {
    crate::{json_utils::access_state_error, jsonrpc::JsonRpcError},
    moved::types::state::StateMessage,
    tokio::sync::{mpsc, oneshot},
};

pub async fn execute(
    _request: serde_json::Value,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<serde_json::Value, JsonRpcError> {
    let response = inner_execute(state_channel).await?;
    Ok(serde_json::to_value(format!("{response:#x}"))
        .expect("Must be able to JSON-serialize response"))
}

async fn inner_execute(state_channel: mpsc::Sender<StateMessage>) -> Result<u64, JsonRpcError> {
    let (tx, rx) = oneshot::channel();
    let msg = StateMessage::ChainId {
        response_channel: tx,
    };
    state_channel.send(msg).await.map_err(access_state_error)?;
    rx.await.map_err(access_state_error)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        moved::{
            block::{Eip1559GasFee, InMemoryBlockRepository},
            primitives::{B256, U256},
            state_actor::StatePayloadId,
            storage::InMemoryState,
        },
    };

    #[tokio::test]
    async fn test_execute() {
        let genesis_config = moved::genesis::config::GenesisConfig::default();
        let (state_channel, rx) = mpsc::channel(10);
        let state = moved::state_actor::StateActor::new(
            rx,
            InMemoryState::new(),
            B256::ZERO,
            genesis_config,
            StatePayloadId,
            B256::ZERO,
            InMemoryBlockRepository::default(),
            Eip1559GasFee::default(),
            U256::ZERO,
            (),
        );
        let state_handle = state.spawn();

        let request = serde_json::json!({
            "id": 30054,
            "jsonrpc": "2.0",
            "method": "eth_chainId",
            "params": []
        });

        let expected_response: serde_json::Value = serde_json::from_str(r#""0x194""#).unwrap();

        let response = execute(request, state_channel).await.unwrap();

        assert_eq!(response, expected_response);
        state_handle.await.unwrap();
    }
}
