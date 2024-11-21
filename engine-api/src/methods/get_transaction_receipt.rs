use {
    crate::{
        json_utils::{self, access_state_error},
        jsonrpc::JsonRpcError,
    },
    moved::{
        primitives::B256,
        types::state::{Query, StateMessage, TransactionReceipt},
    },
    tokio::sync::{mpsc, oneshot},
};

pub async fn execute(
    request: serde_json::Value,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<serde_json::Value, JsonRpcError> {
    let tx_hash = parse_params(request)?;
    let response = inner_execute(tx_hash, state_channel).await?;
    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

async fn inner_execute(
    tx_hash: B256,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<Option<TransactionReceipt>, JsonRpcError> {
    let (response_channel, rx) = oneshot::channel();
    let msg = Query::TransactionReceipt {
        tx_hash,
        response_channel,
    }
    .into();
    state_channel.send(msg).await.map_err(access_state_error)?;
    let maybe_response = rx.await.map_err(access_state_error)?;

    Ok(maybe_response)
}

fn parse_params(request: serde_json::Value) -> Result<B256, JsonRpcError> {
    let params = json_utils::get_params_list(&request);
    match params {
        [] => Err(JsonRpcError::parse_error(request, "Not enough params")),
        [x] => {
            let tx_hash: B256 = json_utils::deserialize(x)?;
            Ok(tx_hash)
        }
        _ => Err(JsonRpcError::parse_error(request, "Too many params")),
    }
}
