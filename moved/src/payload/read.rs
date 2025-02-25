use {
    crate::{block::ExtendedBlock, payload::id::PayloadId},
    alloy::eips::eip2718::Encodable2718,
    moved_shared::primitives::{Address, Bytes, B2048, B256, U256, U64},
    std::fmt::Debug,
};

pub type Withdrawal = alloy::rpc::types::Withdrawal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadResponse {
    pub execution_payload: ExecutionPayload,
    pub block_value: U256,
    pub blobs_bundle: BlobsBundle,
    pub should_override_builder: bool,
    pub parent_beacon_block_root: B256,
}

impl PayloadResponse {
    pub fn from_block(value: ExtendedBlock) -> Self {
        Self {
            parent_beacon_block_root: value
                .block
                .header
                .parent_beacon_block_root
                .unwrap_or_default(),
            block_value: value.value,
            execution_payload: ExecutionPayload::from_block(value),
            blobs_bundle: Default::default(),
            should_override_builder: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecutionPayload {
    pub parent_hash: B256,
    pub fee_recipient: Address,
    pub state_root: B256,
    pub receipts_root: B256,
    pub logs_bloom: B2048,
    pub prev_randao: B256,
    pub block_number: U64,
    pub gas_limit: U64,
    pub gas_used: U64,
    pub timestamp: U64,
    pub extra_data: Bytes,
    pub base_fee_per_gas: U256,
    pub block_hash: B256,
    pub transactions: Vec<Bytes>,
    pub withdrawals: Vec<Withdrawal>,
    pub blob_gas_used: U64,
    pub excess_blob_gas: U64,
}

impl ExecutionPayload {
    pub fn from_block(value: ExtendedBlock) -> Self {
        let transactions = value
            .block
            .transactions
            .into_iter()
            .map(|tx| {
                let capacity = tx.eip2718_encoded_length();
                let mut bytes = Vec::with_capacity(capacity);
                tx.encode_2718(&mut bytes);
                bytes.into()
            })
            .collect();

        Self {
            block_hash: value.hash,
            parent_hash: value.block.header.parent_hash,
            fee_recipient: value.block.header.beneficiary,
            state_root: value.block.header.state_root,
            receipts_root: value.block.header.receipts_root,
            logs_bloom: value.block.header.logs_bloom.0,
            prev_randao: value.block.header.mix_hash,
            block_number: U64::from(value.block.header.number),
            gas_limit: U64::from(value.block.header.gas_limit),
            gas_used: U64::from(value.block.header.gas_used),
            timestamp: U64::from(value.block.header.timestamp),
            extra_data: value.block.header.extra_data,
            base_fee_per_gas: U256::from(value.block.header.base_fee_per_gas.unwrap_or_default()),
            transactions,
            withdrawals: Vec::new(), // TODO: withdrawals
            blob_gas_used: U64::from(value.block.header.blob_gas_used.unwrap_or_default()),
            excess_blob_gas: U64::from(value.block.header.excess_blob_gas.unwrap_or_default()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BlobsBundle {
    pub commitments: Vec<Bytes>,
    pub proofs: Vec<Bytes>,
    pub blobs: Vec<Bytes>,
}

pub trait PayloadQueries {
    type Err: Debug;
    type Storage;

    fn by_hash(
        &self,
        storage: &Self::Storage,
        block_hash: B256,
    ) -> Result<Option<PayloadResponse>, Self::Err>;

    fn by_id(
        &self,
        storage: &Self::Storage,
        id: PayloadId,
    ) -> Result<Option<PayloadResponse>, Self::Err>;
}
