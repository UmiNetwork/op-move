use {
    moved::{
        block::{BlockRepository, ExtendedBlock},
        primitives::B256,
    },
    rocksdb::{AsColumnFamilyRef, DB as RocksDb},
};

pub const BLOCK_COLUMN_FAMILY: &str = "block";

#[derive(Debug)]
pub struct RocksDbBlockRepository;

impl RocksDbBlockRepository {
    pub fn cf(db: &RocksDb) -> impl AsColumnFamilyRef + use<'_> {
        db.cf_handle(BLOCK_COLUMN_FAMILY)
            .expect("Column family should exist")
    }
}

impl BlockRepository for RocksDbBlockRepository {
    type Storage = RocksDb;

    fn add(&mut self, storage: &mut Self::Storage, block: ExtendedBlock) {
        let cf = Self::cf(storage);
        let bytes = bcs::to_bytes(&block).unwrap();
        storage.put_cf(&cf, block.hash, bytes).unwrap()
    }

    fn by_hash(&self, storage: &Self::Storage, hash: B256) -> Option<ExtendedBlock> {
        let cf = Self::cf(storage);
        let bytes = storage.get_cf(&cf, hash).unwrap()?;
        bcs::from_bytes(bytes.as_slice()).ok()
    }
}
