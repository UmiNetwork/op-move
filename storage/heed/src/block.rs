use {
    crate::{
        all::HeedDb,
        generic::{EncodableB256, EncodableU64, SerdeJson},
        transaction::HeedTransactionExt,
    },
    heed::RoTxn,
    std::marker::PhantomData,
    umi_blockchain::block::{
        BlockQueries, BlockRepository, BlockResponse, ExtendedBlock, ForkchoiceState,
    },
    umi_shared::primitives::B256,
};

pub type Key = EncodableB256;
pub type Value = EncodableBlock;
pub type Db = heed::Database<Key, Value>;
pub type HeightKey = EncodableU64;
pub type HeightValue = EncodableB256;
pub type HeightDb = heed::Database<HeightKey, HeightValue>;
pub type FcKey = EncodableU64;
pub type FcValue = SerdeJson<ForkchoiceState>;
pub type FcDb = heed::Database<FcKey, FcValue>;
pub type EncodableBlock = SerdeJson<ExtendedBlock>;

pub const DB: &str = "block";
pub const HEIGHT_DB: &str = "height";
pub const FC_DB: &str = "forkchoice";
const FC_KEY: u64 = 0;

#[derive(Debug)]
pub struct HeedBlockRepository<'db>(PhantomData<&'db ()>);

impl Default for HeedBlockRepository<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl HeedBlockRepository<'_> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl BlockRepository for HeedBlockRepository<'_> {
    type Err = heed::Error;
    type Storage = heed::Env;

    fn add(&mut self, env: &mut Self::Storage, block: ExtendedBlock) -> Result<(), Self::Err> {
        let mut transaction = env.write_txn()?;

        let db = env.block_database(&transaction)?;

        db.put(&mut transaction, &block.hash, &block)?;

        let db = env.block_height_database(&transaction)?;

        db.put(&mut transaction, &block.block.header.number, &block.hash)?;

        transaction.commit()
    }

    fn by_hash(&self, env: &Self::Storage, hash: B256) -> Result<Option<ExtendedBlock>, Self::Err> {
        let transaction = env.read_txn()?;

        let db = env.block_database(&transaction)?;

        let response = db.get(&transaction, &hash);

        transaction.commit()?;

        response
    }

    fn forkchoice_update(
        &mut self,
        env: &mut Self::Storage,
        state: ForkchoiceState,
    ) -> Result<(), Self::Err> {
        let mut transaction = env.write_txn()?;
        let db = env.forkchoice_database(&transaction)?;
        db.put(&mut transaction, &FC_KEY, &state)?;
        transaction.commit()
    }

    fn get_forkchoice_state(&self, env: &Self::Storage) -> Result<ForkchoiceState, Self::Err> {
        let transaction = env.read_txn()?;
        let db = env.forkchoice_database(&transaction)?;
        let state = db.get(&transaction, &FC_KEY)?;
        transaction.commit()?;
        Ok(state.unwrap_or_default())
    }
}

#[derive(Debug, Clone)]
pub struct HeedBlockQueries;

impl Default for HeedBlockQueries {
    fn default() -> Self {
        Self::new()
    }
}

impl HeedBlockQueries {
    pub const fn new() -> Self {
        Self
    }
}

impl BlockQueries for HeedBlockQueries {
    type Err = heed::Error;
    type Storage = heed::Env;

    fn by_hash(
        &self,
        env: &Self::Storage,
        hash: B256,
        include_transactions: bool,
    ) -> Result<Option<BlockResponse>, Self::Err> {
        let db_transaction = env.read_txn()?;

        let db = env.block_database(&db_transaction)?;

        let block = db.get(&db_transaction, &hash)?;

        Ok(Some(match block {
            Some(block) if include_transactions => {
                let db = env.transaction_database(&db_transaction)?;

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

    fn get_forkchoice_state(&self, env: &Self::Storage) -> Result<ForkchoiceState, Self::Err> {
        let transaction = env.read_txn()?;
        let db = env.forkchoice_database(&transaction)?;
        let state = db.get(&transaction, &FC_KEY)?;
        transaction.commit()?;
        Ok(state.unwrap_or_default())
    }

    fn height_to_hash(&self, env: &Self::Storage, height: u64) -> Result<B256, Self::Err> {
        let transaction = env.read_txn()?;
        let db = env.block_height_database(&transaction)?;
        let maybe_hash = db.get(&transaction, &height)?;
        transaction.commit()?;
        maybe_hash.ok_or_else(|| {
            heed::Error::Io(std::io::Error::other(
                "DB access is protected so queried heights always map to hashes",
            ))
        })
    }
}

pub trait HeedBlockExt {
    fn block_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<Key, Value>>;

    fn block_height_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<HeightKey, HeightValue>>;

    fn forkchoice_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<FcKey, FcValue>>;
}

impl HeedBlockExt for heed::Env {
    fn block_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<Key, Value>> {
        let db: Db = self
            .open_database(rtxn, Some(DB))?
            .expect("Block database should exist");

        Ok(HeedDb(db))
    }

    fn block_height_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<HeightKey, HeightValue>> {
        let db: HeightDb = self
            .open_database(rtxn, Some(HEIGHT_DB))?
            .expect("Block height database should exist");

        Ok(HeedDb(db))
    }

    fn forkchoice_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<FcKey, FcValue>> {
        let db: FcDb = self
            .open_database(rtxn, Some(FC_DB))?
            .expect("Forkchoice database should exist");

        Ok(HeedDb(db))
    }
}
