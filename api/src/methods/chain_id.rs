use {
    crate::{json_utils::access_state_error, jsonrpc::JsonRpcError},
    moved_app::{Query, StateMessage},
    tokio::sync::{mpsc, oneshot},
};

pub async fn execute(
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<serde_json::Value, JsonRpcError> {
    let response = inner_execute(state_channel).await?;
    Ok(serde_json::to_value(format!("{response:#x}"))
        .expect("Must be able to JSON-serialize response"))
}

async fn inner_execute(state_channel: mpsc::Sender<StateMessage>) -> Result<u64, JsonRpcError> {
    let (tx, rx) = oneshot::channel();
    let msg = Query::ChainId {
        response_channel: tx,
    }
    .into();
    state_channel.send(msg).await.map_err(access_state_error)?;
    rx.await.map_err(access_state_error)
}

#[cfg(test)]
mod tests {
    use {super::*, crate::methods::tests::create_state_actor};

    #[tokio::test]
    async fn test_execute() {
        let (state, state_channel) = create_state_actor();
        let state_handle = state.spawn();

        let expected_response: serde_json::Value = serde_json::from_str(r#""0x194""#).unwrap();

        let response = execute(state_channel).await.unwrap();

        assert_eq!(response, expected_response);
        state_handle.await.unwrap();
    }
}
