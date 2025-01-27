use {
    crate::{
        json_utils::{access_state_error, parse_params_0},
        jsonrpc::JsonRpcError,
    },
    moved::types::state::{Query, StateMessage},
    tokio::sync::{mpsc, oneshot},
};

pub async fn execute(
    request: serde_json::Value,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<serde_json::Value, JsonRpcError> {
    parse_params_0(request)?;
    let response = inner_execute(state_channel).await?;

    // Format the block number as a hex string
    Ok(serde_json::to_value(format!("0x{:x}", response))
        .expect("Must be able to JSON-serialize response"))
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
    use {super::*, crate::methods::tests::create_state_actor};

    #[tokio::test]
    async fn test_execute() {
        let (state_actor, state_channel) = create_state_actor();

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
