use {
    crate::{
        generic::{EncodableB256, EncodableU64, SerdeJson},
        transaction,
    },
    moved_blockchain::block::{BlockQueries, BlockRepository, BlockResponse, ExtendedBlock},
    moved_shared::primitives::B256,
};

pub type Db = heed::Database<EncodableB256, EncodableBlock>;
pub type HeightDb = heed::Database<EncodableU64, EncodableB256>;
pub type EncodableBlock = SerdeJson<ExtendedBlock>;

pub const DB: &str = "block";
pub const HEIGHT_DB: &str = "height";

#[derive(Debug)]
pub struct HeedBlockRepository;

impl BlockRepository for HeedBlockRepository {
    type Err = heed::Error;
    type Storage = &'static heed::Env;

    fn add(&mut self, env: &mut Self::Storage, block: ExtendedBlock) -> Result<(), Self::Err> {
        let mut transaction = env.write_txn()?;

        let db: Db = env
            .open_database(&transaction, Some(DB))?
            .expect("Block database should exist");

        db.put(&mut transaction, &block.hash, &block)?;

        let db: HeightDb = env
            .open_database(&transaction, Some(HEIGHT_DB))?
            .expect("Block height database should exist");

        db.put(&mut transaction, &block.block.header.number, &block.hash)?;

        transaction.commit()
    }

    fn by_hash(&self, env: &Self::Storage, hash: B256) -> Result<Option<ExtendedBlock>, Self::Err> {
        let transaction = env.read_txn()?;

        let db: Db = env
            .open_database(&transaction, Some(DB))?
            .expect("Block database should exist");

        let response = db.get(&transaction, &hash);

        transaction.commit()?;

        response
    }
}

#[derive(Debug)]
pub struct HeedBlockQueries;

impl BlockQueries for HeedBlockQueries {
    type Err = heed::Error;
    type Storage = &'static heed::Env;

    fn by_hash(
        &self,
        env: &Self::Storage,
        hash: B256,
        include_transactions: bool,
    ) -> Result<Option<BlockResponse>, Self::Err> {
        let db_transaction = env.read_txn()?;

        let db: Db = env
            .open_database(&db_transaction, Some(DB))?
            .expect("Block database should exist");

        let block = db.get(&db_transaction, &hash)?;

        Ok(Some(match block {
            Some(block) if include_transactions => {
                let db: transaction::Db = env
                    .open_database(&db_transaction, Some(transaction::DB))?
                    .expect("Transaction database should exist");

                let transactions = block
                    .transaction_hashes()
                    .filter_map(|hash| db.get(&db_transaction, &hash).transpose())
                    .collect::<Result<Vec<_>, _>>()?;

                db_transaction.commit()?;

                BlockResponse::from_block_with_transactions(block, transactions)
            }
            Some(block) => {
                db_transaction.commit()?;

                BlockResponse::from_block_with_transaction_hashes(block)
            }
            None => {
                db_transaction.commit()?;

                return Ok(None);
            }
        }))
    }

    fn by_height(
        &self,
        env: &Self::Storage,
        height: u64,
        include_transactions: bool,
    ) -> Result<Option<BlockResponse>, Self::Err> {
        let transaction = env.read_txn()?;

        let db: HeightDb = env
            .open_database(&transaction, Some(HEIGHT_DB))?
            .expect("Block height database should exist");

        db.get(&transaction, &height)?
            .map(|hash| {
                transaction.commit()?;
                self.by_hash(env, hash, include_transactions)
            })
            .unwrap_or(Ok(None))
    }
}
