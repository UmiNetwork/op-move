use {
    crate::{json_utils::parse_params_3, jsonrpc::JsonRpcError},
    umi_app::{ApplicationReader, Dependencies},
    umi_shared::primitives::{Address, ToMoveAddress},
};

/// Fetches `module` by name that belongs to the `account` in the blockchain state corresponding
/// with the block `number`.
///
/// # Arguments
/// * `account`: A "0x" prefixed, 20-byte long, hex encoded number that represents an Ethereum
///   account address. Example: `0x0000000000000000000000000000000000000001`
/// * `module`: A string encoded identifier of the module. Example: `event`
/// * `number`: A string that represents a tagged block height, or a "0x" prefixed, hex encoded
///   number that represents the exact block height to read from. Example: `latest`
pub async fn execute<'reader>(
    request: serde_json::Value,
    app: &ApplicationReader<'reader, impl Dependencies<'reader>>,
) -> Result<serde_json::Value, JsonRpcError> {
    let (account, module, number): (Address, String, _) = parse_params_3(request)?;

    let response = app.move_module_by_height(account.to_move_address(), module.as_str(), number)?;

    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::methods::tests::{create_app, send_command},
        alloy::eips::BlockNumberOrTag::{self, *},
        test_case::test_case,
        tokio::sync::mpsc,
        umi_app::{Command, CommandActor, TestDependencies},
        umi_shared::primitives::{ToEthAddress, U64},
    };

    const BYTE_CODE: &str = "0xa11ceb0b0800000a0e01000602060a03104e045e06056430079401a10108b5022010d502d9010aae04080bb604020cb804760dae05040eb205040fb60504000000030009000104010601010406000003000101060101010002000401060101010005050301060101010006020301060101010007020301060100000806030106010101020a08090100010101000b0a0301060100000c07050106010004020607070201060b0001090001060801010900000103010b0001090002070b00010900090001080101060900010a02030a02030900056576656e740b4576656e7448616e646c6507636f756e746572046775696404475549440e64657374726f795f68616e646c6504656d69741b77726974655f6d6f64756c655f6576656e745f746f5f73746f72650a656d69745f6576656e740362637308746f5f62797465731477726974655f746f5f6576656e745f73746f7265106e65775f6576656e745f68616e646c65076163636f756e74066f626a656374000000000000000000000000000000000000000000000000000000000000000114636f6d70696c6174696f6e5f6d65746164617461090003322e3003322e32126170746f733a3a6d657461646174615f7631a501010100000000000000144543414e4e4f545f4352454154455f4556454e548301416e206576656e742063616e6e6f7420626520637265617465642e2054686973206572726f722069732072657475726e6564206279206e617469766520696d706c656d656e746174696f6e73207768656e0a2020202d2054686520747970652074616720666f72206576656e7420697320746f6f20646565706c79206e65737465642e0000000202020303080100020001000003030b003700020101000003040b00370114020201000003050b003a000101020301000003030b003800020501000003120a00370038010a003701140b013802280a00370114060100000000000000160b00360115020803000003040600000000000000000b0039000204000200070002000001000000020102000d000e00";

    pub fn example_request(tag: BlockNumberOrTag) -> serde_json::Value {
        serde_json::json!({
            "id": 1,
            "jsonrpc": "2.0",
            "method": "mv_getModule",
            "params": [umi_genesis::FRAMEWORK_ADDRESS.to_eth_address(), "event", tag]
        })
    }

    pub fn bytecode_from_response(response: serde_json::Value) -> String {
        response
            .as_object()
            .unwrap()
            .get("bytecode")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string()
    }

    #[tokio::test]
    async fn test_execute_reads_module_successfully() {
        let (reader, _app) = create_app();
        let request = example_request(Number(0));

        let expected_response: serde_json::Value = serde_json::from_str(r#"
            {
            "bytecode": "0xa11ceb0b0800000a0e01000602060a03104e045e06056430079401a10108b5022010d502d9010aae04080bb604020cb804760dae05040eb205040fb60504000000030009000104010601010406000003000101060101010002000401060101010005050301060101010006020301060101010007020301060100000806030106010101020a08090100010101000b0a0301060100000c07050106010004020607070201060b0001090001060801010900000103010b0001090002070b00010900090001080101060900010a02030a02030900056576656e740b4576656e7448616e646c6507636f756e746572046775696404475549440e64657374726f795f68616e646c6504656d69741b77726974655f6d6f64756c655f6576656e745f746f5f73746f72650a656d69745f6576656e740362637308746f5f62797465731477726974655f746f5f6576656e745f73746f7265106e65775f6576656e745f68616e646c65076163636f756e74066f626a656374000000000000000000000000000000000000000000000000000000000000000114636f6d70696c6174696f6e5f6d65746164617461090003322e3003322e32126170746f733a3a6d657461646174615f7631a501010100000000000000144543414e4e4f545f4352454154455f4556454e548301416e206576656e742063616e6e6f7420626520637265617465642e2054686973206572726f722069732072657475726e6564206279206e617469766520696d706c656d656e746174696f6e73207768656e0a2020202d2054686520747970652074616720666f72206576656e7420697320746f6f20646565706c79206e65737465642e0000000202020303080100020001000003030b003700020101000003040b00370114020201000003050b003a000101020301000003030b003800020501000003120a00370038010a003701140b013802280a00370114060100000000000000160b00360115020803000003040600000000000000000b0039000204000200070002000001000000020102000d000e00",
            "abi": {
                "address": "0x1",
                "name": "event",
                "friends": [
                "0x1::account",
                "0x1::object"
                ],
                "exposed_functions": [
                {
                    "name": "guid",
                    "visibility": "public",
                    "is_entry": false,
                    "is_view": false,
                    "generic_type_params": [
                    {
                        "constraints": [
                        "drop",
                        "store"
                        ]
                    }
                    ],
                    "params": [
                    "&0x1::event::EventHandle<T0>"
                    ],
                    "return": [
                    "&0x1::guid::GUID"
                    ]
                },
                {
                    "name": "counter",
                    "visibility": "public",
                    "is_entry": false,
                    "is_view": false,
                    "generic_type_params": [
                    {
                        "constraints": [
                        "drop",
                        "store"
                        ]
                    }
                    ],
                    "params": [
                    "&0x1::event::EventHandle<T0>"
                    ],
                    "return": [
                    "u64"
                    ]
                },
                {
                    "name": "destroy_handle",
                    "visibility": "public",
                    "is_entry": false,
                    "is_view": false,
                    "generic_type_params": [
                    {
                        "constraints": [
                        "drop",
                        "store"
                        ]
                    }
                    ],
                    "params": [
                    "0x1::event::EventHandle<T0>"
                    ],
                    "return": []
                },
                {
                    "name": "emit",
                    "visibility": "public",
                    "is_entry": false,
                    "is_view": false,
                    "generic_type_params": [
                    {
                        "constraints": [
                        "drop",
                        "store"
                        ]
                    }
                    ],
                    "params": [
                    "T0"
                    ],
                    "return": []
                },
                {
                    "name": "emit_event",
                    "visibility": "public",
                    "is_entry": false,
                    "is_view": false,
                    "generic_type_params": [
                    {
                        "constraints": [
                        "drop",
                        "store"
                        ]
                    }
                    ],
                    "params": [
                    "&mut 0x1::event::EventHandle<T0>",
                    "T0"
                    ],
                    "return": []
                },
                {
                    "name": "new_event_handle",
                    "visibility": "friend",
                    "is_entry": false,
                    "is_view": false,
                    "generic_type_params": [
                    {
                        "constraints": [
                        "drop",
                        "store"
                        ]
                    }
                    ],
                    "params": [
                    "0x1::guid::GUID"
                    ],
                    "return": [
                    "0x1::event::EventHandle<T0>"
                    ]
                }
                ],
                "structs": [
                {
                    "name": "EventHandle",
                    "is_native": false,
                    "is_event": false,
                    "is_enum": false,
                    "abilities": [
                    "store"
                    ],
                    "generic_type_params": [
                    {
                        "constraints": [
                        "drop",
                        "store"
                        ],
                        "is_phantom": true
                    }
                    ],
                    "fields": [
                    {
                        "name": "counter",
                        "type": "u64"
                    },
                    {
                        "name": "guid",
                        "type": "0x1::guid::GUID"
                    }
                    ]
                }
                ]
            }
            }
        "#).unwrap();

        let response = execute(request, &reader).await.unwrap();
        println!("{}", serde_json::to_string_pretty(&response).unwrap());

        assert_eq!(response, expected_response);
    }

    #[tokio::test]
    async fn test_non_existent_block_is_bad_input() {
        let (reader, _app) = create_app();

        let request = example_request(BlockNumberOrTag::Number(5));

        let response = execute(request, &reader).await;

        let expected_response = JsonRpcError::block_not_found(umi_shared::error::Error::User(
            umi_shared::error::UserError::InvalidBlockHeight(5),
        ));

        assert_eq!(response.unwrap_err(), expected_response);
    }

    #[tokio::test]
    async fn test_byte_code_is_kept_with_newly_built_block() {
        let (state_channel, rx) = mpsc::channel(10);
        let (reader, mut app) = create_app();
        let state: CommandActor<TestDependencies> = CommandActor::new(rx, &mut app);

        umi_app::run_with_actor(state, async move {
            let request = example_request(Latest);
            let response = execute(request, &reader).await.unwrap();

            let actual_bytecode = bytecode_from_response(response);
            let expected_bytecode = BYTE_CODE;

            assert_eq!(actual_bytecode, expected_bytecode);

            // Create a block, so the block height becomes 1
            let msg = Command::StartBlockBuild {
                payload_attributes: Default::default(),
                payload_id: U64::from(0x03421ee50df45cacu64),
            };
            send_command(&state_channel, msg).await;

            let request = example_request(Latest);
            let response = execute(request, &reader).await.unwrap();

            let actual_bytecode = bytecode_from_response(response);
            let expected_bytecode = BYTE_CODE;

            assert_eq!(actual_bytecode, expected_bytecode);
        })
        .await;
    }

    #[test_case(Safe; "safe")]
    #[test_case(Pending; "pending")]
    #[test_case(Finalized; "finalized")]
    #[tokio::test]
    async fn test_latest_block_byte_code_is_same_as_tag(tag: BlockNumberOrTag) {
        let (state_channel, rx) = mpsc::channel(10);
        let (reader, mut app) = create_app();
        let state: CommandActor<TestDependencies> = CommandActor::new(rx, &mut app);

        umi_app::run_with_actor(state, async move {
            let msg = Command::StartBlockBuild {
                payload_attributes: Default::default(),
                payload_id: U64::from(0x03421ee50df45cacu64),
            };
            send_command(&state_channel, msg).await;

            let request = example_request(tag);
            let response = execute(request, &reader).await.unwrap();

            let actual_bytecode = bytecode_from_response(response);
            let expected_bytecode = BYTE_CODE;

            assert_eq!(actual_bytecode, expected_bytecode);
        })
        .await;
    }
}
