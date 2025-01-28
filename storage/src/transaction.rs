use {
    moved::{
        transaction::{ExtendedTransaction, TransactionQueries, TransactionRepository},
        types::state::TransactionResponse,
    },
    moved_shared::primitives::B256,
    rocksdb::{AsColumnFamilyRef, WriteBatchWithTransaction, DB as RocksDb},
};

pub const TRANSACTION_COLUMN_FAMILY: &str = "transaction";

#[derive(Debug)]
pub struct RocksDbTransactionRepository;

impl TransactionRepository for RocksDbTransactionRepository {
    type Err = rocksdb::Error;
    type Storage = RocksDb;

    fn extend(
        &mut self,
        db: &mut Self::Storage,
        transactions: impl IntoIterator<Item = ExtendedTransaction>,
    ) -> Result<(), Self::Err> {
        let cf = transaction_cf(db);
        let mut batch = WriteBatchWithTransaction::<false>::default();

        for transaction in transactions {
            let bytes = bcs::to_bytes(&transaction).unwrap();
            batch.put_cf(&cf, transaction.hash(), bytes);
        }

        db.write(batch)
    }
}

#[derive(Debug)]
pub struct RocksDbTransactionQueries;

impl TransactionQueries for RocksDbTransactionQueries {
    type Err = rocksdb::Error;
    type Storage = RocksDb;

    fn by_hash(
        &self,
        db: &Self::Storage,
        hash: B256,
    ) -> Result<Option<TransactionResponse>, Self::Err> {
        let cf = transaction_cf(db);

        Ok(db
            .get_cf(&cf, hash)?
            .and_then(|v| bcs::from_bytes(v.as_slice()).ok()))
    }
}

fn transaction_cf(db: &RocksDb) -> impl AsColumnFamilyRef + use<'_> {
    db.cf_handle(TRANSACTION_COLUMN_FAMILY)
        .expect("Column family should exist")
}
