use {
    crate::{
        genesis::config::GenesisConfig, primitives::U256,
        types::transactions::NormalizedEthTransaction,
    },
    aptos_gas_meter::{AptosGasMeter, GasAlgebra, StandardGasAlgebra, StandardGasMeter},
};

pub fn new_gas_meter(
    genesis_config: &GenesisConfig,
    gas_limit: u64,
) -> StandardGasMeter<StandardGasAlgebra> {
    StandardGasMeter::new(StandardGasAlgebra::new(
        genesis_config.gas_costs.version,
        genesis_config.gas_costs.vm.clone(),
        genesis_config.gas_costs.storage.clone(),
        false,
        gas_limit,
    ))
}

pub fn total_gas_used<G: AptosGasMeter>(gas_meter: &G, genesis_config: &GenesisConfig) -> u64 {
    let gas_algebra = gas_meter.algebra();
    // Note: this sum is overflow safe because it uses saturating addition
    // by default in the implementation of `GasQuantity`.
    let total = gas_algebra.execution_gas_used()
        + gas_algebra.io_gas_used()
        + gas_algebra.storage_fee_used_in_gas_units();
    let total: u64 = total.into();
    // Aptos scales up the input gas limit for some reason,
    // so we need to reverse that scaling when we return.
    let scaling_factor: u64 = genesis_config.gas_costs.vm.txn.scaling_factor().into();
    total / scaling_factor
}

impl NormalizedEthTransaction {
    /// Calculates an amount of Wei per a single unit of gas that is paid on top of the base fee for
    /// this transaction.
    ///
    /// The max fee per gas should be greater than sum of base fee and max priority fee per gas. The
    /// difference is refunded to the user.
    ///
    /// Therefore, the returned value should be max priority fee per gas, also known as "tip" for
    /// validator.
    pub fn tip_per_gas(&self, base_fee: U256) -> U256 {
        let extra_fee = self
            .max_fee_per_gas
            .checked_sub(base_fee)
            .unwrap_or(U256::ZERO);
        self.max_priority_fee_per_gas.min(extra_fee)
    }
}
