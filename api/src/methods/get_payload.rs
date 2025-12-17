use {
    crate::{
        json_utils::parse_params_1,
        jsonrpc::JsonRpcError,
        schema::{GetPayloadResponseV3, PayloadId},
    },
    umi_app::{ApplicationReader, Dependencies},
    umi_blockchain::payload::MaybePayloadResponse,
};

pub async fn execute_v3<'reader>(
    request: serde_json::Value,
    app: &ApplicationReader<'reader, impl Dependencies<'reader>>,
) -> Result<serde_json::Value, JsonRpcError> {
    let payload_id: PayloadId = parse_params_1(request)?;

    // Spec: https://github.com/ethereum/execution-apis/blob/main/src/engine/cancun.md#specification-2
    let response = match app.payload(payload_id.into())? {
        MaybePayloadResponse::Some(response) => *response,
        MaybePayloadResponse::Delayed(mut rx) => {
            if let Ok(response) = rx.recv().await {
                response
            } else {
                return Err(JsonRpcError::unknown_payload(payload_id));
            }
        }
        MaybePayloadResponse::Unknown => {
            return Err(JsonRpcError::unknown_payload(payload_id));
        }
    };

    Ok(serde_json::to_value(GetPayloadResponseV3::from(response))
        .expect("Must be able to JSON-serialize response"))
}

#[cfg(test)]
mod tests {
    use {super::*, crate::methods::forkchoice_updated};

    #[test]
    fn test_parse_params_v3() {
        let request: serde_json::Value = serde_json::from_str(
            r#"
            {
                "id": 30054,
                "jsonrpc": "2.0",
                "method": "engine_getPayloadV3",
                "params": [
                    "0x03421ee50df45cac"
                ]
            }
        "#,
        )
        .unwrap();

        let params: PayloadId = parse_params_1(request).unwrap();

        let expected_params = PayloadId::from(0x03421ee50df45cacu64);

        assert_eq!(params, expected_params);
    }

    #[tokio::test]
    async fn test_execute_v3() {
        let (reader, mut app) = crate::methods::tests::create_app();
        let (queue, state) = umi_app::create(&mut app, 10);

        umi_app::run_with_actor(state, async move {
            // Update the state with an execution payload
            forkchoice_updated::execute_v3(
                forkchoice_updated::tests::example_request(),
                queue.clone(),
                &reader,
                &0x03421ee50df45cacu64,
            )
                .await
                .unwrap();

            let request: serde_json::Value = serde_json::from_str(
                r#"
                {
                    "id": 30054,
                    "jsonrpc": "2.0",
                    "method": "engine_getPayloadV3",
                    "params": [
                        "0x03421ee50df45cac"
                    ]
                }
            "#,
            )
                .unwrap();

            let expected_response: serde_json::Value = serde_json::from_str(r#"
                {
                    "executionPayload": {
                        "parentHash": "0xe56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d",
                        "feeRecipient": "0x4200000000000000000000000000000000000011",
                        "stateRoot": "0xaf820d7c50ef94dcc289cbe7fdc6965865d94ae15e0c96d543b217149417ae48",
                        "receiptsRoot": "0xac250b3f8a3b68fa0f3e051f2aff11c942e66d7464c261842502859ff2c5f7f6",
                        "logsBloom": "0x00000000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002000000000000000000008000000000000000000000000000000000000000000008000000000000000000000000000000000400000000000001000000000000000000000200000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000400",
                        "prevRandao": "0xbde07f5d381bb84700433fe6c0ae077aa40eaad3a5de7abd298f0e3e27e6e4c9",
                        "blockNumber": "0x1",
                        "gasLimit": "0x1c9c380",
                        "gasUsed": "0x2554f",
                        "timestamp": "0x6660737b",
                        "extraData": "0x01000000fa000000060000000000000000",
                        "baseFeePerGas": "0x0",
                        "blockHash": "0xad21ca5fc03aa0b1db32bd02f5e9b2c50a3dacfbaf80f71a1411cf334c36faa7",
                        "transactions": [
                        "0x7ef8f8a0de86bef815fc910df65a9459ccb2b9a35fa8596dfcfed1ff01bbf28891d86d5e94deaddeaddeaddeaddeaddeaddeaddeaddead00019442000000000000000000000000000000000000158080830f424080b8a4440a5e2000000558000c5fc50000000000000000000000006660735b00000000000001a9000000000000000000000000000000000000000000000000000000000000000700000000000000000000000000000000000000000000000000000000000000017ae3f74f0134521a7d62a387ac75a5153bcd1aab1c7e003e9b9e15a5d8846363000000000000000000000000e25583099ba105d9ec0a67f5ae86d90e50036425",
                        "0x7ef858a000000000000000000000000000000000000000000000000000000000000000009488f9b82462f6c4bf4a0fb15e5c3971559a316e7f9488f9b82462f6c4bf4a0fb15e5c3971559a316e7f7b7b88ffffffffffffffff8080"
                        ],
                        "withdrawals": [],
                        "blobGasUsed": "0x0",
                        "excessBlobGas": "0x0"
                    },
                    "blockValue": "0x0",
                    "blobsBundle": {
                        "commitments": [],
                        "proofs": [],
                        "blobs": []
                    },
                    "shouldOverrideBuilder": false,
                    "parentBeaconBlockRoot": "0x2bd857e239f7e5b5e6415608c76b90600d51fa0f7f0bbbc04e2d6861b3186f1c"
                }
            "#).unwrap();

            queue.wait_for_pending_commands().await;

            let actual_response = execute_v3(request, &reader).await.unwrap();

            assert_eq!(actual_response, expected_response);
        }).await;
    }
}
