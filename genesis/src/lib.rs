pub use {
    framework::{CreateMoveVm, FRAMEWORK_ADDRESS},
    serde::{
        SerdeAccountChanges, SerdeAllChanges, SerdeChanges, SerdeOp, SerdeTableChange,
        SerdeTableChangeSet, SerdeTableInfo,
    },
    vm::MovedVm,
};

use {
    self::config::GenesisConfig, move_core_types::effects::ChangeSet,
    move_table_extension::TableChangeSet, moved_state::State,
};

pub mod config;
mod framework;
mod l2_contracts;
mod serde;
mod vm;

pub fn build(
    vm: &impl CreateMoveVm,
    config: &GenesisConfig,
    state: &impl State,
) -> (ChangeSet, TableChangeSet) {
    // Read L2 contract data
    let l2_genesis_file = std::fs::File::open(&config.l2_contract_genesis)
        .expect("L2 contracts genesis file must exist");
    let l2_contract_genesis =
        serde_json::from_reader(l2_genesis_file).expect("L2 genesis file must parse successfully");

    let mut changes = ChangeSet::new();

    // Deploy Move/Aptos/Sui frameworks
    let (changes_framework, table_changes) = framework::init_state(vm, state);

    // Deploy OP stack L2 contracts
    let changes_l2 = l2_contracts::init_state(l2_contract_genesis, state);

    changes
        .squash(changes_framework)
        .expect("Framework changes should not be in conflict");

    changes
        .squash(changes_l2)
        .expect("L2 contract changes should not be in conflict");

    (changes, table_changes)
}

pub fn apply(
    changes: ChangeSet,
    table_changes: TableChangeSet,
    config: &GenesisConfig,
    state: &mut impl State,
) {
    state
        .apply_with_tables(changes, table_changes)
        .expect("Changes should be applicable");

    // Validate final state
    let actual_state_root = state.state_root();
    let expected_state_root = config.initial_state_root;

    assert_eq!(
        actual_state_root, expected_state_root,
        "Fatal Error: Genesis state root mismatch"
    );
}

pub fn build_and_apply(vm: &impl CreateMoveVm, config: &GenesisConfig, state: &mut impl State) {
    let (changes, table_changes) = build(vm, config, state);
    apply(changes, table_changes, config, state);
}
