//! This module is concerned about calculating fees charged for gas usage.

use std::{cmp::Ordering, num::NonZeroU32};

use alloy::primitives::{Bytes, U64};

pub const DEFAULT_EIP1559_ELASTICITY_MULTIPLIER: NonZeroU32 =
    NonZeroU32::new(6).expect("Supplied a non-zero value");
pub const DEFAULT_EIP1559_BASE_FEE_MAX_CHANGE_DENOMINATOR: NonZeroU32 =
    NonZeroU32::new(250).expect("Supplied a non-zero value");

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BaseFeeParameters {
    pub min_base_fee: u64,
    pub eip1559_params: EIP1559FeeParameters,
}

/// Represents base fee parameters as they are passed from payload attributes
/// or block headers. This is different from [`Eip1559GasFee`] in the sense
/// that it doesn't represent the current state of the blockchain, but a
/// change, and thus e.g. the difference in instantiation logic.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum EIP1559FeeParameters {
    #[default]
    Default,
    Custom {
        denominator: NonZeroU32,
        elasticity: NonZeroU32,
    },
}

impl EIP1559FeeParameters {
    pub fn new(denominator: NonZeroU32, elasticity: NonZeroU32) -> Self {
        if elasticity == DEFAULT_EIP1559_ELASTICITY_MULTIPLIER
            && denominator == DEFAULT_EIP1559_BASE_FEE_MAX_CHANGE_DENOMINATOR
        {
            Self::Default
        } else {
            Self::Custom {
                denominator,
                elasticity,
            }
        }
    }

    pub fn decode(extra_data: U64) -> Result<Self, umi_shared::error::Error> {
        // The first [0, 4) bytes are base fee denominator
        let denominator = extra_data.wrapping_shr(32).saturating_to::<u32>();
        // The bottom 4 bytes reserved for elasticity
        let elasticity = (extra_data.bitand(U64::from(0xFFFF_FFFFu64))).saturating_to::<u32>();

        match (NonZeroU32::new(elasticity), NonZeroU32::new(denominator)) {
            (None, None) => Ok(Self::Default),
            (_, None) => Err(umi_shared::error::Error::fee_denom_invariant_violation()),
            (None, _) => Err(umi_shared::error::Error::fee_elasticity_invariant_violation()),
            (Some(elasticity), Some(denominator)) => Ok(Self::Custom {
                denominator,
                elasticity,
            }),
        }
    }
    pub fn encode(&self) -> u64 {
        let (denominator, elasticity): (u64, u64) = match self {
            EIP1559FeeParameters::Default => (
                DEFAULT_EIP1559_BASE_FEE_MAX_CHANGE_DENOMINATOR.get().into(),
                DEFAULT_EIP1559_ELASTICITY_MULTIPLIER.get().into(),
            ),
            EIP1559FeeParameters::Custom {
                denominator,
                elasticity,
            } => (denominator.get().into(), elasticity.get().into()),
        };
        (denominator << 32) + elasticity
    }
}

/// Determines amount of fees charged per gas used in transaction execution.
///
/// The base fee applies to the entire block and does not include tips for validators.
/// Does not take into account any priority fees.
pub trait BaseGasFee {
    /// Calculates base fee per gas for a block based on the parent block.
    ///
    /// The concrete formula applied depends on the implementation.
    fn base_fee_per_gas(
        &self,
        parent_gas_limit: u64,
        parent_gas_used: u64,
        parent_base_fee_per_gas: u64,
    ) -> u64;

    fn set_parameters_from_extra_data(
        &mut self,
        extra_data: Bytes,
    ) -> Result<(), umi_shared::error::Error>;

    fn set_parameters_from_attrs(&mut self, params: &BaseFeeParameters);

    fn encode_parameters_for_header(&self) -> Bytes;
}

