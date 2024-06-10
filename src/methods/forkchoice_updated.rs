use crate::{
    json_utils,
    types::{
        engine_api::{ForkchoiceStateV1, ForkchoiceUpdatedResponseV1, PayloadAttributesV3},
        jsonrpc::JsonRpcError,
    },
};

#[cfg(test)]
use {
    ethers_core::types::{H160, H256, U64},
    std::str::FromStr,
};

pub fn execute_v3(request: serde_json::Value) -> Result<serde_json::Value, JsonRpcError> {
    let (forkchoice_state, payload_attributes) = parse_params_v3(request)?;
    let response = inner_execute_v3(forkchoice_state, payload_attributes)?;
    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

fn parse_params_v3(
    request: serde_json::Value,
) -> Result<(ForkchoiceStateV1, Option<PayloadAttributesV3>), JsonRpcError> {
    let params = json_utils::get_params_list(&request);
    match params {
        [] => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Not enough params".into(),
        }),
        [x] => {
            let fc_state: ForkchoiceStateV1 = json_utils::deserialize(x)?;
            Ok((fc_state, None))
        }
        [x, y] => {
            let fc_state: ForkchoiceStateV1 = json_utils::deserialize(x)?;
            let payload_attributes: Option<PayloadAttributesV3> = json_utils::deserialize(y)?;
            Ok((fc_state, payload_attributes))
        }
        _ => Err(JsonRpcError {
            code: -32602,
            data: request,
            message: "Too many params".into(),
        }),
    }
}

fn inner_execute_v3(
    forkchoice_state: ForkchoiceStateV1,
    payload_attributes: Option<PayloadAttributesV3>,
) -> Result<ForkchoiceUpdatedResponseV1, JsonRpcError> {
    // Spec: https://github.com/ethereum/execution-apis/blob/main/src/engine/cancun.md#specification-1
    // TODO
    Err(JsonRpcError {
        code: 0,
        data: serde_json::Value::Null,
        message: "Unimplemented".into(),
    })
}

#[test]
fn test_parse_params_v3() {
    // TODO: example request includes `gasLimit` and `transactions` fields
    // that are not part of the Engine API spec?
    let request: serde_json::Value = serde_json::from_str(r#"
        {
            "id": 30053,
            "jsonrpc": "2.0",
            "method": "engine_forkchoiceUpdatedV3",
            "params": [
            {
                "finalizedBlockHash": "0x2c7cb7e2f79c2fa31f2b4280e96c34f7de981c6ccf5d0e998b51f5dc798fa53d",
                "headBlockHash": "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d",
                "safeBlockHash": "0xc9488c812782fac769416f918718107ca8f44f98fd2fe7dbcc12b9f5afa276dd"
            },
            {
                "gasLimit": "0x1c9c380",
                "parentBeaconBlockRoot": "0x2bd857e239f7e5b5e6415608c76b90600d51fa0f7f0bbbc04e2d6861b3186f1c",
                "prevRandao": "0xbde07f5d381bb84700433fe6c0ae077aa40eaad3a5de7abd298f0e3e27e6e4c9",
                "suggestedFeeRecipient": "0x4200000000000000000000000000000000000011",
                "timestamp": "0x6660737b",
                "transactions": [
                    "0x7ef8f8a0de86bef815fc910df65a9459ccb2b9a35fa8596dfcfed1ff01bbf28891d86d5e94deaddeaddeaddeaddeaddeaddeaddeaddead00019442000000000000000000000000000000000000158080830f424080b8a4440a5e2000000558000c5fc50000000000000000000000006660735b00000000000001a9000000000000000000000000000000000000000000000000000000000000000700000000000000000000000000000000000000000000000000000000000000017ae3f74f0134521a7d62a387ac75a5153bcd1aab1c7e003e9b9e15a5d8846363000000000000000000000000e25583099ba105d9ec0a67f5ae86d90e50036425"
                ],
                "withdrawals": []
            }
            ]
        }
    "#).unwrap();

    let params = parse_params_v3(request).unwrap();

    let expected_params = (
        ForkchoiceStateV1 {
            head_block_hash: H256::from_str(
                "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d",
            )
            .unwrap(),
            safe_block_hash: H256::from_str(
                "0xc9488c812782fac769416f918718107ca8f44f98fd2fe7dbcc12b9f5afa276dd",
            )
            .unwrap(),
            finalized_block_hash: H256::from_str(
                "0x2c7cb7e2f79c2fa31f2b4280e96c34f7de981c6ccf5d0e998b51f5dc798fa53d",
            )
            .unwrap(),
        },
        Some(PayloadAttributesV3 {
            timestamp: U64::from_str_radix("0x6660737b", 16).unwrap(),
            prev_randao: H256::from_str(
                "0xbde07f5d381bb84700433fe6c0ae077aa40eaad3a5de7abd298f0e3e27e6e4c9",
            )
            .unwrap(),
            suggested_fee_recipient: H160::from_str("0x4200000000000000000000000000000000000011")
                .unwrap(),
            withdrawals: Vec::new(),
            parent_beacon_block_root: H256::from_str(
                "0x2bd857e239f7e5b5e6415608c76b90600d51fa0f7f0bbbc04e2d6861b3186f1c",
            )
            .unwrap(),
        }),
    );

    assert_eq!(params, expected_params);

    let request: serde_json::Value = serde_json::from_str(r#"
        {
            "id": 32034,
            "jsonrpc": "2.0",
            "method": "engine_forkchoiceUpdatedV3",
            "params": [
                {
                "finalizedBlockHash": "0x2c7cb7e2f79c2fa31f2b4280e96c34f7de981c6ccf5d0e998b51f5dc798fa53d",
                "headBlockHash": "0xb412d0583c92bd00d1987291ba05a894af7483ff9b6e33891a47cf125f400ce2",
                "safeBlockHash": "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d"
                },
                null
            ]
        }
    "#).unwrap();

    let params = parse_params_v3(request).unwrap();

    let expected_params = (
        ForkchoiceStateV1 {
            head_block_hash: H256::from_str(
                "0xb412d0583c92bd00d1987291ba05a894af7483ff9b6e33891a47cf125f400ce2",
            )
            .unwrap(),
            safe_block_hash: H256::from_str(
                "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d",
            )
            .unwrap(),
            finalized_block_hash: H256::from_str(
                "0x2c7cb7e2f79c2fa31f2b4280e96c34f7de981c6ccf5d0e998b51f5dc798fa53d",
            )
            .unwrap(),
        },
        None,
    );

    assert_eq!(params, expected_params);
}
