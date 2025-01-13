use {
    crate::RocksEthTrieDb,
    eth_trie::{EthTrie, DB},
    move_binary_format::errors::PartialVMError,
    move_core_types::{effects::ChangeSet, resolver::MoveResolver},
    move_table_extension::{TableChangeSet, TableResolver},
    moved::{
        primitives::B256,
        state_actor::HistoricResolver,
        storage::{InsertChangeSetIntoMerkleTrie, State},
    },
    std::sync::Arc,
};

/// A blockchain state implementation backed by [`rocksdb`] as its persistent storage engine.
pub struct RocksDbState<'db> {
    db: Arc<RocksEthTrieDb<'db>>,
    resolver: HistoricResolver<RocksEthTrieDb<'db>>,
    state_root: B256,
}

impl<'db> RocksDbState<'db> {
    pub fn new(db: Arc<RocksEthTrieDb<'db>>) -> Self {
        let state_root = db
            .root()
            .expect("Database should be able to fetch state root")
            .unwrap_or(B256::ZERO);

        Self {
            resolver: HistoricResolver::new(db.clone(), state_root),
            state_root,
            db,
        }
    }

    fn persist_state_root(&self) -> Result<(), rocksdb::Error> {
        self.db.put_root(self.state_root)
    }

    fn tree(&self) -> EthTrie<RocksEthTrieDb<'db>> {
        let db = self.db.clone();

        match self.state_root {
            B256::ZERO => EthTrie::new(db),
            root => EthTrie::from(db, root).unwrap(),
        }
    }
}

impl<'db> State for RocksDbState<'db> {
    type Err = PartialVMError;

    fn apply(&mut self, changes: ChangeSet) -> Result<(), Self::Err> {
        self.state_root = self
            .tree()
            .insert_change_set_into_merkle_trie(&changes)
            .unwrap();
        self.resolver = HistoricResolver::new(self.db.clone(), self.state_root);
        self.persist_state_root().unwrap();
        Ok(())
    }

    fn apply_with_tables(
        &mut self,
        changes: ChangeSet,
        _table_changes: TableChangeSet,
    ) -> Result<(), Self::Err> {
        self.state_root = self
            .tree()
            .insert_change_set_into_merkle_trie(&changes)
            .unwrap();
        self.resolver = HistoricResolver::new(self.db.clone(), self.state_root);
        self.persist_state_root().unwrap();
        Ok(())
    }

    fn db(&self) -> Arc<impl DB> {
        self.db.clone()
    }

    fn resolver(&self) -> &(impl MoveResolver<Self::Err> + TableResolver) {
        &self.resolver
    }

    fn state_root(&self) -> B256 {
        self.state_root
    }
}
