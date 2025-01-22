use {
    crate::generic::ToKey,
    moved::{
        block::{BlockQueries, BlockRepository, ExtendedBlock},
        types::state::BlockResponse,
    },
    moved_primitives::B256,
    rocksdb::{AsColumnFamilyRef, DB as RocksDb},
};

pub const BLOCK_COLUMN_FAMILY: &str = "block";
pub const HEIGHT_COLUMN_FAMILY: &str = "height";

#[derive(Debug)]
pub struct RocksDbBlockRepository;

impl BlockRepository for RocksDbBlockRepository {
    type Storage = RocksDb;

    fn add(&mut self, db: &mut Self::Storage, block: ExtendedBlock) {
        let cf = block_cf(db);
        let bytes = bcs::to_bytes(&block).unwrap();
        db.put_cf(&cf, block.hash, bytes).unwrap();
        let cf = height_cf(db);
        db.put_cf(&cf, block.block.header.number.to_key(), block.hash)
            .unwrap();
    }

    fn by_hash(&self, db: &Self::Storage, hash: B256) -> Option<ExtendedBlock> {
        let cf = block_cf(db);
        let bytes = db.get_cf(&cf, hash).unwrap()?;
        bcs::from_bytes(bytes.as_slice()).ok()
    }
}

#[derive(Debug)]
pub struct RocksDbBlockQueries;

impl BlockQueries for RocksDbBlockQueries {
    type Err = rocksdb::Error;
    type Storage = RocksDb;

    fn by_hash(
        &self,
        db: &Self::Storage,
        hash: B256,
        include_transactions: bool,
    ) -> Result<Option<BlockResponse>, Self::Err> {
        let cf = block_cf(db);

        Ok(db
            .get_cf(&cf, hash)?
            .and_then(|v| bcs::from_bytes(v.as_slice()).ok())
            .map(if include_transactions {
                BlockResponse::from_block_with_transactions
            } else {
                BlockResponse::from_block_with_transaction_hashes
            }))
    }

    fn by_height(
        &self,
        db: &Self::Storage,
        height: u64,
        include_transactions: bool,
    ) -> Result<Option<BlockResponse>, Self::Err> {
        let cf = height_cf(db);

        db.get_cf(&cf, height.to_key())?
            .map(|hash| B256::from_slice(hash.as_slice()))
            .map(|hash| self.by_hash(db, hash, include_transactions))
            .unwrap_or(Ok(None))
    }
}

fn block_cf(db: &RocksDb) -> impl AsColumnFamilyRef + use<'_> {
    db.cf_handle(BLOCK_COLUMN_FAMILY)
        .expect("Column family should exist")
}

fn height_cf(db: &RocksDb) -> impl AsColumnFamilyRef + use<'_> {
    db.cf_handle(HEIGHT_COLUMN_FAMILY)
        .expect("Column family should exist")
}
