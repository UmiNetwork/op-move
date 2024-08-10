use {
    crate::primitives::ToH256,
    aptos_jellyfish_merkle::{mock_tree_store::MockTreeStore, JellyfishMerkleTree, TreeReader},
    aptos_types::state_store::state_key::StateKey,
    ethers_core::types::H256,
    move_binary_format::errors::PartialVMError,
    move_core_types::{effects::ChangeSet, resolver::MoveResolver},
    move_table_extension::{TableChangeSet, TableResolver},
    move_vm_test_utils::InMemoryStorage,
    std::fmt::Debug,
};

/// A persistent state trait.
///
/// The state creates [`MoveResolver`] that can resolve both resources and modules.
///
/// with the [`apply`] operation.
///
/// [`apply`]: Self::apply
pub trait State {
    /// The associated error that can occur on storage operations.
    type Err: Debug;

    /// Applies the `changes` to the underlying storage state.
    fn apply(&mut self, changes: ChangeSet) -> Result<(), Self::Err>;

    /// Applies the `changes` to the underlying storage state. In addition, applies `table_changes`
    /// using the [`move_table_extension`].
    fn apply_with_tables(
        &mut self,
        changes: ChangeSet,
        table_changes: TableChangeSet,
    ) -> Result<(), Self::Err>;

    /// Returns a reference to a [`MoveResolver`] that can resolve both resources and modules.
    fn resolver(&self) -> &(impl MoveResolver<Self::Err> + TableResolver);

    /// Retrieves a state root at `block_height`.
    fn state_root(&self, block_height: u64) -> H256;
}

pub struct InMemoryState {
    resolver: InMemoryStorage,
    db: MockTreeStore<StateKey>,
}

impl InMemoryState {
    const ALLOW_OVERWRITE: bool = false;

    pub fn new() -> Self {
        Self {
            resolver: InMemoryStorage::new(),
            db: MockTreeStore::new(Self::ALLOW_OVERWRITE),
        }
    }

    fn tree(&self) -> JellyfishMerkleTree<impl TreeReader<StateKey>, StateKey> {
        JellyfishMerkleTree::new(&self.db)
    }
}

impl State for InMemoryState {
    type Err = PartialVMError;

    fn apply(&mut self, changes: ChangeSet) -> Result<(), Self::Err> {
        self.resolver.apply(changes)
    }

    fn apply_with_tables(
        &mut self,
        changes: ChangeSet,
        table_changes: TableChangeSet,
    ) -> Result<(), Self::Err> {
        self.resolver.apply_extended(changes, table_changes)
    }

    fn resolver(&self) -> &(impl MoveResolver<Self::Err> + TableResolver) {
        &self.resolver
    }

    fn state_root(&self, block_height: u64) -> H256 {
        self.tree()
            .get_root_hash(block_height)
            .map(ToH256::to_h256)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*, aptos_crypto::HashValue, aptos_jellyfish_merkle::TreeUpdateBatch,
        aptos_storage_interface::AptosDbError, aptos_types::transaction::Version,
    };

    impl InMemoryState {
        pub fn put_value_set_test(
            &self,
            value_set: Vec<(HashValue, Option<&(HashValue, StateKey)>)>,
            version: Version,
        ) -> Result<(HashValue, TreeUpdateBatch<StateKey>), AptosDbError> {
            let tree = self.tree();
            let mut tree_update_batch = TreeUpdateBatch::new();
            let mut shard_root_nodes = Vec::with_capacity(16);

            for shard_id in 0..16 {
                let value_set_for_shard = value_set
                    .iter()
                    .filter(|(k, _v)| k.nibble(0) == shard_id)
                    .cloned()
                    .collect();
                let (shard_root_node, shard_batch) = tree.batch_put_value_set_for_shard(
                    shard_id,
                    value_set_for_shard,
                    None,
                    version.checked_sub(1),
                    version,
                )?;

                tree_update_batch.combine(shard_batch);
                shard_root_nodes.push(shard_root_node);
            }

            let (root_hash, top_levels_batch) =
                tree.put_top_levels_nodes(shard_root_nodes, version.checked_sub(1), version)?;
            tree_update_batch.combine(top_levels_batch);

            Ok((root_hash, tree_update_batch))
        }
    }

    #[test]
    fn test_insert_to_empty_tree_produces_new_state_root() {
        let state = InMemoryState::new();

        let key = HashValue::zero();
        let state_key = StateKey::raw(&[1u8, 2u8, 3u8, 4u8]);
        let value_hash = HashValue::zero();
        let version = 0;

        let (new_root_hash, batch) = state
            .put_value_set_test(vec![(key, Some(&(value_hash, state_key)))], version)
            .unwrap();

        state.db.write_tree_update_batch(batch).unwrap();

        let actual_state_root = state.state_root(version);
        let expected_state_root = new_root_hash.to_h256();

        assert_eq!(actual_state_root, expected_state_root);

        let version = 1;
        let (empty_root_hash, batch) = state
            .put_value_set_test(vec![(key, None)], version)
            .unwrap();

        state.db.write_tree_update_batch(batch).unwrap();

        let actual_state_root = state.state_root(version);
        let expected_state_root = empty_root_hash.to_h256();

        assert_eq!(actual_state_root, expected_state_root);
    }
}
