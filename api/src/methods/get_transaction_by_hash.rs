use {
    crate::{
        json_utils::{access_state_error, parse_params_1},
        jsonrpc::JsonRpcError,
        schema::GetTransactionResponse,
    },
    moved::types::state::{Query, StateMessage},
    moved_shared::primitives::B256,
    tokio::sync::{mpsc, oneshot},
};

pub async fn execute(
    request: serde_json::Value,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<serde_json::Value, JsonRpcError> {
    let tx_hash = parse_params_1(request)?;
    let response = inner_execute(tx_hash, state_channel).await?;
    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

async fn inner_execute(
    tx_hash: B256,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<Option<GetTransactionResponse>, JsonRpcError> {
    let (response_channel, rx) = oneshot::channel();
    let msg = Query::TransactionByHash {
        tx_hash,
        response_channel,
    }
    .into();
    state_channel.send(msg).await.map_err(access_state_error)?;

    Ok(rx
        .await
        .map_err(access_state_error)?
        .map(GetTransactionResponse::from))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::methods::{send_raw_transaction, tests::create_state_actor},
        std::iter,
    };

    #[tokio::test]
    async fn test_execute() {
        let (state, state_channel) = create_state_actor();
        let state_handle = state.spawn();

        let tx_hash = send_raw_transaction::execute(
            send_raw_transaction::tests::example_request(),
            state_channel.clone(),
        )
        .await
        .unwrap();

        let request = serde_json::Value::Object(
            iter::once((
                "params".to_string(),
                serde_json::Value::Array(vec![tx_hash]),
            ))
            .collect(),
        );
        let tx: serde_json::Value =
            serde_json::from_value(execute(request, state_channel).await.unwrap()).unwrap();

        assert_eq!(tx, serde_json::Value::Null);

        state_handle.await.unwrap();
    }
}
