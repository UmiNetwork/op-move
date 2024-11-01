use {moved_engine_api::jsonrpc::JsonRpcResponse, serde::Serialize};

#[derive(Debug, Serialize)]
pub struct MirrorLog<'a> {
    pub request: &'a serde_json::Value,
    pub geth_response: &'a serde_json::Value,
    pub op_move_response: &'a JsonRpcResponse,
    pub port: &'a str,
}
