use {
    self::config::GenesisConfig, crate::storage::State, move_binary_format::errors::PartialVMError,
};

pub use framework::{FRAMEWORK_ADDRESS, L2_CROSS_DOMAIN_MESSENGER_ADDRESS};

pub mod config;
mod framework;
mod l2_contracts;

pub fn init_state(config: &GenesisConfig, state: &mut impl State<Err = PartialVMError>) {
    // Read L2 contract data
    let l2_genesis_file = std::fs::File::open(&config.l2_contract_genesis)
        .expect("L2 contracts genesis file must exist");
    let l2_contract_genesis =
        serde_json::from_reader(l2_genesis_file).expect("L2 genesis file must parse successfully");

    // Deploy Move/Aptos/Sui frameworks
    framework::init_state(state);

    // Deploy OP stack L2 contracts
    l2_contracts::init_state(l2_contract_genesis, state);

    // Validate final state
    let actual_state_root = state.state_root();
    let expected_state_root = config.initial_state_root;

    assert_eq!(
        actual_state_root, expected_state_root,
        "Fatal Error: Genesis state root mismatch"
    );
}
