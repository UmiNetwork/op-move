use {crate::block::BlockQueries, moved_shared::primitives::B256, std::fmt::Debug};

pub trait ReceiptQueries {
    type Err: Debug;
    type Storage;

    fn by_transaction_hash<B: BlockQueries>(
        &self,
        storage: &Self::Storage,
        block_queries: B,
        block_storage: &B::Storage,
        transaction_hash: B256,
    ) -> Result<Option<TransactionReceipt>, Self::Err>
    where
        Self::Err: From<B::Err>;
}

pub type TransactionReceipt = op_alloy::rpc_types::OpTransactionReceipt;
