use {
    alloy::{genesis::Genesis, primitives::hex},
    aptos_gas_schedule::{InitialGasSchedule, NativeGasParameters, VMGasParameters},
    aptos_vm_types::storage::StorageGasParameters,
    move_core_types::{account_address::AccountAddress, gas_algebra::GasQuantity},
    moved_shared::primitives::B256,
};

pub const CHAIN_ID: u64 = 404;
const DEFAULT_L2_CONTRACT_GENESIS: &str =
    include_str!("../../execution/src/tests/res/l2_genesis_tests.json");

// We're setting the scale factor lower than Aptos because we want
// our gas costs to align with expected values for EVM chains.
// For example, a simple token transfer should be around 21_000 gas.
const EXTERNAL_GAS_SCALE_FACTOR: u64 = 600;

// If we spend around 10k internal gas units per ms of computation
// as the Aptos code comments suggest
// (https://github.com/aptos-labs/aptos-core/blob/aptos-node-v1.27.2/aptos-move/aptos-gas-schedule/src/gas_schedule/transaction.rs#L212)
// then this is around 5 minutes worth of computation.
// However, with the current EVM conversion of 600 internal gas units
// per EVM gas unit, this is only 167 ms of computation.
// I'm not sure which is right, benchmarks are needed.
const INTERNAL_EXECUTION_LIMIT: u64 = 3_000_000_000;

#[derive(Debug, Clone)]
pub struct GasCosts {
    pub vm: VMGasParameters,
    pub storage: StorageGasParameters,
    pub natives: NativeGasParameters,

    pub version: u64,
}

#[derive(Debug, Clone)]
pub struct GenesisConfig {
    pub chain_id: u64,
    pub initial_state_root: B256,
    pub gas_costs: GasCosts,
    pub treasury: AccountAddress,
    pub l2_contract_genesis: Genesis,
}

impl Default for GasCosts {
    fn default() -> Self {
        let mut result = Self {
            vm: VMGasParameters::initial(),
            storage: StorageGasParameters::latest(),
            natives: NativeGasParameters::initial(),
            version: aptos_gas_schedule::LATEST_GAS_FEATURE_VERSION,
        };
        result.vm.txn.gas_unit_scaling_factor = GasQuantity::new(EXTERNAL_GAS_SCALE_FACTOR);
        result.vm.txn.max_execution_gas = GasQuantity::new(INTERNAL_EXECUTION_LIMIT);
        result
    }
}

impl Default for GenesisConfig {
    fn default() -> Self {
        Self {
            chain_id: CHAIN_ID,
            initial_state_root: B256::from(hex!(
                "710e6935c134f27cb4c45c600a479750516cee9353ed2d292dab36edb8c16908"
            )),
            gas_costs: GasCosts::default(),
            treasury: AccountAddress::ONE, // todo: fill in the real address
            l2_contract_genesis: serde_json::from_str(DEFAULT_L2_CONTRACT_GENESIS)
                .expect("Default L2 contract genesis should be JSON encoded `Genesis` struct"),
        }
    }
}
