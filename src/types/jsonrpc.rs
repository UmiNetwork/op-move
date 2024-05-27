#[derive(Debug, serde::Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub data: serde_json::Value,
    pub message: String,
}

#[derive(Debug, serde::Serialize)]
pub struct JsonRpcResponse {
    pub id: serde_json::Value,
    pub jsonrpc: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}
