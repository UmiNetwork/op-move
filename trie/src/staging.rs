use eth_trie::{DB, MemoryDB};

const INFALLIBLE: &str = "Memory DB should be infallible";

/// A [`DB`] implementation that wraps another [`DB`] and uses it only for reading.
///
/// Every write is made into an in-memory buffer. The in-memory buffer serves as a "staging area"
/// for modifications that are to be committed to the `inner` storage.
#[derive(Debug)]
pub struct StagingEthTrieDb<D> {
    pub memory: MemoryDB,
    pub inner: D,
}

impl<D: DB> StagingEthTrieDb<D> {
    pub fn new(inner: D) -> Self {
        Self {
            inner,
            memory: MemoryDB::new(true),
        }
    }
}

impl<D: DB> DB for StagingEthTrieDb<D> {
    type Error = D::Error;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        if let Some(value) = self.memory.get(key).expect(INFALLIBLE) {
            Ok(Some(value))
        } else {
            self.inner.get(key)
        }
    }

    fn insert(&self, key: &[u8], value: Vec<u8>) -> Result<(), Self::Error> {
        self.memory.insert(key, value).expect(INFALLIBLE);
        Ok(())
    }

    fn insert_batch(&self, keys: Vec<Vec<u8>>, values: Vec<Vec<u8>>) -> Result<(), Self::Error> {
        self.memory.insert_batch(keys, values).expect(INFALLIBLE);
        Ok(())
    }

    fn remove(&self, key: &[u8]) -> Result<(), Self::Error> {
        self.memory.remove(key).expect(INFALLIBLE);
        Ok(())
    }

    fn flush(&self) -> Result<(), Self::Error> {
        Ok(())
    }
}
