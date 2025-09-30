//! Checks our handling of invalid (i.e. malformed) transactions.
//! Such transactions cannot be executed and therefore should be dropped
//! without being included in any block.

use {
    crate::tests::test_context::TestContext,
    alloy::{
        consensus::{SignableTransaction, TxEip1559, TxEnvelope},
        network::TxSignerSync,
        primitives::{Address, TxKind, U256},
        signers::local::PrivateKeySigner,
    },
    umi_api::schema::Status,
};

const INVALID_PAYLOAD: &[u8] = b"a5cad08a000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000057465737431000000000000000000000000000000000000000000000000000000";

#[tokio::test]
async fn test_invalid_transaction() -> anyhow::Result<()> {
    TestContext::run(|mut ctx| async move {
        // Submit an invalid transaction
        let tx = create_invalid_tx(ctx.genesis_config.chain_id);
        let tx_hash = ctx.send_raw_transaction(tx).await?;

        // Produce a new block
        let update = ctx.engine_forkchoice_update().await?;
        ctx.queue.wait_for_pending_commands().await;
        let payload = ctx.engine_get_payload(update.payload_id.unwrap()).await?;
        let block = ctx
            .get_block_by_number(payload.execution_payload.block_number.saturating_to())
            .await?;

        // Confirm payload is accepted
        let new_payload_response = ctx.engine_new_payload(payload.execution_payload).await?;
        assert_eq!(new_payload_response.status, Status::Valid);

        // Confirm transaction has no receipt.
        let receipt = ctx.get_transaction_receipt(tx_hash).await?;
        assert_eq!(receipt, None);

        // Confirm transaction is not included in the block.
        let tx_count_in_block = block
            .0
            .transactions
            .hashes()
            .filter(|x| x == &tx_hash)
            .count();
        assert_eq!(tx_count_in_block, 0);

        ctx.shutdown().await;
        Ok(())
    })
    .await
}

fn create_invalid_tx(chain_id: u64) -> TxEnvelope {
    let mut tx = TxEip1559 {
        chain_id,
        nonce: 0,
        gas_limit: u64::MAX,
        max_fee_per_gas: 0,
        max_priority_fee_per_gas: 0,
        to: TxKind::Call(Address::ZERO),
        value: U256::ZERO,
        access_list: Default::default(),
        input: INVALID_PAYLOAD.into(),
    };
    let signer = PrivateKeySigner::random();
    let signature = signer.sign_transaction_sync(&mut tx).unwrap();
    TxEnvelope::Eip1559(tx.into_signed(signature))
}
