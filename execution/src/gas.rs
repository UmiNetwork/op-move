use {
    crate::transaction::NormalizedEthTransaction,
    aptos_gas_algebra::{GasExpression, NumBytes},
    aptos_gas_meter::{AptosGasMeter, GasAlgebra, StandardGasAlgebra, StandardGasMeter},
    aptos_gas_schedule::gas_params::natives::aptos_framework::{
        CODE_REQUEST_PUBLISH_BASE, CODE_REQUEST_PUBLISH_PER_BYTE,
    },
    move_core_types::{account_address::AccountAddress, ident_str},
    op_alloy::rpc_types::L1BlockInfo,
    umi_genesis::config::GenesisConfig,
    umi_shared::primitives::U256,
};

const JOVIAN_CALLDATA_SIZE: usize = 178;
const DA_SCALING_FACTOR: u64 = 1_000_000;

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

pub fn charge_new_module_processing<G: AptosGasMeter>(
    gas_meter: &mut G,
    genesis_config: &GenesisConfig,
    address: &AccountAddress,
    module_size: u64,
) -> Result<(), umi_shared::error::Error> {
    let module_size = NumBytes::new(module_size);

    // Charge for requesting to publish a module
    let publish_request_exp =
        CODE_REQUEST_PUBLISH_BASE + CODE_REQUEST_PUBLISH_PER_BYTE * module_size;
    let publish_request_cost = publish_request_exp.evaluate(
        gas_meter.feature_version(),
        &genesis_config.gas_costs.natives,
    );
    gas_meter
        .algebra_mut()
        .charge_execution(publish_request_cost)
        .map_err(umi_shared::error::Error::from)?;

    // Charge for loading that module into memory
    // Note: the name does not matter because it is not used in the
    // standard gas meter implementation.
    gas_meter
        .charge_dependency(true, address, ident_str!("does_not_matter"), module_size)
        .map_err(umi_shared::error::Error::from)?;

    Ok(())
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
    pub fn tip_per_gas(&self, base_fee: u64) -> u128 {
        let extra_fee = self.max_fee_per_gas.saturating_sub(base_fee.into());
        self.max_priority_fee_per_gas.min(extra_fee)
    }

    pub fn effective_gas_price(&self, base_fee: u64) -> u128 {
        self.tip_per_gas(base_fee).saturating_add(base_fee.into())
    }
}

pub trait L1GasFee {
    fn l1_fee(&self, input: L1GasFeeInput) -> U256;
    fn l1_block_info(&self, input: L1GasFeeInput) -> Option<L1BlockInfo>;
    fn da_footprint(&self, input: L1GasFeeInput) -> u64;
    fn operator_fee(&self, _gas_limit: u64) -> U256;
    fn operator_fee_scalar(&self) -> U256;
}

pub trait L2GasFee {
    fn l2_fee(&self, input: L2GasFeeInput) -> U256;
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct L1GasFeeInput {
    zero_bytes: U256,
    non_zero_bytes: U256,
    fast_lz_size: U256,
}

impl L1GasFeeInput {
    pub fn new(zero_bytes: U256, non_zero_bytes: U256, fast_lz_size: U256) -> Self {
        Self {
            zero_bytes,
            non_zero_bytes,
            fast_lz_size,
        }
    }
}

impl<T: AsRef<[u8]>> From<T> for L1GasFeeInput {
    fn from(value: T) -> Self {
        let tx_data = value.as_ref();
        let zero_bytes = U256::from(tx_data.iter().filter(|&&v| v == 0).count());
        let non_zero_bytes = U256::from(tx_data.len()) - zero_bytes;
        // From FastLZ binding docs: the output buffer must be at least 5% larger than the input buffer and can not be smaller than 66 bytes.
        let mut output = vec![0u8; tx_data.len() * 21 / 20 + 66];
        let fast_lz_size = fastlz::compress(tx_data, &mut output)
            .expect("Compression can only panic on out buffer overflow")
            .len();

        Self::new(zero_bytes, non_zero_bytes, U256::from(fast_lz_size))
    }
}

/// Transaction-defined parameters necessary for
/// calculation of L2 gas costs.
#[derive(Debug, Clone)]
pub struct L2GasFeeInput {
    pub gas_limit: u64,
    pub effective_gas_price: u128,
}

impl L2GasFeeInput {
    pub fn new(gas_limit: u64, effective_gas_price: u128) -> Self {
        Self {
            gas_limit,
            effective_gas_price,
        }
    }
}

impl From<(u64, u128)> for L2GasFeeInput {
    fn from(value: (u64, u128)) -> Self {
        Self {
            gas_limit: value.0,
            effective_gas_price: value.1,
        }
    }
}

#[derive(Debug)]
pub struct JovianGasFee {
    base_fee: U256,
    base_fee_scalar: U256,
    blob_base_fee: U256,
    blob_base_fee_scalar: U256,
    operator_fee_scalar: U256,
    operator_fee_constant: U256,
    da_footprint_gas_scalar: u16,
}

impl JovianGasFee {
    const GAS_PRICE_MULTIPLIER: U256 = U256::from_limbs([16, 0, 0, 0]);
    /// Absolute part of the negative intercept
    const INTERCEPT_ABS: u32 = 42_585_600;
    const FAST_LZ_COEF: u32 = 836_500;
    const MIN_TX_SIZE: u32 = 100;

