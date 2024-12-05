use {
    crate::{
        json_utils::{self, access_state_error},
        jsonrpc::JsonRpcError,
        schema::{BlockNumberOrTag, GetBlockResponse},
    },
    moved::types::state::{Query, StateMessage},
    tokio::sync::{mpsc, oneshot},
};

pub async fn execute(
    request: serde_json::Value,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<serde_json::Value, JsonRpcError> {
    let (number, include_transactions) = parse_params(request)?;
    let response = inner_execute(number, include_transactions, state_channel).await?;
    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

async fn inner_execute(
    height: BlockNumberOrTag,
    include_transactions: bool,
    state_channel: mpsc::Sender<StateMessage>,
) -> Result<Option<GetBlockResponse>, JsonRpcError> {
    let (response_channel, rx) = oneshot::channel();
    let msg = Query::BlockByHeight {
        height,
        include_transactions,
        response_channel,
    }
    .into();
    state_channel.send(msg).await.map_err(access_state_error)?;
    let maybe_response = rx.await.map_err(access_state_error)?;

    Ok(maybe_response.map(GetBlockResponse::from))
}

fn parse_params(request: serde_json::Value) -> Result<(BlockNumberOrTag, bool), JsonRpcError> {
    let params = json_utils::get_params_list(&request);
    match params {
        [] | [_] => Err(JsonRpcError::parse_error(request, "Not enough params")),
        [x, y] => {
            let number: BlockNumberOrTag = json_utils::deserialize(x)?;
            let include_transactions: bool = json_utils::deserialize(y)?;
            Ok((number, include_transactions))
        }
        _ => Err(JsonRpcError::parse_error(request, "Too many params")),
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*, crate::methods::tests::create_state_actor, alloy::eips::BlockNumberOrTag::*,
        moved::types::state::Command, test_case::test_case,
    };

    pub fn example_request(tag: BlockNumberOrTag) -> serde_json::Value {
        serde_json::json!({
            "id": 1,
            "jsonrpc": "2.0",
            "method": "eth_getBlockByNumber",
            "params": [tag, false]
        })
    }

    pub fn get_block_number_from_response(response: serde_json::Value) -> String {
        response
            .as_object()
            .unwrap()
            .get("number") // Block number
            .unwrap()
            .as_str()
            .unwrap()
            .to_string()
    }

    #[tokio::test]
    async fn test_execute_reads_genesis_block_successfully() {
        let (state, state_channel) = create_state_actor();
        let state_handle = state.spawn();
        let request = example_request(Number(0));

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
            "transactions": []
        }"#).unwrap();

        let response = execute(request, state_channel).await.unwrap();

        assert_eq!(response, expected_response);
        state_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_latest_block_height_is_updated_with_newly_built_block() {
        let (state, state_channel) = create_state_actor();
        let state_handle = state.spawn();

        let request = example_request(Latest);
        let response = execute(request, state_channel.clone()).await.unwrap();
        assert_eq!(get_block_number_from_response(response), "0x0");

        // Create a block, so the block height becomes 1
        let (tx, _) = oneshot::channel();
        let msg = Command::StartBlockBuild {
            payload_attributes: Default::default(),
            response_channel: tx,
        }
        .into();
        state_channel.send(msg).await.unwrap();

        let request = example_request(Latest);
        let response = execute(request, state_channel).await.unwrap();
        assert_eq!(get_block_number_from_response(response), "0x1");
        state_handle.await.unwrap();
    }

    #[test_case(Safe; "safe")]
    #[test_case(Pending; "pending")]
    #[test_case(Finalized; "finalized")]
    #[tokio::test]
    async fn test_latest_block_height_is_same_as_tag(tag: BlockNumberOrTag) {
        let (state, state_channel) = create_state_actor();
        let state_handle = state.spawn();

        let (tx, _) = oneshot::channel();
        let msg = Command::StartBlockBuild {
            payload_attributes: Default::default(),
            response_channel: tx,
        }
        .into();
        state_channel.send(msg).await.unwrap();

        let request = example_request(tag);
        let response = execute(request, state_channel).await.unwrap();
        assert_eq!(get_block_number_from_response(response), "0x1");
        state_handle.await.unwrap();
    }
}
