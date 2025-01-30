use {crate::transaction::ExtendedTransaction, moved_shared::primitives::B256, std::fmt::Debug};

pub trait TransactionQueries {
    type Err: Debug;
    type Storage;

    fn by_hash(
        &self,
        storage: &Self::Storage,
        hash: B256,
    ) -> Result<Option<TransactionResponse>, Self::Err>;
}

pub type TransactionResponse = op_alloy::rpc_types::Transaction;

impl From<ExtendedTransaction> for TransactionResponse {
    fn from(value: ExtendedTransaction) -> Self {
        let (deposit_nonce, deposit_receipt_version) = value
            .deposit_nonce()
            .map(|nonce| (Some(nonce.nonce), Some(nonce.version)))
            .unwrap_or((None, None));

        Self {
            inner: alloy::rpc::types::eth::Transaction {
                from: value
                    .from()
                    .expect("Block transactions should contain valid signature"),
                inner: value.inner,
                block_hash: Some(value.block_hash),
                block_number: Some(value.block_number),
                transaction_index: Some(value.transaction_index),
                effective_gas_price: Some(value.effective_gas_price),
            },
            deposit_nonce,
            deposit_receipt_version,
        }
    }
}
