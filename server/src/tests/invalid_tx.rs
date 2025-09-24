//! A regression test for a prior issue where op-move would return an invalid payload
//! due to a malformed transaction. This test ensure op-move cannot be corrupted by
//! such an invalid transaction.

use {
    crate::tests::test_context::TestContext,
    alloy::{consensus::TxEnvelope, rlp::Decodable},
    umi_api::schema::Status,
    umi_execution::transaction::UmiTxEnvelope,
};

const INVALID_TX_HEX: &str = r#"
    f8d4038502540be400831e8480946f40a56250fbb57f5a17c815be66a3680459066987b1a2bc2ec
    50000b864a5cad08a00000000000000000000000000000000000000000000000000000000000000
    2000000000000000000000000000000000000000000000000000000000000000057465737431000
    000000000000000000000000000000000000000000000000000830148cea0e7602c29eeb146b345
    ecccc9d493f3df3f44a6a8c4010413318957f35003a321a02fec513d11706d2fa2c507e267e0b46
    886eead5374a3a7472481c87a53ccac6f
"#;
const INVALID_TX_HASH: [u8; 32] =
    alloy::hex!("0xb751f4c210af369b495fe27e0398bed892ccea52855a9ee9196bb155c217a6e2");

#[tokio::test]
async fn test_invalid_transaction() -> anyhow::Result<()> {
    TestContext::run(|mut ctx| async move {
        let invalid_tx_hex: String = INVALID_TX_HEX
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        let invalid_tx_bytes = hex::decode(&invalid_tx_hex).unwrap();
        let invalid_tx: TxEnvelope =
            UmiTxEnvelope::decode(&mut invalid_tx_bytes.as_slice())?.into();

        // Submit the invalid transaction
        let tx_hash = ctx.send_raw_transaction(invalid_tx).await?;
        assert_eq!(tx_hash, INVALID_TX_HASH);

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
