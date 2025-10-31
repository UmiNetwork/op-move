use {
    crate::RocksDb,
    rocksdb::{AsColumnFamilyRef, WriteBatchWithTransaction},
    std::sync::Arc,
    umi_blockchain::state::HashToStateRootIndex,
    umi_shared::primitives::B256,
};

pub const COLUMN_FAMILY: &str = "state";

#[derive(Debug, Clone)]
pub struct RocksDbStateRootIndex {
    db: Arc<RocksDb>,
}

impl RocksDbStateRootIndex {
    pub const fn new(db: Arc<RocksDb>) -> Self {
        Self { db }
    }
}

impl HashToStateRootIndex for RocksDbStateRootIndex {
    type Err = rocksdb::Error;

    fn root_by_hash(&self, hash: B256) -> Result<Option<B256>, Self::Err> {
        Ok(self
            .db
            .get_pinned_cf(&self.cf(), hash)?
            .map(|v| B256::from_slice(v.as_ref())))
    }

    fn push_state_root(&self, block_hash: B256, state_root: B256) -> Result<(), Self::Err> {
        let mut batch = WriteBatchWithTransaction::<false>::default();

        batch.put_cf(&self.cf(), block_hash, state_root);

        self.db.write(batch)
    }
}

impl RocksDbStateRootIndex {
    fn cf(&self) -> impl AsColumnFamilyRef + use<'_> {
        self.db
            .cf_handle(COLUMN_FAMILY)
            .expect("Column family should exist")
    }
}
