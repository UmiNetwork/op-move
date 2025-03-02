use {
    crate::{
        block,
        generic::{EncodableB256, EncodableU64},
    },
    moved_blockchain::payload::{PayloadId, PayloadQueries, PayloadResponse},
    moved_shared::primitives::{ToU64, B256},
};

pub type Db = heed::Database<EncodableU64, EncodableB256>;

pub const DB: &str = "payload";

#[derive(Debug)]
pub struct HeedPayloadQueries {
    env: &'static heed::Env,
}

impl HeedPayloadQueries {
    pub fn new(env: &'static heed::Env) -> Self {
        Self { env }
    }

    pub fn add_block_hash(&self, id: PayloadId, block_hash: B256) -> Result<(), heed::Error> {
        let mut transaction = self.env.write_txn()?;

        let db: Db = self
            .env
            .open_database(&transaction, Some(DB))?
            .expect("Payload database should exist");

        db.put(&mut transaction, &id.to_u64(), &block_hash)?;

        transaction.commit()
    }
}

impl PayloadQueries for HeedPayloadQueries {
    type Err = heed::Error;
    type Storage = &'static heed::Env;

    fn by_hash(
        &self,
        env: &Self::Storage,
        hash: B256,
    ) -> Result<Option<PayloadResponse>, Self::Err> {
        let transaction = env.read_txn()?;

        let db: block::Db = env
            .open_database(&transaction, Some(block::DB))?
            .expect("Block database should exist");

        let response = db.get(&transaction, &hash);

        transaction.commit()?;

        Ok(response?.map(PayloadResponse::from_block))
    }

    fn by_id(
        &self,
        env: &Self::Storage,
        id: PayloadId,
    ) -> Result<Option<PayloadResponse>, Self::Err> {
        let transaction = env.read_txn()?;

        let db: Db = env
            .open_database(&transaction, Some(DB))?
            .expect("Payload database should exist");

        db.get(&transaction, &id.to_u64())?
            .map(|hash| {
                transaction.commit()?;
                self.by_hash(env, hash)
            })
            .unwrap_or(Ok(None))
    }
}
