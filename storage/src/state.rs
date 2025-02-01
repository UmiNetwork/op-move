use {
    crate::{trie::FromOptRoot, RocksEthTrieDb},
    eth_trie::{EthTrie, TrieError, DB},
    move_binary_format::errors::PartialVMError,
    move_core_types::{effects::ChangeSet, resolver::MoveResolver},
    move_table_extension::{TableChangeSet, TableResolver},
    moved::state_actor::EthTrieResolver,
    moved_shared::primitives::B256,
    moved_state::{InsertChangeSetIntoMerkleTrie, State},
    std::sync::Arc,
};

/// A blockchain state implementation backed by [`rocksdb`] as its persistent storage engine.
pub struct RocksDbState<'db> {
    db: Arc<RocksEthTrieDb<'db>>,
    resolver: EthTrieResolver<RocksEthTrieDb<'db>>,
    state_root: Option<B256>,
}

impl<'db> RocksDbState<'db> {
    pub fn new(db: Arc<RocksEthTrieDb<'db>>) -> Self {
        let state_root = db
            .root()
            .expect("Database should be able to fetch state root");

        Self {
            resolver: EthTrieResolver::new(EthTrie::from_opt_root(db.clone(), state_root)),
            state_root,
            db,
        }
    }

    fn persist_state_root(&self) -> Result<(), rocksdb::Error> {
        self.state_root
            .map(|root| self.db.put_root(root))
            .unwrap_or(Ok(()))
    }

    fn tree(&self) -> EthTrie<RocksEthTrieDb<'db>> {
        EthTrie::from_opt_root(self.db.clone(), self.state_root)
    }
}

impl<'db> State for RocksDbState<'db> {
    type Err = TrieError;

    fn apply(&mut self, changes: ChangeSet) -> Result<(), Self::Err> {
        let mut tree = self.tree();
        let root = tree.insert_change_set_into_merkle_trie(&changes)?;
        self.state_root.replace(root);
        self.resolver = EthTrieResolver::new(tree);
        self.persist_state_root().unwrap();
        Ok(())
    }

    fn apply_with_tables(
        &mut self,
        changes: ChangeSet,
        _table_changes: TableChangeSet,
    ) -> Result<(), Self::Err> {
        self.apply(changes)
    }

    fn db(&self) -> Arc<impl DB> {
        self.db.clone()
    }

    fn resolver(&self) -> &(impl MoveResolver<PartialVMError> + TableResolver) {
        &self.resolver
    }

    fn state_root(&self) -> B256 {
        self.state_root.unwrap_or_default()
    }
}
