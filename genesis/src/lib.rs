pub use {
    framework::{FRAMEWORK_ADDRESS, load_aptos_framework_snapshot},
    serde::{
        SerdeAccountChanges, SerdeAllChanges, SerdeChanges, SerdeOp, SerdeTableChange,
        SerdeTableChangeSet, SerdeTableInfo,
    },
};

use {
    self::{config::GenesisConfig, vm::UmiVm},
    alloy::primitives::B256,
    std::sync::Arc,
    umi_evm_ext::state::{
        InMemoryDb, InMemoryStorageTrieRepository, StorageTrieRepository, StorageTriesChanges,
    },
    umi_state::{Changes, EthTrieState, InMemoryState, State},
};

pub mod config;

mod framework;

mod bridged_tokens;
mod l2_contracts;
mod serde;
mod table_changes;
pub mod vm;

/// Function to compute the initial state root from scratch in memory
/// (ignoring the `initial_state_root` field) using the given config.
pub fn compute_state_root(config: &GenesisConfig) -> B256 {
    let storage_trie = InMemoryStorageTrieRepository::new();
    let vm = UmiVm::new(config);

    let (changes, _) = build(&vm, config, &storage_trie);

    let db = InMemoryDb::empty();
    let mut state = EthTrieState::empty(Arc::new(db));

    state.apply(changes).expect("Changes should be applicable");
    state.state_root()
}

pub fn build(
    vm: &UmiVm,
    config: &GenesisConfig,
    storage_trie: &impl StorageTrieRepository,
) -> (Changes, StorageTriesChanges) {
    let mut state = InMemoryState::default();
    // Deploy Move/Aptos/Sui frameworks
    let changes_framework = framework::init_state(vm, &mut state);

    // Deploy OP stack L2 contracts
    let mut changes_l2 =
        l2_contracts::init_state(config.l2_contract_genesis.clone(), &state, storage_trie)
            .expect("L2 contracts must deploy");

    // Deploy additional bridged tokens (if any)
    if !config.token_list.is_empty() {
        changes_l2 = bridged_tokens::deploy_bridged_tokens(changes_l2, config.token_list.clone())
            .expect("Bridged tokens must deploy");
    }

    let mut changes = Changes::empty();

    changes
        .squash(changes_framework)
        .expect("Framework changes should not be in conflict");

    changes
        .squash(Changes::without_tables(changes_l2.accounts))
        .expect("L2 contract changes should not be in conflict");

    (changes, changes_l2.storage)
}

pub fn apply(
    changes: Changes,
    evm_storage_changes: StorageTriesChanges,
    config: &GenesisConfig,
    state: &mut impl State,
    storage_trie: &mut impl StorageTrieRepository,
) {
    state.apply(changes).expect("Changes should be applicable");
    storage_trie
        .apply(evm_storage_changes)
        .expect("EVM storage changes should be applicable");

    // Validate final state
    let actual_state_root = state.state_root();
    let expected_state_root = config.initial_state_root;

    assert_eq!(
        actual_state_root, expected_state_root,
        "Fatal Error: Genesis state root mismatch"
    );
}

pub fn build_and_apply(
    vm: &UmiVm,
    config: &GenesisConfig,
    state: &mut impl State,
    storage_trie: &mut impl StorageTrieRepository,
) {
    let (changes, evm_storage) = build(vm, config, storage_trie);
    apply(changes, evm_storage, config, state, storage_trie);
}

#[test]
fn test_compute_state_root() {
    let config = GenesisConfig::default();
    let computed_root = compute_state_root(&config);
    assert_eq!(computed_root, config.initial_state_root);
}
