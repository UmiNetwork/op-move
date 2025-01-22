use {
    crate::{
        json_utils::{self, access_state_error},
        jsonrpc::JsonRpcError,
        schema::GetBlockResponse,
    },
    moved::{
        primitives::B256,
        types::state::{Query, StateMessage},
    },
    tokio::sync::{mpsc, oneshot},
};

pub async fn execute(
    request: serde_json::Value,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<serde_json::Value, JsonRpcError> {
    let (block_hash, include_transactions) = parse_params(request)?;
    let response = inner_execute(block_hash, include_transactions, state_channel).await?;
    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

async fn inner_execute(
    hash: B256,
    include_transactions: bool,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<Option<GetBlockResponse>, JsonRpcError> {
    let (response_channel, rx) = oneshot::channel();
    let msg = Query::BlockByHash {
        hash,
        include_transactions,
        response_channel,
    }
    .into();
    state_channel.send(msg).await.map_err(access_state_error)?;
    let maybe_response = rx.await.map_err(access_state_error)?;

    Ok(maybe_response.map(GetBlockResponse::from))
}

fn parse_params(request: serde_json::Value) -> Result<(B256, bool), JsonRpcError> {
    let params = json_utils::get_params_list(&request);
    match params {
        [] | [_] => Err(JsonRpcError::parse_error(request, "Not enough params")),
        [x, y] => {
            let block_hash: B256 = json_utils::deserialize(x)?;
            let include_transactions: bool = json_utils::deserialize(y)?;
            Ok((block_hash, include_transactions))
        }
        _ => Err(JsonRpcError::parse_error(request, "Too many params")),
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::methods::tests::create_state_actor};

    pub fn example_request() -> serde_json::Value {
        serde_json::from_str(
            r#"
            {
                "id": 1,
                "jsonrpc": "2.0",
                "method": "eth_getBlockByHash",
                "params": [
                    "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d",
                    false
                ]
            }
        "#,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_execute_reads_genesis_block_successfully() {
        let (state, state_channel) = create_state_actor();
        let state_handle = state.spawn();
        let request = example_request();

        let expected_response: serde_json::Value = serde_json::from_str(r#"
        {
            "hash": "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d",
            "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
            "miner": "0x0000000000000000000000000000000000000000",
            "stateRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
            "transactionsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
            "receiptsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
            "logsBloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
            "difficulty": "0x0",
            "number": "0x0",
            "gasLimit": "0x0",
            "gasUsed": "0x0",
            "timestamp": "0x0",
            "extraData": "0x",
            "mixHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "nonce": "0x0000000000000000",
            "uncles": [],
            "transactions": [],
            "withdrawals": []
        }"#).unwrap();

        let response = execute(request, state_channel).await.unwrap();

        assert_eq!(response, expected_response);
        state_handle.await.unwrap();
    }
}
