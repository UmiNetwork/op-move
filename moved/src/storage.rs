use {
    crate::{iter::GroupIterator, primitives::ToH256},
    aptos_crypto::hash::CryptoHash,
    aptos_jellyfish_merkle::{
        mock_tree_store::MockTreeStore, node_type::Node, JellyfishMerkleTree, TreeReader,
        TreeUpdateBatch,
    },
    aptos_storage_interface::AptosDbError,
    aptos_types::{
        state_store::{state_key::StateKey, state_value::StateValue},
        transaction::Version,
    },
    ethers_core::types::H256,
    move_binary_format::errors::PartialVMError,
    move_core_types::{effects::ChangeSet, resolver::MoveResolver},
    move_table_extension::{TableChangeSet, TableResolver},
    move_vm_test_utils::InMemoryStorage,
    std::{collections::HashMap, fmt::Debug},
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

    /// Retrieves the current state root.
    fn state_root(&self) -> H256;
}

pub struct InMemoryState {
    resolver: InMemoryStorage,
    db: MockTreeStore<StateKey>,
    version: Version,
}

impl InMemoryState {
    const ALLOW_OVERWRITE: bool = false;

    pub fn new() -> Self {
        Self {
            resolver: InMemoryStorage::new(),
            db: MockTreeStore::new(Self::ALLOW_OVERWRITE),
            version: 0,
        }
    }

    fn tree(&self) -> JellyfishMerkleTree<impl TreeReader<StateKey>, StateKey> {
        JellyfishMerkleTree::new(&self.db)
    }
}

impl State for InMemoryState {
    type Err = PartialVMError;

    fn apply(&mut self, changes: ChangeSet) -> Result<(), Self::Err> {
        self.resolver.apply(changes.clone())?;
        self.insert_change_set_into_merkle_trie(changes).unwrap();
        Ok(())
    }

    fn apply_with_tables(
        &mut self,
        changes: ChangeSet,
        table_changes: TableChangeSet,
    ) -> Result<(), Self::Err> {
        self.resolver
            .apply_extended(changes.clone(), table_changes)?;
        self.insert_change_set_into_merkle_trie(changes).unwrap();
        Ok(())
    }

    fn resolver(&self) -> &(impl MoveResolver<Self::Err> + TableResolver) {
        &self.resolver
    }

    fn state_root(&self) -> H256 {
        self.tree()
            .get_root_hash(self.version)
            .map(ToH256::to_h256)
            .unwrap()
    }
}

impl InMemoryState {
    fn increment_version(&mut self) -> Version {
        self.version += 1;
        self.version
    }

    fn insert_change_set_into_merkle_trie(
        &mut self,
        change_set: ChangeSet,
    ) -> Result<H256, AptosDbError> {
        let version = self.increment_version();
        let tree = self.tree();
        let mut tree_update_batch = TreeUpdateBatch::new();
        let persisted_versions = tree.get_shard_persisted_versions(None)?;

        let values = change_set
            .into_inner()
            .iter()
            .flat_map(|(address, changes)| {
                changes
                    .modules()
                    .iter()
                    .map(move |(k, v)| {
                        let value = v.clone().ok().map(StateValue::new_legacy);
                        let key = StateKey::module(address, k.as_ident_str());

                        (key, value)
                    })
                    .chain(changes.resources().iter().map(move |(k, v)| {
                        let value = v.clone().ok().map(StateValue::new_legacy);
                        let key = StateKey::resource(address, k).unwrap();

                        (key, value)
                    }))
            })
            .map(|(k, v)| (k.hash(), v.as_ref().map(|v| (v.hash(), k.clone()))))
            .collect::<HashMap<_, _>>();

        let values_per_shard = values
            .iter()
            .map(|(k, v)| (*k, v.as_ref()))
            .group_by(|(k, _)| k.nibble(0));
        const NIL: Node<StateKey> = Node::Null;
        let mut shard_root_nodes = [NIL; 16];

        for (shard_id, values) in values_per_shard {
            let (shard_root_node, batch) = tree.batch_put_value_set_for_shard(
                shard_id,
                values,
                None,
                persisted_versions[shard_id as usize],
                version,
            )?;

            tree_update_batch.combine(batch);
            shard_root_nodes[shard_id as usize] = shard_root_node;
        }

        let (root_hash, batch) = tree.put_top_levels_nodes(
            shard_root_nodes.to_vec(),
            self.version.checked_sub(1),
            self.version,
        )?;
        tree_update_batch.combine(batch);

        self.db.write_tree_update_batch(tree_update_batch)?;

        Ok(root_hash.to_h256())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        bytes::Bytes,
        move_core_types::{
            account_address::AccountAddress,
            effects::{AccountChanges, Op},
            identifier::Identifier,
        },
    };

    #[test]
    #[should_panic]
    fn test_state_root_from_empty_tree_fails() {
        InMemoryState::new().state_root();
    }

    #[test]
    fn test_insert_to_empty_tree_produces_new_state_root() {
        let mut state = InMemoryState::new();
        let mut change_set = ChangeSet::new();

        change_set
            .add_account_changeset(AccountAddress::new([0; 32]), AccountChanges::new())
            .unwrap();

        let expected_root_hash = state
            .insert_change_set_into_merkle_trie(change_set)
            .unwrap();
        let actual_state_root = state.state_root();

        assert_eq!(actual_state_root, expected_root_hash);
    }

    #[test]
    fn test_state_root_is_different_after_update_changes_trie() {
        let mut state = InMemoryState::new();
        let mut change_set = ChangeSet::new();

        change_set
            .add_account_changeset(AccountAddress::new([0; 32]), AccountChanges::new())
            .unwrap();
        state
            .insert_change_set_into_merkle_trie(change_set)
            .unwrap();
        let old_state_root = state.state_root();

        let mut change_set = ChangeSet::new();

        let mut account_change_set = AccountChanges::new();
        account_change_set
            .add_module_op(
                Identifier::new("lala").unwrap(),
                Op::New(Bytes::from_static(&[1u8; 2])),
            )
            .unwrap();
        change_set
            .add_account_changeset(AccountAddress::new([9; 32]), account_change_set)
            .unwrap();
        state
            .insert_change_set_into_merkle_trie(change_set)
            .unwrap();
        let new_state_root = state.state_root();

        assert_ne!(old_state_root, new_state_root);
    }

    #[test]
    fn test_state_root_remains_the_same_when_update_does_not_change_trie() {
        let mut state = InMemoryState::new();
        let mut change_set = ChangeSet::new();

        let mut account_change_set = AccountChanges::new();
        account_change_set
            .add_module_op(
                Identifier::new("lala").unwrap(),
                Op::New(Bytes::from_static(&[1u8; 2])),
            )
            .unwrap();

        change_set
            .add_account_changeset(AccountAddress::new([9; 32]), account_change_set)
            .unwrap();
        state
            .insert_change_set_into_merkle_trie(change_set)
            .unwrap();
        let expected_state_root = state.state_root();

        let mut change_set = ChangeSet::new();

        let mut account_change_set = AccountChanges::new();
        account_change_set
            .add_module_op(
                Identifier::new("lala").unwrap(),
                Op::New(Bytes::from_static(&[1u8; 2])),
            )
            .unwrap();
        change_set
            .add_account_changeset(AccountAddress::new([9; 32]), account_change_set)
            .unwrap();
        state
            .insert_change_set_into_merkle_trie(change_set)
            .unwrap();
        let actual_state_root = state.state_root();

        assert_eq!(actual_state_root, expected_state_root);
    }
}
