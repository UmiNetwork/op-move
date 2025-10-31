use {
    crate::{all::HeedDb, generic::EncodableB256},
    heed::RoTxn,
    umi_blockchain::state::HashToStateRootIndex,
    umi_shared::primitives::B256,
};

pub type Key = EncodableB256;
pub type Value = EncodableB256;
pub type Db = heed::Database<Key, Value>;

pub const DB: &str = "state";

#[derive(Debug, Clone)]
pub struct HeedStateRootIndex {
    env: heed::Env,
}

impl HeedStateRootIndex {
    pub const fn new(env: heed::Env) -> Self {
        Self { env }
    }
}

impl HashToStateRootIndex for HeedStateRootIndex {
    type Err = heed::Error;

    fn root_by_hash(&self, hash: B256) -> Result<Option<B256>, Self::Err> {
        let transaction = self.env.read_txn()?;

        let db = self.env.state_database(&transaction)?;

        let state_root = db.get(&transaction, &hash)?;

        Ok(state_root)
    }

    fn push_state_root(&self, block_hash: B256, state_root: B256) -> Result<(), Self::Err> {
        let mut transaction = self.env.write_txn()?;

        let db = self.env.state_database(&transaction)?;

        db.put(&mut transaction, &block_hash, &state_root)?;

        transaction.commit()
    }
}

pub trait HeedStateExt {
    fn state_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<Key, Value>>;
}

impl HeedStateExt for heed::Env {
    fn state_database(&self, rtxn: &RoTxn) -> heed::Result<HeedDb<Key, Value>> {
        let db: Db = self
            .open_database(rtxn, Some(DB))?
            .expect("State root database should exist");

        Ok(HeedDb(db))
    }
}
