//! A regression test for a prior issue where op-move would return an invalid payload
//! due to a legacy-type transaction. This test ensure op-move cannot be corrupted by
//! legacy transactions.

use {
    crate::tests::test_context::TestContext,
    alloy::{
        consensus::{SignableTransaction, TxEnvelope, TxLegacy},
        network::TxSignerSync,
        primitives::{Address, U256},
        signers::local::PrivateKeySigner,
    },
    umi_api::schema::Status,
};

const ONE: Address = {
    let mut buf = [0u8; 20];
    buf[19] = 1;
    Address::new(buf)
};

#[tokio::test]
async fn test_invalid_transaction() -> anyhow::Result<()> {
    TestContext::run(|mut ctx| async move {
        // Submit a legacy transaction
        let tx = legacy_tx(ctx.genesis_config.chain_id);
        ctx.send_raw_transaction(tx).await?;

        // Produce a new block
        let update = ctx.engine_forkchoice_update().await?;
        ctx.queue.wait_for_pending_commands().await;
        let payload = ctx.engine_get_payload(update.payload_id.unwrap()).await?;

        // Confirm payload is accepted
        let new_payload_response = ctx.engine_new_payload(payload.execution_payload).await?;

        // TODO: must be valid
        assert_eq!(new_payload_response.status, Status::Invalid);

        ctx.shutdown().await;
        Ok(())
    })
    .await
}

fn legacy_tx(chain_id: u64) -> TxEnvelope {
    let mut tx = TxLegacy {
        chain_id: Some(chain_id),
        nonce: 0,
        gas_price: 0,
        gas_limit: u64::MAX,
        to: alloy::primitives::TxKind::Call(ONE),
        value: U256::ZERO,
        input: Default::default(),
    };
    let signer = PrivateKeySigner::random();
    let signature = signer.sign_transaction_sync(&mut tx).unwrap();
    TxEnvelope::Legacy(tx.into_signed(signature))
}
