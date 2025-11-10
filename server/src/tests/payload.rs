use {
    crate::tests::test_context::{TestContext, DEPOSIT_TX},
    alloy::hex,
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
                parent_hash: B256::new(hex!("14d3698a2c6b14767ab3707f4c07586d72e6db14e523e8701e667908d39e06c2")),
                fee_recipient: Address::new(hex!("4200000000000000000000000000000000000011")),
                state_root: B256::new(hex!("4a9f5effd03badf1fd514c50a7fb49ee7639a48110e5f09d5a345db3614ec92a")),
                #[cfg(not(feature = "op-upgrade"))]
                receipts_root: B256::new(hex!("22be36dff53ef8da81f4e83975db185d076920d1773031d748cca48d193cbf24")),
                #[cfg(feature = "op-upgrade")]
                receipts_root: B256::new(hex!("efb37db18e32368e95ad9c735b57e1d4bbab95111d559214daa0fc4259b3c6f9")),
                logs_bloom: B2048::new(hex!("00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000")),
                prev_randao: B256::new(hex!("dd9b0c0d88d7d9e5fe6718d97f5f2cfd9d825cf6265a39c08650de249e138339")),
                block_number: U64::from_limbs([1]),
                gas_limit: U64::from_limbs([30000000]),
                #[cfg(not(feature = "op-upgrade"))]
                gas_used: U64::from_limbs([119461]),
                #[cfg(feature = "op-upgrade")]
                gas_used: U64::from_limbs([119509]),
                timestamp: U64::from_limbs([1747148047]),
                #[cfg(not(feature = "op-upgrade"))]
                extra_data: Bytes::new(),
                #[cfg(feature = "op-upgrade")]
                extra_data: Bytes::from_iter(std::iter::repeat_n(0, 9)),
                base_fee_per_gas: U256::ZERO,
                #[cfg(not(feature = "op-upgrade"))]
                block_hash: B256::new(hex!("697183dc6d590291df6afed449c722b54501edbc3ccea1f3098215bab18d085e")),
                #[cfg(feature = "op-upgrade")]
                block_hash: B256::new(hex!("db21af46037ccc77a155c29288849c1e11ce935b4ea94da766d2c1bd34f90fe1")),
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