/// Calculates base fee per gas according to the Ethereum model based on EIP-1559,
/// with modifications according to the
/// [OP Stack Jovian Spec](https://specs.optimism.io/protocol/jovian/exec-engine.html).
///
/// The formula works in these steps:
/// 1. Calculate the *gas target*. The *gas target* is less than or equal to *gas limit*.
/// 2. Compare the *gas target* to *gas used* of the parent block.
/// 3. If *gas used* exceeds *gas target*, the fee is increased.
/// 4. If *gas used* falls below *gas target*, the fee is decreased.
/// 5. Otherwise, the fee is not changed.
///
/// The greater the difference between *gas used* and *gas target*, the greater the increase or
/// decrease of the fee.
///
/// The formula can be controlled by the given parameters. Their effect is:
/// * The greater the `elasticity_multiplier`, the smaller the *gas target*.
/// * The greater the `base_fee_max_change_denominator`, the smaller the increase or decrease of
///   the fee.
#[derive(Debug, Clone)]
pub struct JovianGasFee {
    /// Magnifies the difference between target gas amount and limit. Here are some facts about this
    /// parameter:
    ///
    /// * The greater the value the smaller the target gas.
    /// * This value has to be greater than zero.
    /// * A value of 1 makes the target the same as the limit.
    elasticity_multiplier: NonZeroU32,
    /// Reduces the difference between block's base fee per gas and its parent. Some properties can
    /// be observed:
    ///
    /// * The greater the value the smaller the increase or decrease of the base fee per gas.
    /// * This value has to be greater than zero.
    /// * A value of 1 makes the greatest fee increases or decreases.
    base_fee_max_change_denominator: NonZeroU32,
    /// The `minBaseFee` field is an absolute minimum expressed in wei.
    /// During base fee computation, if the computed baseFee is less
    /// than `minBaseFee`, it MUST be clamped to `minBaseFee`.
    min_base_fee: u64,
}

impl JovianGasFee {
    /// Sets up the base fee per gas calculation with given parameters.
    ///
    /// # Panics
    /// If either `elasticity_multiplier` or `base_fee_max_change_denominator` is zero.
    pub fn new(
        elasticity_multiplier: NonZeroU32,
        base_fee_max_change_denominator: NonZeroU32,
        min_base_fee: u64,
    ) -> Self {
        Self {
            elasticity_multiplier,
            base_fee_max_change_denominator,
            min_base_fee,
        }
    }
}

impl BaseGasFee for JovianGasFee {
    fn base_fee_per_gas(
        &self,
        parent_gas_limit: u64,
        parent_gas_used: u64,
        parent_base_fee_per_gas: u64,
    ) -> u64 {
        let gas_target = parent_gas_limit / self.elasticity_multiplier.get() as u64;

        let calculated = match parent_gas_used.cmp(&gas_target) {
            Ordering::Greater => {
                let delta = (parent_base_fee_per_gas.saturating_mul(parent_gas_used - gas_target)
                    / gas_target
                    / self.base_fee_max_change_denominator.get() as u64)
                    .max(1);

                parent_base_fee_per_gas.saturating_add(delta)
            }
            Ordering::Less => {
                let delta = parent_base_fee_per_gas.saturating_mul(gas_target - parent_gas_used)
                    / gas_target
                    / self.base_fee_max_change_denominator.get() as u64;

                parent_base_fee_per_gas.saturating_sub(delta)
            }
            Ordering::Equal => parent_base_fee_per_gas,
        };

        // Spec: if the computed baseFee is less than `minBaseFee`,
        // it MUST be clamped to `minBaseFee`.
        // See  https://specs.optimism.io/protocol/jovian/exec-engine.html
        calculated.max(self.min_base_fee)
    }

    fn set_parameters_from_extra_data(
        &mut self,
        extra_data: Bytes,
    ) -> Result<(), umi_shared::error::Error> {
        // See https://specs.optimism.io/protocol/jovian/exec-engine.html for format specification.
        if extra_data.len() != 17 {
            return Err(umi_shared::error::Error::extra_data_invariant_violation());
        };

        // As during block build the parameters are parsed from a byte-encoded field
        // in payload attributes, we have to do some conversions to read it from the
        // block header, most importantly skipping the version byte that is present
        // in the header, but absent from the attributes
        let encoded = U64::from_be_slice(&extra_data.slice(1..9));
        let eip1559_params = EIP1559FeeParameters::decode(encoded)?;
        let min_base_fee = U64::from_be_slice(&extra_data.slice(9..17));
        let params = BaseFeeParameters {
            eip1559_params,
            min_base_fee: min_base_fee.saturating_to(),
        };
        self.set_parameters_from_attrs(&params);
        Ok(())
    }

