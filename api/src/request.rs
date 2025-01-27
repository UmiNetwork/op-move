use {
    crate::{
        json_utils,
        jsonrpc::{JsonRpcError, JsonRpcResponse},
        method_name::MethodName,
    },
    moved::types::state::StateMessage,
    tokio::sync::mpsc,
};

pub async fn handle(
    request: serde_json::Value,
    state_channel: mpsc::Sender<StateMessage>,
) -> JsonRpcResponse {
    let id = json_utils::get_field(&request, "id");
    let jsonrpc = json_utils::get_field(&request, "jsonrpc");

    match inner_handle_request(request, state_channel).await {
        Ok(r) => JsonRpcResponse {
            id,
            jsonrpc,
            result: Some(r),
            error: None,
        },
        Err(e) => JsonRpcResponse {
            id,
            jsonrpc,
            result: None,
            error: Some(e),
        },
    }
}

async fn inner_handle_request(
    request: serde_json::Value,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<serde_json::Value, JsonRpcError> {
    use {crate::methods::*, MethodName::*};

    let method: MethodName = json_utils::get_field(&request, "method")
        .as_str()
        .ok_or(JsonRpcError::without_data(-32601, "Invalid/missing method"))?
        .parse()?;

    match method {
        ForkChoiceUpdatedV3 => forkchoice_updated::execute_v3(request, state_channel).await,
        GetPayloadV3 => get_payload::execute_v3(request, state_channel).await,
        NewPayloadV3 => new_payload::execute_v3(request, state_channel).await,
        SendRawTransaction => send_raw_transaction::execute(request, state_channel).await,
        ChainId => chain_id::execute(state_channel).await,
        GetBalance => get_balance::execute(request, state_channel).await,
        GetNonce => get_nonce::execute(request, state_channel).await,
        GetBlockByHash => get_block_by_hash::execute(request, state_channel).await,
        GetBlockByNumber => get_block_by_number::execute(request, state_channel).await,
        BlockNumber => block_number::execute(request, state_channel).await,
        FeeHistory => fee_history::execute(request, state_channel).await,
        EstimateGas => estimate_gas::execute(request, state_channel).await,
        Call => call::execute(request, state_channel).await,
        TransactionReceipt => get_transaction_receipt::execute(request, state_channel).await,
        GetProof => get_proof::execute(request, state_channel).await,
        ForkChoiceUpdatedV2 => todo!(),
        GetPayloadV2 => todo!(),
        NewPayloadV2 => todo!(),
    }
}