    pub fn new(
        base_fee: U256,
        base_fee_scalar: u32,
        blob_base_fee: U256,
        blob_base_fee_scalar: u32,
        operator_fee_scalar: u32,
        operator_fee_constant: u64,
        da_footprint_gas_scalar: u16,
    ) -> Self {
        Self {
            base_fee,
            base_fee_scalar: U256::from(base_fee_scalar),
            blob_base_fee,
            blob_base_fee_scalar: U256::from(blob_base_fee_scalar),
            operator_fee_scalar: U256::from(operator_fee_scalar),
            operator_fee_constant: U256::from(operator_fee_constant),
            da_footprint_gas_scalar,
        }
    }

    fn linear_size_estimate_scaled(&self, fast_lz_size: U256) -> U256 {
        // The spec <https://specs.optimism.io/protocol/fjord/exec-engine.html#fjord-l1-cost-fee-changes-fastlz-estimator>
        // returns a `U256` as the final result, so we can widen the types in advance.
        let intercept = U256::from(Self::INTERCEPT_ABS);
        let fast_lz_coef = U256::from(Self::FAST_LZ_COEF);

        (fast_lz_coef * fast_lz_size).saturating_sub(intercept)
    }
}

impl L1GasFee for JovianGasFee {
    fn l1_fee(&self, input: L1GasFeeInput) -> U256 {
        let min_tx_size = U256::from(Self::MIN_TX_SIZE);

        let estimated_size_scaled = {
            let min_scaled = min_tx_size * U256::from(1_000_000);
            let scaled = self.linear_size_estimate_scaled(input.fast_lz_size);
            scaled.max(min_scaled)
        };

        let weighted_gas_price = Self::GAS_PRICE_MULTIPLIER * self.base_fee_scalar * self.base_fee
            + self.blob_base_fee_scalar * self.blob_base_fee;

        // We scale down by 1e6 instead of 1e12 to preserve the previous Ecotone omission of
        // a 1e6 divisor.
        estimated_size_scaled * weighted_gas_price / U256::from(1_000_000)
    }

    fn l1_block_info(&self, input: L1GasFeeInput) -> Option<L1BlockInfo> {
        Some(L1BlockInfo {
            l1_gas_price: Some(self.base_fee.saturating_to()),
            l1_gas_used: None,
            l1_fee: Some(self.l1_fee(input).saturating_to()),
            l1_fee_scalar: None,
            l1_base_fee_scalar: Some(self.base_fee_scalar.saturating_to()),
            l1_blob_base_fee: Some(self.blob_base_fee.saturating_to()),
            l1_blob_base_fee_scalar: Some(self.blob_base_fee_scalar.saturating_to()),
            operator_fee_scalar: Some(self.operator_fee_scalar.saturating_to()),
            operator_fee_constant: Some(self.operator_fee_constant.saturating_to()),
            da_footprint_gas_scalar: Some(self.da_footprint_gas_scalar),
        })
    }

    fn da_footprint(&self, input: L1GasFeeInput) -> u64 {
        let linear_estimate: u64 = (self.linear_size_estimate_scaled(input.fast_lz_size)
            / U256::from(DA_SCALING_FACTOR))
        .saturating_to();
        let da_usage_estimate = std::cmp::max(Self::MIN_TX_SIZE.into(), linear_estimate);
        da_usage_estimate.saturating_mul(self.da_footprint_gas_scalar.into())
    }

    fn operator_fee(&self, gas_limit: u64) -> U256 {
        // TODO: add the 1e6 multiplier (#569)
        U256::from(gas_limit)
            .saturating_mul(self.operator_fee_scalar)
            .saturating_add(self.operator_fee_constant)
    }

