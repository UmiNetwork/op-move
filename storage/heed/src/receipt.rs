use {
    crate::generic::{EncodableB256, SerdeJson},
    moved_blockchain::receipt::{
        ExtendedReceipt, ReceiptQueries, ReceiptRepository, TransactionReceipt,
    },
    moved_shared::primitives::B256,
};

pub type Db = heed::Database<EncodableB256, EncodableReceipt>;
pub type EncodableReceipt = SerdeJson<ExtendedReceipt>;

pub const DB: &str = "receipt";

#[derive(Debug)]
pub struct HeedReceiptRepository;

impl ReceiptRepository for HeedReceiptRepository {
    type Err = heed::Error;
    type Storage = &'static heed::Env;

    fn contains(&self, env: &Self::Storage, transaction_hash: B256) -> Result<bool, Self::Err> {
        let transaction = env.read_txn()?;

        let db: Db = env
            .open_database(&transaction, Some(DB))?
            .expect("Receipt database should exist");
        let db = db.lazily_decode_data();

        let response = db.get(&transaction, &transaction_hash).map(|v| v.is_some());

        transaction.commit()?;

        response
    }

    fn extend(
        &self,
        env: &mut Self::Storage,
        receipts: impl IntoIterator<Item = ExtendedReceipt>,
    ) -> Result<(), Self::Err> {
        let mut transaction = env.write_txn()?;

        let db: Db = env
            .open_database(&transaction, Some(DB))?
            .expect("Receipt database should exist");

        receipts.into_iter().try_for_each(|receipt| {
            db.put(&mut transaction, &receipt.transaction_hash, &receipt)
        })?;

        transaction.commit()
    }
}

#[derive(Debug)]
pub struct HeedReceiptQueries;

impl ReceiptQueries for HeedReceiptQueries {
    type Err = heed::Error;
    type Storage = &'static heed::Env;

    fn by_transaction_hash(
        &self,
        env: &Self::Storage,
        transaction_hash: B256,
    ) -> Result<Option<TransactionReceipt>, Self::Err> {
        let transaction = env.read_txn()?;

        let db: Db = env
            .open_database(&transaction, Some(DB))?
            .expect("Receipt database should exist");

        let response = db.get(&transaction, &transaction_hash);

        transaction.commit()?;

        Ok(response?.map(TransactionReceipt::from))
    }
}
