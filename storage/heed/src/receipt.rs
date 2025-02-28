use {
    crate::generic::EncodableB256,
    heed::types::{LazyDecode, SerdeBincode},
    moved_blockchain::receipt::{
        ExtendedReceipt, ReceiptQueries, ReceiptRepository, TransactionReceipt,
    },
    moved_shared::primitives::B256,
};

pub type EncodableReceipt = SerdeBincode<ExtendedReceipt>;

pub const RECEIPT_DB: &str = "receipt";

#[derive(Debug)]
pub struct HeedReceiptRepository;

impl ReceiptRepository for HeedReceiptRepository {
    type Err = heed::Error;
    type Storage = &'static heed::Env;

    fn contains(&self, env: &Self::Storage, transaction_hash: B256) -> Result<bool, Self::Err> {
        let transaction = env.read_txn()?;

        let db: heed::Database<EncodableB256, LazyDecode<EncodableReceipt>> = env
            .open_database(&transaction, Some(RECEIPT_DB))?
            .expect("Receipt database should exist")
            .lazily_decode_data();

        db.get(&transaction, &transaction_hash).map(|v| v.is_some())
    }

    fn extend(
        &self,
        env: &mut Self::Storage,
        receipts: impl IntoIterator<Item = ExtendedReceipt>,
    ) -> Result<(), Self::Err> {
        let mut transaction = env.write_txn()?;

        let db: heed::Database<EncodableB256, EncodableReceipt> = env
            .open_database(&transaction, Some(RECEIPT_DB))?
            .expect("Receipt database should exist");

        receipts
            .into_iter()
            .try_for_each(|receipt| db.put(&mut transaction, &receipt.transaction_hash, &receipt))
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

        let db: heed::Database<EncodableB256, EncodableReceipt> = env
            .open_database(&transaction, Some(RECEIPT_DB))?
            .expect("Receipt database should exist");

        Ok(db
            .get(&transaction, &transaction_hash)?
            .map(TransactionReceipt::from))
    }
}
