//! This module is concerned about calculating fees charged for gas usage.

use std::{cmp::Ordering, num::NonZeroU32};

#[cfg(feature = "op-upgrade")]
use alloy::primitives::{Bytes, U64};

pub const DEFAULT_EIP1559_ELASTICITY_MULTIPLIER: NonZeroU32 =
    NonZeroU32::new(6).expect("Supplied a non-zero value");
pub const DEFAULT_EIP1559_BASE_FEE_MAX_CHANGE_DENOMINATOR: NonZeroU32 =
    NonZeroU32::new(250).expect("Supplied a non-zero value");

#[cfg(feature = "op-upgrade")]
/// Represents base fee parameters as they are passed from payload attributes
/// or block headers. This is different from [`Eip1559GasFee`] in the sense
/// that it doesn't represent the current state of the blockchain, but a
/// change, and thus e.g. the difference in instantiation logic.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum BaseFeeParameters {
    #[default]
    Default,
    Custom {
        denominator: NonZeroU32,
        elasticity: NonZeroU32,
    },
}

#[cfg(feature = "op-upgrade")]
impl BaseFeeParameters {
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
            BaseFeeParameters::Default => (
                DEFAULT_EIP1559_BASE_FEE_MAX_CHANGE_DENOMINATOR.get().into(),
                DEFAULT_EIP1559_ELASTICITY_MULTIPLIER.get().into(),
            ),
            BaseFeeParameters::Custom {
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

    #[cfg(feature = "op-upgrade")]
    fn set_parameters_from_extra_data(
        &mut self,
        extra_data: Bytes,
    ) -> Result<(), umi_shared::error::Error>;

    #[cfg(feature = "op-upgrade")]
    fn set_parameters_from_attrs(&mut self, eip1559_params: &BaseFeeParameters);

    #[cfg(feature = "op-upgrade")]
    fn encode_parameters_for_header(&self) -> Bytes;
}

/// Calculates base fee per gas according to the Ethereum model based on EIP-1559.
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
pub struct Eip1559GasFee {
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
}

impl Eip1559GasFee {
    /// Sets up the base fee per gas calculation with given parameters.
    ///
    /// # Panics
    /// If either `elasticity_multiplier` or `base_fee_max_change_denominator` is zero.
    pub fn new(
        elasticity_multiplier: NonZeroU32,
        base_fee_max_change_denominator: NonZeroU32,
    ) -> Self {
        Self {
            elasticity_multiplier,
            base_fee_max_change_denominator,
        }
    }
}

impl BaseGasFee for Eip1559GasFee {
    fn base_fee_per_gas(
        &self,
        parent_gas_limit: u64,
        parent_gas_used: u64,
        parent_base_fee_per_gas: u64,
    ) -> u64 {
        let gas_target = parent_gas_limit / self.elasticity_multiplier.get() as u64;

        match parent_gas_used.cmp(&gas_target) {
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
        }
    }

    #[cfg(feature = "op-upgrade")]
    fn set_parameters_from_extra_data(
        &mut self,
        extra_data: Bytes,
    ) -> Result<(), umi_shared::error::Error> {
        // OP uses this field for dynamic EIP-1559 parameters only. The format is
        // <https://specs.optimism.io/protocol/holocene/exec-engine.html#eip-1559-parameters-in-block-header>
        if extra_data.len() != 9 {
            return Err(umi_shared::error::Error::extra_data_invariant_violation());
        };

        // As during block build the parameters are parsed from a byte-encoded field
        // in payload attributes, we have to do some conversions to read it from the
        // block header, most importantly skipping the version byte that is present
        // in the header, but absent from the attributes
        let encoded = U64::from_be_slice(&extra_data.slice(1..9));
        let params = BaseFeeParameters::decode(encoded)?;
        self.set_parameters_from_attrs(&params);
        Ok(())
    }

    #[cfg(feature = "op-upgrade")]
    fn set_parameters_from_attrs(&mut self, eip1559_params: &BaseFeeParameters) {
        match eip1559_params {
            BaseFeeParameters::Default => {
                self.base_fee_max_change_denominator =
                    DEFAULT_EIP1559_BASE_FEE_MAX_CHANGE_DENOMINATOR;
                self.elasticity_multiplier = DEFAULT_EIP1559_ELASTICITY_MULTIPLIER;
            }
            BaseFeeParameters::Custom {
                denominator,
                elasticity,
            } => {
                self.base_fee_max_change_denominator = *denominator;
                self.elasticity_multiplier = *elasticity;
            }
        }
    }

    #[cfg(feature = "op-upgrade")]
    fn encode_parameters_for_header(&self) -> Bytes {
        let mut out = Vec::with_capacity(9);

        // Header `extra_data` should be prepended with a 0 version byte
        out.extend_from_slice(&[0u8]);

        // Conversion to base fee parameter form to reuse the encoding
        let params = BaseFeeParameters::new(
            self.base_fee_max_change_denominator,
            self.elasticity_multiplier,
        )
        .encode()
        .to_be_bytes();
        out.extend_from_slice(&params);

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

    impl Default for Eip1559GasFee {
        fn default() -> Self {
            Self::new(ELASTICITY_MULTIPLIER, BASE_FEE_MAX_CHANGE_DENOMINATOR)
        }
    }

    impl Eip1559GasFee {
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

        let actual_fee = Eip1559GasFee::default()
            .with_max_gas_target()
            .base_fee_per_gas(gas_limit, gas_limit, parent_fee);

        assert_eq!(actual_fee, parent_fee);
    }

    #[test]
    fn test_fee_is_increased_when_gas_used_exceeds_gas_target() {
        let gas_limit = 15_000_000;
        let gas_used = 8_500_000;
        let parent_fee = 2;

        let actual_fee = Eip1559GasFee::default().base_fee_per_gas(gas_limit, gas_used, parent_fee);

        assert!(actual_fee > parent_fee, "{actual_fee} > {parent_fee}");
    }

    #[test]
    fn test_fee_is_decreased_when_gas_used_falls_below_gas_target() {
        let gas_limit = 15_000_000;
        let gas_used = 6_500_000;
        let parent_fee = 200;

        let actual_fee = Eip1559GasFee::default().base_fee_per_gas(gas_limit, gas_used, parent_fee);

        assert!(actual_fee < parent_fee, "{actual_fee} < {parent_fee}");
    }
}
