use {
    crate::generic::{EncodableB256, EncodableU64},
    heed::types::SerdeBincode,
    moved_blockchain::block::{BlockRepository, ExtendedBlock},
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
