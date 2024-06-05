use ethers_core::types::H256;

use crate::types::{
    engine_api::{ExecutionPayloadV3, PayloadStatusV1},
    jsonrpc::JsonRpcError,
};

pub fn execute_v3(request: serde_json::Value) -> Result<serde_json::Value, JsonRpcError> {
    let (execution_payload, expected_blob_versioned_hashes, parent_beacon_block_root) =
        parse_params_v3(request)?;
    let response = inner_execute_v3(
        execution_payload,
        expected_blob_versioned_hashes,
        parent_beacon_block_root,
    )?;
    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

fn parse_params_v3(
    request: serde_json::Value,
) -> Result<(ExecutionPayloadV3, Vec<H256>, H256), JsonRpcError> {
    // Spec: https://github.com/ethereum/execution-apis/blob/main/src/engine/cancun.md#specification
    // TODO
    Err(JsonRpcError {
        code: 0,
        data: serde_json::Value::Null,
        message: "Unimplemented".into(),
    })
}

fn inner_execute_v3(
    execution_payload: ExecutionPayloadV3,
    expected_blob_versioned_hashes: Vec<H256>,
    parent_beacon_block_root: H256,
) -> Result<PayloadStatusV1, JsonRpcError> {
    // TODO
    Err(JsonRpcError {
        code: 0,
        data: serde_json::Value::Null,
        message: "Unimplemented".into(),
    })
}