    fn operator_fee_scalar(&self) -> U256 {
        self.operator_fee_scalar
    }
}

/// This struct holds additional parameters and behavior as
/// defined by Umi network for L2 gas calculation that are
/// independent of transaction-defined limits or block state.
#[derive(Debug, Clone)]
pub struct UmiGasFee {
    gas_fee_multiplier: U256,
}

impl L2GasFee for UmiGasFee {
    fn l2_fee(&self, input: L2GasFeeInput) -> U256 {
        U256::from(input.effective_gas_price)
            .saturating_mul(U256::from(input.gas_limit))
            .saturating_mul(self.gas_fee_multiplier)
    }
}

/// Creates algorithm for calculating cost of publishing a transaction to layer-1 blockchain.
pub trait CreateL1GasFee {
    /// Extracts parameters from deposit transaction and creates the algorithm for calculating L1
    /// gas cost.
    fn for_deposit(&self, data: &[u8]) -> impl L1GasFee + 'static;
}

pub struct CreateFjordL1GasFee;

impl CreateL1GasFee for CreateFjordL1GasFee {
    fn for_deposit(&self, data: &[u8]) -> impl L1GasFee + 'static {
        // Sanity check for the `L1BlockInfo` having all recent fields
        if data.len() != JOVIAN_CALLDATA_SIZE {
            tracing::warn!(
                "Received L1BlockInfo that wasn't Jovian size: expected {}, got {}",
                JOVIAN_CALLDATA_SIZE,
                data.len(),
            );
        }

        // As specified in <https://specs.optimism.io/protocol/isthmus/l1-attributes.html>
        let l1_base_fee_scalar =
            u32::from_be_bytes(data[4..8].try_into().expect("Slice should be 4 bytes"));
        let l1_blob_base_fee_scalar =
            u32::from_be_bytes(data[8..12].try_into().expect("Slice should be 4 bytes"));
        let l1_base_fee = U256::from_be_slice(&data[36..68]);
        let l1_blob_base_fee = U256::from_be_slice(&data[68..100]);
        let operator_fee_scalar =
            u32::from_be_bytes(data[164..168].try_into().expect("Slice should be 4 bytes"));
        let operator_fee_constant =
            u64::from_be_bytes(data[168..176].try_into().expect("Slice should be 8 bytes"));
        let da_footprint_gas_scalar =
            u16::from_be_bytes(data[176..178].try_into().expect("Slice is 2 bytes"));

        JovianGasFee::new(
            l1_base_fee,
            l1_base_fee_scalar,
            l1_blob_base_fee,
            l1_blob_base_fee_scalar,
            operator_fee_scalar,
            operator_fee_constant,
            da_footprint_gas_scalar,
        )
    }
}

pub struct CreateUmiL2GasFee;

/// Creates algorithm for calculating cost of publishing a transaction to layer-2 blockchain.
pub trait CreateL2GasFee {
    const DEFAULT_L2_GAS_MULTIPLIER: U256 = U256::from_limbs([1, 0, 0, 0]);
    /// Instantiates L2 gas fee structure with a given multiplier. Basically a decoupled
    /// constructor.
    fn with_gas_fee_multiplier(&self, gas_fee_multiplier: U256) -> impl L2GasFee + 'static + Clone;

    fn with_default_gas_fee_multiplier(&self) -> impl L2GasFee + 'static + Clone {
        self.with_gas_fee_multiplier(Self::DEFAULT_L2_GAS_MULTIPLIER)
    }
}

impl CreateL2GasFee for CreateUmiL2GasFee {
    fn with_gas_fee_multiplier(&self, gas_fee_multiplier: U256) -> impl L2GasFee + 'static + Clone {
        UmiGasFee { gas_fee_multiplier }
    }
}

#[cfg(any(feature = "test-doubles", test))]
mod tests {
    use super::*;

    impl L1GasFee for U256 {
        fn l1_fee(&self, _input: L1GasFeeInput) -> U256 {
            *self
        }

        fn l1_block_info(&self, _input: L1GasFeeInput) -> Option<L1BlockInfo> {
            None
        }

        fn da_footprint(&self, _input: L1GasFeeInput) -> u64 {
            0
        }

        fn operator_fee(&self, _gas_limit: u64) -> U256 {
            U256::ZERO
        }

        fn operator_fee_scalar(&self) -> U256 {
            U256::ZERO
        }
    }

    impl L2GasFee for U256 {
        fn l2_fee(&self, _input: L2GasFeeInput) -> U256 {
            *self
        }
    }

    impl CreateL1GasFee for U256 {
        fn for_deposit(&self, _data: &[u8]) -> impl L1GasFee + 'static {
            *self
        }
    }

    impl CreateL2GasFee for U256 {
        fn with_gas_fee_multiplier(&self, _base_fee: U256) -> impl L2GasFee + 'static + Clone {
            *self
        }
    }
}
