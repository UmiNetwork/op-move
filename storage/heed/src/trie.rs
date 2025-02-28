use {
    crate::generic::{EncodableB256, EncodableBytes, EncodableU64},
    eth_trie::{EthTrie, TrieError, DB},
    moved_shared::primitives::B256,
    std::sync::Arc,
};

pub const TRIE_DB: &str = "trie";
pub const ROOT_DB: &str = "trie_root";
pub const ROOT_KEY: u64 = 0u64;

pub struct HeedEthTrieDb<'db> {
    env: &'db heed::Env,
}

impl<'db> HeedEthTrieDb<'db> {
    pub fn new(env: &'db heed::Env) -> Self {
        Self { env }
    }

    pub fn root(&self) -> Result<Option<B256>, heed::Error> {
        let transaction = self.env.read_txn()?;

        let db: heed::Database<EncodableU64, EncodableB256> = self
            .env
            .open_database(&transaction, Some(ROOT_DB))?
            .expect("Trie root database should exist");

        db.get(&transaction, &ROOT_KEY)
    }

    pub fn put_root(&self, root: B256) -> Result<(), heed::Error> {
        let mut transaction = self.env.write_txn()?;

        let db: heed::Database<EncodableU64, EncodableB256> = self
            .env
            .open_database(&transaction, Some(ROOT_DB))?
            .expect("Trie root database should exist");

        db.put(&mut transaction, &ROOT_KEY, &root)
    }
}

impl<'db> DB for HeedEthTrieDb<'db> {
    type Error = heed::Error;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        let transaction = self.env.read_txn()?;

        let db: heed::Database<EncodableBytes, EncodableBytes> = self
            .env
            .open_database(&transaction, Some(TRIE_DB))?
            .expect("Trie root database should exist");

        Ok(db.get(&transaction, key)?.map(<[u8]>::to_vec))
    }

    fn insert(&self, key: &[u8], value: Vec<u8>) -> Result<(), Self::Error> {
        let mut transaction = self.env.write_txn()?;

        let db: heed::Database<EncodableBytes, EncodableBytes> = self
            .env
            .open_database(&transaction, Some(TRIE_DB))?
            .expect("Trie root database should exist");

        db.put(&mut transaction, key, value.as_slice())
    }

    fn remove(&self, _key: &[u8]) -> Result<(), Self::Error> {
        // Intentionally ignored to not remove historical trie nodes
        Ok(())
    }

    fn flush(&self) -> Result<(), Self::Error> {
        // Intentionally ignored as cache management is delegated to the database
        Ok(())
    }
}

pub trait TryFromOptRoot<D> {
    fn try_from_opt_root(db: Arc<D>, root: Option<B256>) -> Result<Self, TrieError>
    where
        Self: Sized;
}

impl<D: DB> TryFromOptRoot<D> for EthTrie<D> {
    fn try_from_opt_root(db: Arc<D>, root: Option<B256>) -> Result<EthTrie<D>, TrieError> {
        match root {
            None => Ok(EthTrie::new(db)),
            Some(root) => EthTrie::from(db, root),
        }
    }
}

pub trait FromOptRoot<D> {
    fn from_opt_root(db: Arc<D>, root: Option<B256>) -> Self
    where
        Self: Sized;
}

impl<D, T: TryFromOptRoot<D>> FromOptRoot<D> for T {
    fn from_opt_root(db: Arc<D>, root: Option<B256>) -> Self {
        Self::try_from_opt_root(db, root).expect("Root node should exist")
    }
}
