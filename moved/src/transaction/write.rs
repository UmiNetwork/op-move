use {
    alloy::eips::eip2718::Encodable2718, moved_shared::primitives::B256,
    op_alloy::consensus::OpTxEnvelope, std::fmt::Debug,
};

#[derive(Debug, Clone)]
pub struct ExtendedTransaction {
    pub effective_gas_price: u128,
    pub inner: OpTxEnvelope,
    pub block_number: u64,
    pub block_hash: B256,
    pub transaction_index: u64,
}

impl ExtendedTransaction {
    pub fn new(
        effective_gas_price: u128,
        inner: OpTxEnvelope,
        block_number: u64,
        block_hash: B256,
        transaction_index: u64,
    ) -> Self {
        Self {
            effective_gas_price,
            inner,
            block_number,
            block_hash,
            transaction_index,
        }
    }

    pub fn inner(&self) -> &OpTxEnvelope {
        &self.inner
    }

    pub fn hash(&self) -> B256 {
        self.inner.trie_hash()
    }
}

pub trait TransactionRepository {
    type Err: Debug;
    type Storage;

    fn extend(
        &mut self,
        storage: &mut Self::Storage,
        transactions: impl IntoIterator<Item = ExtendedTransaction>,
    ) -> Result<(), Self::Err>;
}
