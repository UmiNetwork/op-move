//! Types used to represent EVM data in the Moved trie.

use {
    alloy::{
        consensus,
        primitives::{keccak256, B256, U256},
        rlp,
    },
    eth_trie::{EthTrie, Trie, DB},
    revm::primitives::AccountInfo,
    std::{
        collections::BTreeMap,
        convert::Infallible,
        sync::{Arc, RwLock},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub inner: consensus::Account,
}

impl Account {
    pub fn new(nonce: u64, balance: U256, code_hash: B256, storage_root: B256) -> Self {
        Self {
            inner: consensus::Account {
                nonce,
                balance,
                storage_root,
                code_hash,
            },
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        rlp::encode(self.inner)
    }

    pub fn try_deserialize(bytes: &[u8]) -> Option<Self> {
        let inner = rlp::decode_exact(bytes).ok()?;
        Some(Self { inner })
    }
}

impl From<Account> for AccountInfo {
    fn from(value: Account) -> Self {
        Self {
            balance: value.inner.balance,
            nonce: value.inner.nonce,
            code_hash: value.inner.code_hash,
            code: None,
        }
    }
}

pub struct AccountStorage {
    pub trie: EthTrie<AccountStorageTrie>,
    root_hash: B256,
}

impl AccountStorage {
    const TRIE_EXPECT: &str = "AccountStorageTrie is infallible";

    pub fn serialize(&self) -> Vec<u8> {
        let db_bytes = self.trie.db.serialize();
        [self.root_hash.as_slice(), &db_bytes].concat()
    }

    pub fn try_deserialize(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 32 {
            return None;
        }
        let root_hash = B256::from_slice(&bytes[..32]);
        let db = AccountStorageTrie::try_deserialize(&bytes[32..])?;
        let trie = EthTrie::from(Arc::new(db), root_hash).ok()?;
        Some(Self { trie, root_hash })
    }

    pub fn root_hash(&mut self) -> B256 {
        self.root_hash = self.trie.root_hash().expect(Self::TRIE_EXPECT);
        self.root_hash
    }

    pub fn get(&self, index: U256) -> U256 {
        let trie_key = keccak256::<[u8; 32]>(index.to_be_bytes());
        let bytes = self.trie.get(trie_key.as_slice()).expect(Self::TRIE_EXPECT);
        bytes
            .map(|bytes| U256::from_be_slice(&bytes))
            .unwrap_or_default()
    }

    pub fn insert(&mut self, index: U256, value: U256) {
        let trie_key = keccak256::<[u8; 32]>(index.to_be_bytes());
        if value.is_zero() {
            self.trie
                .remove(trie_key.as_slice())
                .expect(Self::TRIE_EXPECT);
        } else {
            let value: [u8; 32] = value.to_be_bytes();
            self.trie
                .insert(trie_key.as_slice(), &value)
                .expect(Self::TRIE_EXPECT);
        }
    }
}

impl Default for AccountStorage {
    fn default() -> Self {
        let db = AccountStorageTrie::default();
        let mut trie = EthTrie::new(Arc::new(db));
        let root_hash = trie.root_hash().unwrap();
        Self { trie, root_hash }
    }
}

#[derive(Debug)]
pub struct AccountStorageTrie {
    inner: RwLock<BTreeMap<Vec<u8>, Vec<u8>>>,
}

impl AccountStorageTrie {
    // The `RWLock` cannot be poisoned because access to it is single-threaded.
    const LOCK_EXPECT: &str = "AccountStorageTrie lock not poisoned";

    pub fn serialize(&self) -> Vec<u8> {
        let inner_guard = self.inner.read().expect(Self::LOCK_EXPECT);
        let inner: &BTreeMap<Vec<u8>, Vec<u8>> = &inner_guard;
        bcs::to_bytes(inner).expect("AccountStorageTrie must serialize")
    }

    pub fn try_deserialize(bytes: &[u8]) -> Option<Self> {
        let inner = bcs::from_bytes(bytes).ok()?;
        Some(Self {
            inner: RwLock::new(inner),
        })
    }
}

impl Default for AccountStorageTrie {
    fn default() -> Self {
        Self {
            inner: RwLock::new(BTreeMap::new()),
        }
    }
}

impl DB for AccountStorageTrie {
    type Error = Infallible;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Self::Error> {
        let inner_guard = self.inner.read().expect(Self::LOCK_EXPECT);
        Ok(inner_guard.get(key).cloned())
    }

    fn insert(&self, key: &[u8], value: Vec<u8>) -> Result<(), Self::Error> {
        let mut inner_guard = self.inner.write().expect(Self::LOCK_EXPECT);
        inner_guard.insert(key.to_vec(), value);
        Ok(())
    }

    fn remove(&self, key: &[u8]) -> Result<(), Self::Error> {
        let mut inner_guard = self.inner.write().expect(Self::LOCK_EXPECT);
        inner_guard.remove(key);
        Ok(())
    }

    fn flush(&self) -> Result<(), Self::Error> {
        Ok(())
    }
}
