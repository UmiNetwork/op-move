//! See https://github.com/ethereum/execution-apis/blob/main/src/engine/
//! for specification of types.

use {
    crate::state_actor::NewPayloadIdInput,
    ethers_core::types::{Bytes, Withdrawal, H160, H256, U256, U64},
    serde::{Deserialize, Serialize},
    std::str::FromStr,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPayloadV1 {
    pub parent_hash: H256,
    pub fee_recipient: H160,
    pub state_root: H256,
    pub receipts_root: H256,
    pub logs_bloom: Bytes,
    pub prev_randao: H256,
    pub block_number: U64,
    pub gas_limit: U64,
    pub gas_used: U64,
    pub timestamp: U64,
    pub extra_data: Bytes,
    pub base_fee_per_gas: U256,
    pub block_hash: H256,
    pub transactions: Vec<Bytes>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawalV1 {
    pub index: U64,
    pub validator_index: U64,
    pub address: H160,
    pub amount: U64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPayloadV2 {
    pub parent_hash: H256,
    pub fee_recipient: H160,
    pub state_root: H256,
    pub receipts_root: H256,
    pub logs_bloom: Bytes,
    pub prev_randao: H256,
    pub block_number: U64,
    pub gas_limit: U64,
    pub gas_used: U64,
    pub timestamp: U64,
    pub extra_data: Bytes,
    pub base_fee_per_gas: U256,
    pub block_hash: H256,
    pub transactions: Vec<Bytes>,
    pub withdrawals: Vec<WithdrawalV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPayloadV3 {
    pub parent_hash: H256,
    pub fee_recipient: H160,
    pub state_root: H256,
    pub receipts_root: H256,
    pub logs_bloom: Bytes,
    pub prev_randao: H256,
    pub block_number: U64,
    pub gas_limit: U64,
    pub gas_used: U64,
    pub timestamp: U64,
    pub extra_data: Bytes,
    pub base_fee_per_gas: U256,
    pub block_hash: H256,
    pub transactions: Vec<Bytes>,
    pub withdrawals: Vec<WithdrawalV1>,
    pub blob_gas_used: U64,
    pub excess_blob_gas: U64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ForkchoiceStateV1 {
    pub head_block_hash: H256,
    pub safe_block_hash: H256,
    pub finalized_block_hash: H256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PayloadAttributesV1 {
    pub timestamp: U64,
    pub prev_randao: H256,
    pub suggested_fee_recipient: H160,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PayloadAttributesV2 {
    pub timestamp: U64,
    pub prev_randao: H256,
    pub suggested_fee_recipient: H160,
    pub withdrawals: Vec<WithdrawalV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PayloadAttributesV3 {
    pub timestamp: U64,
    pub prev_randao: H256,
    pub suggested_fee_recipient: H160,
    pub withdrawals: Vec<WithdrawalV1>,
    pub parent_beacon_block_root: H256,
    pub transactions: Vec<Bytes>,
    pub gas_limit: U64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "U64", into = "String")]
pub struct PayloadId(pub U64);

impl FromStr for PayloadId {
    type Err = <U64 as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(U64::from_str(s)?))
    }
}

impl<U: Into<U64>> From<U> for PayloadId {
    fn from(value: U) -> Self {
        Self(value.into())
    }
}

impl From<PayloadId> for String {
    fn from(value: PayloadId) -> Self {
        let inner: u64 = value.0 .0[0];
        format!("{inner:#018x}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayloadStatusV1 {
    pub status: Status,
    pub latest_valid_hash: Option<H256>,
    #[serde(default)]
    pub validation_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status {
    Valid,
    Invalid,
    Syncing,
    Accepted,
    InvalidBlockHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BlobsBundleV1 {
    pub commitments: Vec<Bytes>,
    pub proofs: Vec<Bytes>,
    pub blobs: Vec<Bytes>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForkchoiceUpdatedResponseV1 {
    pub payload_status: PayloadStatusV1,
    pub payload_id: Option<PayloadId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPayloadResponseV3 {
    pub execution_payload: ExecutionPayloadV3,
    pub block_value: U256,
    pub blobs_bundle: BlobsBundleV1,
    pub should_override_builder: bool,
    pub parent_beacon_block_root: H256,
}

trait ToWithdrawal {
    fn to_withdrawal(&self) -> Withdrawal;
}

impl ToWithdrawal for WithdrawalV1 {
    fn to_withdrawal(&self) -> Withdrawal {
        Withdrawal {
            index: self.index,
            validator_index: self.validator_index,
            address: self.address,
            amount: U256::from(self.amount.as_u64()),
        }
    }
}

pub(crate) trait ToPayloadIdInput<'a> {
    fn to_payload_id_input(&'a self, head: &'a H256) -> NewPayloadIdInput<'a>;
}

impl<'a> ToPayloadIdInput<'a> for &'a PayloadAttributesV3 {
    fn to_payload_id_input(&'a self, head: &'a H256) -> NewPayloadIdInput<'a> {
        NewPayloadIdInput::new_v3(
            head,
            self.timestamp.as_u64(),
            &self.prev_randao,
            &self.suggested_fee_recipient,
        )
        .with_beacon_root(&self.parent_beacon_block_root)
        .with_withdrawals(
            self.withdrawals
                .iter()
                .map(ToWithdrawal::to_withdrawal)
                .collect::<Vec<_>>(),
        )
    }
}
