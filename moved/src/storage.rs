use aptos_crypto::HashValue;
use aptos_jellyfish_merkle::mock_tree_store::MockTreeStore;
use aptos_jellyfish_merkle::{JellyfishMerkleTree, TreeReader};
use aptos_types::state_store::state_key::StateKey;
use ethers_core::types::H256;
use {
    move_binary_format::errors::PartialVMError,
    move_core_types::{effects::ChangeSet, resolver::MoveResolver},
    move_table_extension::{TableChangeSet, TableResolver},
    move_vm_test_utils::InMemoryStorage,
    std::fmt::Debug,
};

/// A persistent storage trait.
///
/// This trait inherits [`MoveResolver`] that can resolve both resources and modules and extends it
/// with the [`apply`] operation.
///
/// [`apply`]: Self::apply
pub trait Storage {
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

impl Storage for InMemoryStorage {
    type Err = PartialVMError;

    fn apply(&mut self, changes: ChangeSet) -> Result<(), PartialVMError> {
        InMemoryStorage::apply(self, changes)
    }

    fn apply_with_tables(
        &mut self,
        changes: ChangeSet,
        table_changes: TableChangeSet,
    ) -> Result<(), PartialVMError> {
        InMemoryStorage::apply_extended(self, changes, table_changes)
    }

    fn resolver(&self) -> &(impl MoveResolver<Self::Err> + TableResolver) {
        self
    }

    fn state_root(&self, _: u64) -> H256 {
        H256::zero()
    }
}

struct InMemoryState {
    storage: InMemoryStorage,
    tree: MockTreeStore<StateKey>,
}

impl InMemoryState {
    const ALLOW_OVERWRITE: bool = true;

    pub fn new() -> Self {
        Self {
            storage: InMemoryStorage::new(),
            tree: MockTreeStore::new(Self::ALLOW_OVERWRITE),
        }
    }

    fn tree(&self) -> JellyfishMerkleTree<impl TreeReader<StateKey>, StateKey> {
        JellyfishMerkleTree::new(&self.tree)
    }
}

impl Storage for InMemoryState {
    type Err = PartialVMError;

    fn apply(&mut self, changes: ChangeSet) -> Result<(), Self::Err> {
        self.storage.apply(changes)?;

        Ok(())
    }

    fn apply_with_tables(
        &mut self,
        changes: ChangeSet,
        table_changes: TableChangeSet,
    ) -> Result<(), Self::Err> {
        self.storage.apply_with_tables(changes, table_changes)?;

        Ok(())
    }

    fn resolver(&self) -> &(impl MoveResolver<Self::Err> + TableResolver) {
        &self.storage
    }

    fn state_root(&self, block_height: u64) -> H256 {
        self.tree().get_root_hash(block_height).unwrap().as_h256()
    }
}

trait AsH256 {
    fn as_h256(&self) -> H256;
}

impl AsH256 for HashValue {
    fn as_h256(&self) -> H256 {
        H256::from_slice(self.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aptos_crypto::HashValue;
    use aptos_jellyfish_merkle::TreeUpdateBatch;
    use aptos_storage_interface::AptosDbError;
    use aptos_types::transaction::Version;

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

        state.tree.write_tree_update_batch(batch).unwrap();

        let actual_state_root = state.state_root(version);
        let expected_state_root = new_root_hash.as_h256();

        assert_eq!(actual_state_root, expected_state_root);

        let version = 1;
        let (empty_root_hash, batch) = state
            .put_value_set_test(vec![(key, None)], version)
            .unwrap();

        state.tree.write_tree_update_batch(batch).unwrap();

        let actual_state_root = state.state_root(version);
        let expected_state_root = empty_root_hash.as_h256();

        assert_eq!(actual_state_root, expected_state_root);
    }
}
