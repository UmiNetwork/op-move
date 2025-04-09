use {
    alloy::{genesis::Genesis, primitives::hex},
    aptos_gas_schedule::{InitialGasSchedule, NativeGasParameters, VMGasParameters},
    aptos_vm_types::storage::StorageGasParameters,
    move_core_types::{account_address::AccountAddress, gas_algebra::GasQuantity},
    moved_evm_ext::EVM_SCALE_FACTOR,
    moved_shared::primitives::B256,
};

pub const CHAIN_ID: u64 = 404;
const DEFAULT_L2_CONTRACT_GENESIS: &str =
    include_str!("../../execution/src/tests/res/l2_genesis_tests.json");

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
        // We're setting the scale factor lower than Aptos because we want
        // our gas costs to align with expected values for EVM chains.
        result.vm.txn.gas_unit_scaling_factor = GasQuantity::new(EVM_SCALE_FACTOR);
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
