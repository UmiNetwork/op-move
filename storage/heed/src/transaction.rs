use {
    crate::generic::EncodableB256,
    heed::types::SerdeBincode,
    moved_blockchain::transaction::{
        ExtendedTransaction, TransactionQueries, TransactionRepository, TransactionResponse,
    },
    moved_shared::primitives::B256,
};

pub type EncodableTransaction = SerdeBincode<ExtendedTransaction>;

pub const TRANSACTION_DB: &str = "transaction";

#[derive(Debug)]
pub struct HeedTransactionRepository;

impl TransactionRepository for HeedTransactionRepository {
    type Err = heed::Error;
    type Storage = &'static heed::Env;

    fn extend(
        &mut self,
        env: &mut Self::Storage,
        transactions: impl IntoIterator<Item = ExtendedTransaction>,
    ) -> Result<(), Self::Err> {
        let mut db_transaction = env.write_txn()?;

        let db: heed::Database<EncodableB256, EncodableTransaction> = env
            .open_database(&db_transaction, Some(TRANSACTION_DB))?
            .expect("Transaction database should exist");

        transactions.into_iter().try_for_each(|transaction| {
            db.put(&mut db_transaction, &transaction.hash(), &transaction)
        })
    }
}

#[derive(Debug)]
pub struct HeedTransactionQueries;

impl TransactionQueries for HeedTransactionQueries {
    type Err = heed::Error;
    type Storage = &'static heed::Env;

    fn by_hash(
        &self,
        env: &Self::Storage,
        hash: B256,
    ) -> Result<Option<TransactionResponse>, Self::Err> {
        let transaction = env.read_txn()?;

        let db: heed::Database<EncodableB256, EncodableTransaction> = env
            .open_database(&transaction, Some(TRANSACTION_DB))?
            .expect("Transaction database should exist");

        Ok(db.get(&transaction, &hash)?.map(TransactionResponse::from))
    }
}
