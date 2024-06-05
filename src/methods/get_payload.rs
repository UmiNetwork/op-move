use crate::types::{
    engine_api::{GetPayloadResponseV3, PayloadId},
    jsonrpc::JsonRpcError,
};

pub fn execute_v3(request: serde_json::Value) -> Result<serde_json::Value, JsonRpcError> {
    let payload_id = parse_params_v3(request)?;
    let response = inner_execute_v3(payload_id)?;
    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

fn parse_params_v3(request: serde_json::Value) -> Result<PayloadId, JsonRpcError> {
    // Spec: https://github.com/ethereum/execution-apis/blob/main/src/engine/cancun.md#specification-2
    // TODO
    Err(JsonRpcError {
        code: 0,
        data: serde_json::Value::Null,
        message: "Unimplemented".into(),
    })
}

fn inner_execute_v3(payload_id: PayloadId) -> Result<GetPayloadResponseV3, JsonRpcError> {
    // TODO
    Err(JsonRpcError {
        code: 0,
        data: serde_json::Value::Null,
        message: "Unimplemented".into(),
    })
}
