use {
    ethers_core::types::H256,
    move_vm_test_utils::gas_schedule::{self, CostTable},
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct GenesisConfig {
    pub chain_id: u64,
    pub initial_state_root: H256,
    // TODO: review if we should be using the test utils gas meter or the meter from aptos-gas-meter
    pub gas_costs: &'static CostTable,
}

impl Default for GenesisConfig {
    fn default() -> Self {
        Self {
            chain_id: 404,
            // TODO: determine real value based on result after deploying framework
            initial_state_root: H256::default(),
            gas_costs: &gas_schedule::INITIAL_COST_SCHEDULE,
        }
    }
}
