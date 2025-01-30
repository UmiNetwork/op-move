use {moved_shared::primitives::B256, std::fmt::Debug};

pub trait ReceiptQueries {
    type Err: Debug;
    type Storage;

    fn by_transaction_hash(
        &self,
        storage: &Self::Storage,
        transaction_hash: B256,
    ) -> Result<Option<TransactionReceipt>, Self::Err>;
}

pub type TransactionReceipt = op_alloy::rpc_types::OpTransactionReceipt;
