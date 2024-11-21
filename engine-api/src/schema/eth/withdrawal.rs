use {
    moved::primitives::{Address, U256},
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Withdrawal {
    /// The unique identifier of the withdrawal.
    index: u64,
    /// The unique identifier of the validator who initiated the withdrawal.
    validator_index: u64,
    /// The address to which the withdrawn `amount` is sent.
    address: Address,
    /// A value of this withdrawal in GWei.
    amount: U256,
}
