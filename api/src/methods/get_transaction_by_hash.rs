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
        crate::{
            methods::{
                forkchoice_updated, get_payload, send_raw_transaction, tests::create_state_actor,
            },
            schema::{ForkchoiceUpdatedResponseV1, GetPayloadResponseV3},
        },
        serde_json::json,
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
        let _block_hash = payload_response.execution_payload.block_hash;

        let request = serde_json::Value::Object(
            iter::once((
                "params".to_string(),
                serde_json::Value::Array(vec![tx_hash]),
            ))
            .collect(),
        );
        let actual_response: serde_json::Value =
            serde_json::from_value(execute(request, state_channel).await.unwrap()).unwrap();
        let expected_response = json!({
            "type": "0x2",
            "chainId": "0x194",
            "nonce": "0x0",
            "gas": "0xffffffffffffffff",
            "maxFeePerGas": "0x0",
            "maxPriorityFeePerGas": "0x0",
            "to": "0x8fd379246834eac74b8419ffda202cf8051f7a03",
            "value": "0x3d",
            "accessList": [],
            "input": "0x",
            "r": "0x78c716fef14bfcb7c2c9ff4abeb741529874fe7046ac042871f9d8490db55f5c",
            "s": "0x1fd5186e08990692d54912b476496f12c48bd7cc540a92d211dde232133ed17",
            "yParity": "0x0",
            "v": "0x0",
            "hash": "0x3545efb3ce7a22353c346c98771640131b81baa64eb03113b20ad2bef5c0ec53",
            "blockHash": null,
            "blockNumber": null,
            "transactionIndex": null,
            "from": "0x0000000000000000000000000000000000000000"
        });

        assert_eq!(actual_response, expected_response);

        state_handle.await.unwrap();
    }
}
