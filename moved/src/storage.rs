use aptos_jellyfish_merkle::mock_tree_store::MockTreeStore;
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
pub trait Storage: MoveResolver<Self::Err> + TableResolver {
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

    fn state_root(&self) -> H256;
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

    fn state_root(&self) -> H256 {
        H256::zero()
    }
}

struct InMemoryBaba {
    storage: InMemoryStorage,
    tree: MockTreeStore<StateKey>,
}

impl InMemoryBaba {
    const ALLOW_OVERWRITE: bool = true;

    pub fn new() -> Self {
        Self {
            storage: InMemoryStorage::new(),
            tree: MockTreeStore::new(Self::ALLOW_OVERWRITE),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aptos_crypto::hash::SPARSE_MERKLE_PLACEHOLDER_HASH;
    use aptos_crypto::HashValue;
    use aptos_jellyfish_merkle::test_helper::ValueBlob;
    use aptos_jellyfish_merkle::{JellyfishMerkleTree, TreeUpdateBatch};
    use aptos_storage_interface::AptosDbError;
    use aptos_types::transaction::Version;

    impl InMemoryBaba {
        pub fn put_value_set_test(
            &self,
            value_set: Vec<(HashValue, Option<&(HashValue, StateKey)>)>,
            version: Version,
        ) -> Result<(HashValue, TreeUpdateBatch<StateKey>), AptosDbError> {
            let tree = JellyfishMerkleTree::new(&self.tree);
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

        pub fn get(
            &self,
            key: HashValue,
            version: Version,
        ) -> aptos_storage_interface::Result<Option<HashValue>> {
            Ok(JellyfishMerkleTree::new(&self.tree)
                .get_with_proof(key, version)?
                .0
                .map(|x| x.0))
        }
    }

    #[test]
    fn test_insert_to_empty_tree() {
        let baba = InMemoryBaba::new();
        let tree = JellyfishMerkleTree::new(&baba.tree);

        // Tree is initially empty. Root is a null node. We'll insert a key-value pair which creates a
        // leaf node.
        let key = HashValue::random();
        let state_key = StateKey::raw(&[1u8, 2u8, 3u8, 4u8]);
        let value_hash = HashValue::random();

        // batch version
        let (_new_root_hash, batch) = baba
            .put_value_set_test(
                vec![(key, Some(&(value_hash, state_key)))],
                0, /* version */
            )
            .unwrap();
        assert!(batch
            .stale_node_index_batch
            .iter()
            .flatten()
            .next()
            .is_none());

        baba.tree.write_tree_update_batch(batch).unwrap();
        assert_eq!(baba.get(key, 0).unwrap().unwrap(), value_hash);

        let (empty_root_hash, batch) = baba
            .put_value_set_test(vec![(key, None)], 1 /* version */)
            .unwrap();
        baba.tree.write_tree_update_batch(batch).unwrap();
        assert_eq!(baba.get(key, 1).unwrap(), None);
        assert_eq!(empty_root_hash, *SPARSE_MERKLE_PLACEHOLDER_HASH);
    }
}
