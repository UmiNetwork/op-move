//! Tests to make sure we can handle building blocks on different forks.

use {
    crate::tests::test_context::TestContext,
    alloy::{
        consensus::{SignableTransaction, TxEip1559, TxEnvelope},
        network::TxSignerSync,
        primitives::{Address, U256},
        signers::local::PrivateKeySigner,
    },
};

#[tokio::test]
async fn test_forks() -> anyhow::Result<()> {
    TestContext::run(|mut ctx| async move {
        // Build a chain of 4 blocks
        for _ in 0..4 {
            ctx.produce_block().await?;
        }

        // Build a 5th block and remember its hash
        let block_5 = ctx.produce_block().await?;

        // Build a 6th block and remember its hash
        let block_6 = ctx.produce_block().await?;

        // Suppose block 6 is finalized
        ctx.advance_finalized_block(block_6).await?;

        // Continue the chain for a few more blocks
        for _ in 0..3 {
            ctx.produce_block().await?;
        }

        // Execute a transaction
        let tx = create_tx(ctx.genesis_config.chain_id);
        let receipt_fork_1 = ctx.execute_transaction(tx.clone()).await?;

        // Change the forkchoice back to block 6
        ctx.update_head_block(block_6).await?;

        // Since we are building on a new fork, we can submit the same transaction again
        // and put it into a different block.
        let receipt_fork_2 = ctx.execute_transaction(tx).await?;

        assert_eq!(receipt_fork_1.inner.block_number, Some(10));
        assert_eq!(receipt_fork_2.inner.block_number, Some(7));
        assert_eq!(ctx.get_latest_block_number().await.unwrap(), 7);

        // It is forbidden to change the finalized block to number 5
        // because it is earlier than the current last finalized block
        ctx.advance_finalized_block(block_5).await.unwrap_err();

        ctx.shutdown().await;
        Ok(())
    })
    .await
}

fn create_tx(chain_id: u64) -> TxEnvelope {
    let mut tx = TxEip1559 {
        chain_id,
        nonce: 0,
        max_fee_per_gas: 0,
        max_priority_fee_per_gas: 0,
        gas_limit: u64::MAX,
        to: alloy::primitives::TxKind::Call(Address::ZERO),
        value: U256::ZERO,
        input: Default::default(),
        access_list: Default::default(),
    };
    let signer = PrivateKeySigner::random();
    let signature = signer.sign_transaction_sync(&mut tx).unwrap();
    TxEnvelope::Eip1559(tx.into_signed(signature))
}
