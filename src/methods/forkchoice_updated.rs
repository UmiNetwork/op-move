use crate::types::{
    engine_api::{ForkchoiceStateV1, ForkchoiceUpdatedResponseV1, PayloadAttributesV3},
    jsonrpc::JsonRpcError,
};

pub fn execute_v3(request: serde_json::Value) -> Result<serde_json::Value, JsonRpcError> {
    let (forkchoice_state, payload_attributes) = parse_params_v3(request)?;
    let response = inner_execute_v3(forkchoice_state, payload_attributes)?;
    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

fn parse_params_v3(
    request: serde_json::Value,
) -> Result<(ForkchoiceStateV1, Option<PayloadAttributesV3>), JsonRpcError> {
    // Spec: https://github.com/ethereum/execution-apis/blob/main/src/engine/cancun.md#specification-1
    todo!()
}

fn inner_execute_v3(
    forkchoice_state: ForkchoiceStateV1,
    payload_attributes: Option<PayloadAttributesV3>,
) -> Result<ForkchoiceUpdatedResponseV1, JsonRpcError> {
    todo!()
}
