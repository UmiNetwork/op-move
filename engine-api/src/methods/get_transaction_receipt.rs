use {
    crate::{
        json_utils::{self, access_state_error},
        jsonrpc::JsonRpcError,
    },
    moved::types::state::{Query, StateMessage, TransactionReceipt},
    moved_shared::primitives::B256,
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            methods::{
                forkchoice_updated, get_payload, send_raw_transaction, tests::create_state_actor,
            },
            schema::{ForkchoiceUpdatedResponseV1, GetPayloadResponseV3},
        },
        std::iter,
    };

    #[tokio::test]
    async fn test_execute() {
        let (state, state_channel) = create_state_actor();
        let state_handle = state.spawn();

        // 1. Send transaction
        let tx_hash = send_raw_transaction::execute(
            send_raw_transaction::tests::example_request(),
            state_channel.clone(),
        )
        .await
        .unwrap();

        // 2. Trigger block production
        let forkchoice_response: ForkchoiceUpdatedResponseV1 = serde_json::from_value(
            forkchoice_updated::execute_v3(
                forkchoice_updated::tests::example_request(),
                state_channel.clone(),
            )
            .await
            .unwrap(),
        )
        .unwrap();
        let request = serde_json::Value::Object(
            iter::once((
                "params".to_string(),
                serde_json::Value::Array(vec![serde_json::to_value(
                    forkchoice_response.payload_id.unwrap(),
                )
                .unwrap()]),
            ))
            .collect(),
        );
        let payload_response: GetPayloadResponseV3 = serde_json::from_value(
            get_payload::execute_v3(request, state_channel.clone())
                .await
                .unwrap(),
        )
        .unwrap();
        let block_hash = payload_response.execution_payload.block_hash;

        // 3. Get transaction receipt
        let request = serde_json::Value::Object(
            iter::once((
                "params".to_string(),
                serde_json::Value::Array(vec![tx_hash]),
            ))
            .collect(),
        );
        let receipt: TransactionReceipt =
            serde_json::from_value(execute(request, state_channel).await.unwrap()).unwrap();

        // Confirm the receipt contains correct information about the transaction
        assert_eq!(receipt.inner.transaction_index, Some(2));
        assert_eq!(receipt.inner.block_hash, Some(block_hash));
        assert!(receipt.inner.inner.status());
        assert_eq!(receipt.inner.inner.logs().len(), 2);

        state_handle.await.unwrap();
    }
}
