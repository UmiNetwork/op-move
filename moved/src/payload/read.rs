use {
    crate::types::state::{PayloadId, PayloadResponse},
    moved_shared::primitives::B256,
    std::fmt::Debug,
};

pub trait PayloadQueries {
    type Err: Debug;
    type Storage;

    fn by_hash(
        &self,
        storage: &Self::Storage,
        block_hash: B256,
    ) -> Result<Option<PayloadResponse>, Self::Err>;

    fn by_id(
        &self,
        storage: &Self::Storage,
        id: PayloadId,
    ) -> Result<Option<PayloadResponse>, Self::Err>;
}
