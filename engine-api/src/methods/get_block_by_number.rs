use {
    crate::{
        json_utils,
        jsonrpc::JsonRpcError,
        schema::{BlockNumberOrTag, GetBlockResponse},
    },
    alloy::hex,
    moved::{
        block::{Block, ExtendedBlock},
        primitives::{B256, U256},
        types::state::{BlockResponse, StateMessage},
    },
    tokio::sync::mpsc,
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
    _number: BlockNumberOrTag,
    _include_transactions: bool,
    _state_channel: mpsc::Sender<StateMessage>,
) -> Result<Option<GetBlockResponse>, JsonRpcError> {
    // TODO: Replace dummy response
    Ok(Some(GetBlockResponse::from(BlockResponse::from(
        ExtendedBlock::new(
            B256::new(hex!(
                "e56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d"
            )),
            U256::ZERO,
            Block::default(),
        ),
    ))))
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
        super::*,
        alloy::hex,
        moved::{
            block::{Block, BlockRepository, Eip1559GasFee, InMemoryBlockRepository},
            genesis::{config::GenesisConfig, init_state},
            primitives::{B256, U256},
            storage::InMemoryState,
        },
    };

    pub fn example_request() -> serde_json::Value {
        serde_json::from_str(
            r#"
            {
                "id": 1,
                "jsonrpc": "2.0",
                "method": "eth_getBlockByHash",
                "params": [
                    "0x0",
                    false
                ]
            }
        "#,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_execute_reads_genesis_block_successfully() {
        let genesis_config = GenesisConfig::default();
        let (state_channel, rx) = mpsc::channel(10);

        let head_hash = B256::new(hex!(
            "e56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d"
        ));
        let genesis_block = Block::default().with_hash(head_hash).with_value(U256::ZERO);

        let mut repository = InMemoryBlockRepository::new();
        repository.add(genesis_block);

        let mut state = InMemoryState::new();
        init_state(&genesis_config, &mut state);

        let state = moved::state_actor::StateActor::new(
            rx,
            state,
            head_hash,
            genesis_config,
            0x03421ee50df45cacu64,
            B256::ZERO,
            repository,
            Eip1559GasFee::default(),
            U256::ZERO,
            (),
        );
        let state_handle = state.spawn();
        let request = example_request();

        let expected_response: serde_json::Value = serde_json::from_str(r#"
        {
            "hash": "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d",
            "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "sha3Uncles": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "miner": "0x0000000000000000000000000000000000000000",
            "stateRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "transactionsRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "receiptsRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "logsBloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
            "difficulty": "0x0",
            "number": "0x0",
            "gasLimit": "0x0",
            "gasUsed": "0x0",
            "timestamp": "0x0",
            "extraData": "0x",
            "nonce": "0x0000000000000000",
            "baseFeePerGas": "0x0",
            "blobGasUsed": "0x0",
            "excessBlobGas": "0x0",
            "parentBeaconBlockRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "uncles": []
        }"#).unwrap();

        let response = execute(request, state_channel).await.unwrap();

        assert_eq!(response, expected_response);
        state_handle.await.unwrap();
    }
}
