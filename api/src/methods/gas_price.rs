use crate::jsonrpc::JsonRpcError;

pub async fn execute() -> Result<serde_json::Value, JsonRpcError> {
    Ok(serde_json::to_value("0x3b9aca00").expect("Must be able to JSON-serialize response"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute() {
        // TODO: Return the actual gas price, currently hardcoded to 1,000,000,000
        let expected_response: serde_json::Value = serde_json::from_str(r#""0x3b9aca00""#).unwrap();
        let response = execute().await.unwrap();
        assert_eq!(response, expected_response);
    }
}
