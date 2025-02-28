use {
    crate::{
        generic::{EncodableB256, EncodableU64},
        transaction::{EncodableTransaction, TRANSACTION_DB},
    },
    heed::types::SerdeBincode,
    moved_blockchain::block::{BlockQueries, BlockRepository, BlockResponse, ExtendedBlock},
    moved_shared::primitives::B256,
};

pub const BLOCK_DB: &str = "block";
pub const HEIGHT_DB: &str = "height";

pub type EncodableBlock = SerdeBincode<ExtendedBlock>;

#[derive(Debug)]
pub struct HeedBlockRepository;

impl BlockRepository for HeedBlockRepository {
    type Err = heed::Error;
    type Storage = &'static heed::Env;

    fn add(&mut self, env: &mut Self::Storage, block: ExtendedBlock) -> Result<(), Self::Err> {
        let mut transaction = env.write_txn()?;

        let db: heed::Database<EncodableB256, EncodableBlock> = env
            .open_database(&transaction, Some(BLOCK_DB))?
            .expect("Block database should exist");

        db.put(&mut transaction, &block.hash, &block)?;

        let db: heed::Database<EncodableU64, EncodableB256> = env
            .open_database(&transaction, Some(HEIGHT_DB))?
            .expect("Block height database should exist");

        db.put(&mut transaction, &block.block.header.number, &block.hash)
    }

    fn by_hash(&self, env: &Self::Storage, hash: B256) -> Result<Option<ExtendedBlock>, Self::Err> {
        let transaction = env.read_txn()?;

        let db: heed::Database<EncodableB256, EncodableBlock> = env
            .open_database(&transaction, Some(BLOCK_DB))?
            .expect("Block database should exist");

        db.get(&transaction, &hash)
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
        let transaction = env.read_txn()?;

        let db: heed::Database<EncodableB256, EncodableBlock> = env
            .open_database(&transaction, Some(BLOCK_DB))?
            .expect("Database should exist");

        let block = db.get(&transaction, &hash)?;

        Ok(Some(match block {
            Some(block) if include_transactions => {
                let db_transaction = env.read_txn()?;

                let db: heed::Database<EncodableB256, EncodableTransaction> = env
                    .open_database(&db_transaction, Some(TRANSACTION_DB))?
                    .expect("Database should exist");

                let transactions = block
                    .transaction_hashes()
                    .filter_map(|hash| db.get(&transaction, &hash).transpose())
                    .collect::<Result<Vec<_>, _>>()?;

                BlockResponse::from_block_with_transactions(block, transactions)
            }
            Some(block) => BlockResponse::from_block_with_transaction_hashes(block),
            None => return Ok(None),
        }))
    }

    fn by_height(
        &self,
        env: &Self::Storage,
        height: u64,
        include_transactions: bool,
    ) -> Result<Option<BlockResponse>, Self::Err> {
        let transaction = env.read_txn()?;

        let db: heed::Database<EncodableU64, EncodableB256> = env
            .open_database(&transaction, Some(HEIGHT_DB))?
            .expect("Database should exist");

        db.get(&transaction, &height)?
            .map(|hash| self.by_hash(env, hash, include_transactions))
            .unwrap_or(Ok(None))
    }
}
