use {
    crate::tests::test_context::{TestContext, DEPOSIT_TX},
    alloy::{hex, hex::FromHex},
    umi_api::schema::{
        BlobsBundleV1, ExecutionPayloadV3, ForkchoiceUpdatedResponseV1, GetPayloadResponseV3,
        PayloadStatusV1, Status,
    },
    umi_execution::U256,
    umi_shared::primitives::{Address, Bytes, B2048, B256, U64},
};

#[tokio::test]
async fn test_sending_the_same_payload_twice_produces_one_block() -> anyhow::Result<()> {
    TestContext::run(|ctx| async move {
        let block_hash = ctx.get_head();

        let request = serde_json::json!({
            "jsonrpc":"2.0",
            "id":10,
            "method":"engine_forkchoiceUpdatedV3",
            "params":[
                {
                    "headBlockHash": format!("{block_hash}"),
                    "safeBlockHash": format!("{block_hash}"),
                    "finalizedBlockHash": format!("{block_hash}")
                },
                {
                    "timestamp":"0x68235d0f",
                    "prevRandao":"0xdd9b0c0d88d7d9e5fe6718d97f5f2cfd9d825cf6265a39c08650de249e138339",
                    "suggestedFeeRecipient":"0x4200000000000000000000000000000000000011",
                    "withdrawals":[],
                    "parentBeaconBlockRoot":"0x0000000000000000000000000000000000000000000000000000000000000000",
                    "transactions":[ hex::encode(DEPOSIT_TX) ],
                    "eip1559Params": "0x0000000000000000",
                    "minBaseFee": 0,
                    "gasLimit":"0x1c9c380"
                }
            ]
        });

        let actual_response: ForkchoiceUpdatedResponseV1 = ctx.handle_request(&request)
            .await
            .unwrap();

        let payload_id = actual_response.payload_id.unwrap();
        let expected_response = ForkchoiceUpdatedResponseV1 {
            payload_status: PayloadStatusV1 {
                status: Status::Valid,
                latest_valid_hash: Some(block_hash),
                validation_error: None,
            },
            payload_id: Some(payload_id),
        };

        assert_eq!(actual_response, expected_response);

        let actual_response: ForkchoiceUpdatedResponseV1 = ctx.handle_request(&request)
            .await
            .unwrap();

        assert_eq!(actual_response, expected_response);

        ctx.queue.wait_for_pending_commands().await;

        let request = serde_json::json!({
            "jsonrpc":"2.0",
            "id":11,
            "method":"engine_getPayloadV3",
            "params":[
                format!("{payload_id}")
            ]
        });

        let actual_response: GetPayloadResponseV3 = ctx.handle_request(&request)
            .await
            .unwrap();

        let expected_response: GetPayloadResponseV3 = GetPayloadResponseV3 {
            execution_payload: ExecutionPayloadV3 {
                parent_hash: block_hash,
                fee_recipient: Address::new(hex!("4200000000000000000000000000000000000011")),
                state_root: B256::new(hex!("0482a97d8a072b8c8fdb2a0b6897f00b9f5445bc30a36d449df43da2564d3350")),
                receipts_root: B256::new(hex!("c496a834569d2ba3d4359ec588d41c58a176f6f6b39d1ea5aa1e2bb10736f297")),
                logs_bloom: B2048::new(hex!("00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000")),
                prev_randao: B256::new(hex!("dd9b0c0d88d7d9e5fe6718d97f5f2cfd9d825cf6265a39c08650de249e138339")),
                block_number: U64::from_limbs([1]),
                gas_limit: U64::from_limbs([30000000]),
                gas_used: U64::from_limbs([6690]),
                timestamp: U64::from_limbs([1747148047]),
                extra_data: Bytes::from_hex("0x01000000fa000000060000000000000000").unwrap(),
                base_fee_per_gas: U256::ZERO,
                block_hash: B256::new(hex!("0x7cbbfeb1e08534429e9faa6c5c5754693422c04fbe418cc30e5d9f4bb8ad8388")),
                transactions: vec![Bytes::from_iter(DEPOSIT_TX)],
                withdrawals: vec![],
                blob_gas_used: U64::ZERO,
                excess_blob_gas: U64::ZERO,
            },
            block_value: U256::ZERO,
            blobs_bundle: BlobsBundleV1 { commitments: vec![], proofs: vec![], blobs: vec![] },
            should_override_builder: false,
            parent_beacon_block_root: B256::ZERO,
        };

        assert_eq!(actual_response, expected_response);

        ctx.shutdown().await;
        Ok(())
    }).await
}
