//! See https://github.com/ethereum/execution-apis/blob/main/src/engine/
//! for specification of types.

use {
    ethers_core::types::{Bytes, H160, H256, U256, U64},
    serde::{Deserialize, Serialize},
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PayloadId(pub U64);

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
}
