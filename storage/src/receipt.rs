use {
    moved::receipt::{ExtendedReceipt, ReceiptQueries, ReceiptRepository, TransactionReceipt},
    moved_shared::primitives::B256,
    rocksdb::{AsColumnFamilyRef, WriteBatchWithTransaction, DB as RocksDb},
};

pub const COLUMN_FAMILY: &str = "receipt";

#[derive(Debug)]
pub struct RocksDbReceiptRepository;

impl ReceiptRepository for RocksDbReceiptRepository {
    type Err = rocksdb::Error;
    type Storage = RocksDb;

    fn contains(&self, db: &Self::Storage, transaction_hash: B256) -> Result<bool, Self::Err> {
        let cf = cf(db);
        db.get_cf(&cf, transaction_hash)
            .map(|v: Option<Vec<u8>>| v.is_some())
    }

    fn extend(
        &self,
        db: &mut Self::Storage,
        receipts: impl IntoIterator<Item = ExtendedReceipt>,
    ) -> Result<(), Self::Err> {
        let cf = cf(db);
        let mut batch = WriteBatchWithTransaction::<false>::default();

        for receipt in receipts {
            let bytes = bcs::to_bytes(&receipt).unwrap();
            batch.put_cf(&cf, receipt.transaction_hash, bytes);
        }

        db.write(batch)
    }
}

#[derive(Debug)]
pub struct RocksDbReceiptQueries;

impl ReceiptQueries for RocksDbReceiptQueries {
    type Err = rocksdb::Error;
    type Storage = RocksDb;

    fn by_transaction_hash(
        &self,
        db: &Self::Storage,
        transaction_hash: B256,
    ) -> Result<Option<TransactionReceipt>, Self::Err> {
        let cf = cf(db);

        Ok(db
            .get_cf(&cf, transaction_hash)?
            .and_then(|v| bcs::from_bytes(v.as_slice()).ok()))
    }
}

fn cf(db: &RocksDb) -> impl AsColumnFamilyRef + use<'_> {
    db.cf_handle(COLUMN_FAMILY)
        .expect("Column family should exist")
}
