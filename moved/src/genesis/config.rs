use {
    alloy_primitives::hex,
    aptos_gas_schedule::{InitialGasSchedule, VMGasParameters},
    aptos_vm_types::storage::StorageGasParameters,
    ethers_core::types::H256,
};

pub const CHAIN_ID: u64 = 404;

#[derive(Debug, Clone)]
pub struct GasCosts {
    pub vm: VMGasParameters,
    pub storage: StorageGasParameters,
    pub version: u64,
}

#[derive(Debug, Clone)]
pub struct GenesisConfig {
    pub chain_id: u64,
    pub initial_state_root: H256,
    pub gas_costs: GasCosts,
}

impl Default for GasCosts {
    fn default() -> Self {
        Self {
            vm: VMGasParameters::initial(),
            storage: StorageGasParameters::latest(),
            version: aptos_gas_schedule::LATEST_GAS_FEATURE_VERSION,
        }
    }
}

impl Default for GenesisConfig {
    fn default() -> Self {
        Self {
            chain_id: CHAIN_ID,
            initial_state_root: H256::from(hex!(
                "2503e9898a861f2753c4bd406d6454acba57f101096fa13ab01c5d7d585fcbf4"
            )),
            gas_costs: GasCosts::default(),
        }
    }
}
