use {
    crate::jsonrpc::JsonRpcError,
    moved::{genesis::config::CHAIN_ID, types::state::StateMessage},
    tokio::sync::mpsc,
};

pub async fn execute(
    _request: serde_json::Value,
    _state_channel: mpsc::Sender<StateMessage>,
) -> Result<serde_json::Value, JsonRpcError> {
    Ok(serde_json::to_value(format!("0x{:x}", CHAIN_ID))
        .expect("Must be able to JSON-serialize response"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute() {
        let (state_channel, _rx) = mpsc::channel(10);

        let request: serde_json::Value = serde_json::from_str(
            r#"
            {
                "id": 30054,
                "jsonrpc": "2.0",
                "method": "eth_chainId",
                "params": []
            }
        "#,
        )
        .unwrap();

        let expected_response: serde_json::Value = serde_json::from_str(r#""0x194""#).unwrap();

        let response = execute(request, state_channel).await.unwrap();

        assert_eq!(response, expected_response);
    }
}
