use {
    crate::{
        block::block_cf,
        generic::{FromValue, ToKey},
    },
    moved_blockchain::{
        block::ExtendedBlock,
        payload::{PayloadId, PayloadQueries, PayloadResponse},
    },
    moved_shared::primitives::B256,
    rocksdb::{AsColumnFamilyRef, DB as RocksDb},
};

pub const COLUMN_FAMILY: &str = "payload";

impl ToKey for PayloadId {
    fn to_key(&self) -> impl AsRef<[u8]> {
        self.to_be_bytes::<8>()
    }
}

#[derive(Debug)]
pub struct RocksDbPayloadQueries {
    db: &'static RocksDb,
}

impl RocksDbPayloadQueries {
    pub fn new(db: &'static RocksDb) -> Self {
        Self { db }
    }

    pub fn add_block_hash(&self, id: PayloadId, block_hash: B256) -> Result<(), rocksdb::Error> {
        self.db.put_cf(&self.cf(), id.to_key(), block_hash)
    }

    fn cf(&self) -> impl AsColumnFamilyRef + use<'_> {
        cf(self.db)
    }
}

impl PayloadQueries for RocksDbPayloadQueries {
    type Err = rocksdb::Error;
    type Storage = &'static RocksDb;

    fn by_hash(
        &self,
        db: &Self::Storage,
        hash: B256,
    ) -> Result<Option<PayloadResponse>, Self::Err> {
        Ok(db
            .get_pinned_cf(&block_cf(db), hash)?
            .map(|bytes| ExtendedBlock::from_value(bytes.as_ref()))
            .map(PayloadResponse::from_block))
    }

    fn by_id(
        &self,
        db: &Self::Storage,
        id: PayloadId,
    ) -> Result<Option<PayloadResponse>, Self::Err> {
        db.get_pinned_cf(&cf(db), id.to_key())?
            .map(|hash| B256::new(hash.as_ref().try_into().unwrap()))
            .map(|hash| self.by_hash(db, hash))
            .unwrap_or(Ok(None))
    }
}

pub(crate) fn cf(db: &RocksDb) -> impl AsColumnFamilyRef + use<'_> {
    db.cf_handle(COLUMN_FAMILY)
        .expect("Column family should exist")
}
