use {crate::types::state::TransactionResponse, moved_shared::primitives::B256, std::fmt::Debug};

pub trait TransactionQueries {
    type Err: Debug;
    type Storage;

    fn by_hash(
        &self,
        storage: &Self::Storage,
        hash: B256,
    ) -> Result<Option<TransactionResponse>, Self::Err>;
}
