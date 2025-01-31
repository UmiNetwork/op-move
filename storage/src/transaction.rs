use {
    crate::generic::{FromValue, ToValue},
    moved::transaction::{
        ExtendedTransaction, TransactionQueries, TransactionRepository, TransactionResponse,
    },
    moved_shared::primitives::B256,
    rocksdb::{AsColumnFamilyRef, WriteBatchWithTransaction, DB as RocksDb},
};

pub const COLUMN_FAMILY: &str = "transaction";

#[derive(Debug)]
pub struct RocksDbTransactionRepository;

impl TransactionRepository for RocksDbTransactionRepository {
    type Err = rocksdb::Error;
    type Storage = &'static RocksDb;

    fn extend(
        &mut self,
        db: &mut Self::Storage,
        transactions: impl IntoIterator<Item = ExtendedTransaction>,
    ) -> Result<(), Self::Err> {
        let cf = cf(db);
        let mut batch = WriteBatchWithTransaction::<false>::default();

        for transaction in transactions {
            let bytes = transaction.to_value();
            batch.put_cf(&cf, transaction.hash(), bytes);
        }

        db.write(batch)
    }
}

#[derive(Debug)]
pub struct RocksDbTransactionQueries;

impl TransactionQueries for RocksDbTransactionQueries {
    type Err = rocksdb::Error;
    type Storage = &'static RocksDb;

    fn by_hash(
        &self,
        db: &Self::Storage,
        hash: B256,
    ) -> Result<Option<TransactionResponse>, Self::Err> {
        let cf = cf(db);

        Ok(db
            .get_pinned_cf(&cf, hash)?
            .and_then(|v| FromValue::from_value(v.as_ref())))
    }
}

pub(crate) fn cf(db: &RocksDb) -> impl AsColumnFamilyRef + use<'_> {
    db.cf_handle(COLUMN_FAMILY)
        .expect("Column family should exist")
}