    fn set_parameters_from_attrs(&mut self, params: &BaseFeeParameters) {
        match params.eip1559_params {
            EIP1559FeeParameters::Default => {
                self.base_fee_max_change_denominator =
                    DEFAULT_EIP1559_BASE_FEE_MAX_CHANGE_DENOMINATOR;
                self.elasticity_multiplier = DEFAULT_EIP1559_ELASTICITY_MULTIPLIER;
            }
            EIP1559FeeParameters::Custom {
                denominator,
                elasticity,
            } => {
                self.base_fee_max_change_denominator = denominator;
                self.elasticity_multiplier = elasticity;
            }
        }
        self.min_base_fee = params.min_base_fee;
    }

    fn encode_parameters_for_header(&self) -> Bytes {
        let mut out = Vec::with_capacity(17);

        // Header `extra_data` MUST be prepended with a version byte equal to 1.
        out.extend_from_slice(&[1u8]);

        // Conversion to base fee parameter form to reuse the encoding
        let eip1559_params = EIP1559FeeParameters::new(
            self.base_fee_max_change_denominator,
            self.elasticity_multiplier,
        )
        .encode()
        .to_be_bytes();

        // Bytes 1-9 are the EIP-1559 parameters.
        out.extend_from_slice(&eip1559_params);

        // Bytes 9-17 is the min base fee.
        out.extend_from_slice(&self.min_base_fee.to_be_bytes());

        out.into()
    }
}

#[cfg(any(feature = "test-doubles", test))]
mod test_doubles {
    use super::*;

    const ELASTICITY_MULTIPLIER: NonZeroU32 =
        NonZeroU32::new(2).expect("Supplied a non-zero value");
    const BASE_FEE_MAX_CHANGE_DENOMINATOR: NonZeroU32 =
        NonZeroU32::new(8).expect("Supplied a non-zero value");

    impl Default for JovianGasFee {
        fn default() -> Self {
            Self::new(ELASTICITY_MULTIPLIER, BASE_FEE_MAX_CHANGE_DENOMINATOR, 0)
        }
    }

    impl JovianGasFee {
        /// Creates a new [`Eip1559GasFee`] that always makes the gas target equal to gas limit.
        pub fn with_max_gas_target(mut self) -> Self {
            self.elasticity_multiplier = NonZeroU32::MIN;
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fee_is_not_changed_when_gas_used_matches_gas_target() {
        let gas_limit = 15_000_000;
        let parent_fee = 1;

        let actual_fee = JovianGasFee::default()
            .with_max_gas_target()
            .base_fee_per_gas(gas_limit, gas_limit, parent_fee);

        assert_eq!(actual_fee, parent_fee);
    }

    #[test]
    fn test_fee_is_increased_when_gas_used_exceeds_gas_target() {
        let gas_limit = 15_000_000;
        let gas_used = 8_500_000;
        let parent_fee = 2;

        let actual_fee = JovianGasFee::default().base_fee_per_gas(gas_limit, gas_used, parent_fee);

        assert!(actual_fee > parent_fee, "{actual_fee} > {parent_fee}");
    }

    #[test]
    fn test_fee_is_decreased_when_gas_used_falls_below_gas_target() {
        let gas_limit = 15_000_000;
        let gas_used = 6_500_000;
        let parent_fee = 200;

        let actual_fee = JovianGasFee::default().base_fee_per_gas(gas_limit, gas_used, parent_fee);

        assert!(actual_fee < parent_fee, "{actual_fee} < {parent_fee}");
    }
}
