//! Module defining types related to the state of op-move.
//! E.g. known block hashes, current head of the chain, etc.
//! Also defines the messages the State Actor (which manages the state)
//! accepts.

use {
    crate::{
        block::{ExtendedBlock, Header},
        primitives::{Address, Bytes, ToU64, B2048, B256, U256, U64},
        state_actor::NewPayloadIdInput,
    },
    alloy::{consensus::transaction::TxEnvelope, eips::BlockNumberOrTag},
    alloy_rlp::Encodable,
    tokio::sync::oneshot,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Payload {
    pub timestamp: U64,
    pub prev_randao: B256,
    pub suggested_fee_recipient: Address,
    pub withdrawals: Vec<Withdrawal>,
    pub parent_beacon_block_root: B256,
    pub transactions: Vec<Bytes>,
    pub gas_limit: U64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Withdrawal {
    pub index: U64,
    pub validator_index: U64,
    pub address: Address,
    pub amount: U64,
}

pub type PayloadId = U64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadResponse {
    pub execution_payload: ExecutionPayload,
    pub block_value: U256,
    pub blobs_bundle: BlobsBundle,
    pub should_override_builder: bool,
    pub parent_beacon_block_root: B256,
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BlobsBundle {
    pub commitments: Vec<Bytes>,
    pub proofs: Vec<Bytes>,
    pub blobs: Vec<Bytes>,
}

#[derive(Debug)]
pub enum StateMessage {
    UpdateHead {
        block_hash: B256,
    },
    StartBlockBuild {
        payload_attributes: Payload,
        response_channel: oneshot::Sender<PayloadId>,
    },
    GetPayload {
        id: PayloadId,
        response_channel: oneshot::Sender<Option<PayloadResponse>>,
    },
    GetPayloadByBlockHash {
        block_hash: B256,
        response_channel: oneshot::Sender<Option<PayloadResponse>>,
    },
    AddTransaction {
        tx: TxEnvelope,
    },
    ChainId {
        response_channel: oneshot::Sender<u64>,
    },
    GetBalance {
        address: Address,
        block_number: BlockNumberOrTag,
        response_channel: oneshot::Sender<u64>,
    },
}

#[derive(Debug)]
pub struct ExecutionOutcome {
    pub receipts_root: B256,
    pub state_root: B256,
    pub logs_bloom: B2048,
    pub gas_used: U64,
    pub total_tip: U256,
}

pub(crate) trait WithExecutionOutcome {
    fn with_execution_outcome(self, outcome: ExecutionOutcome) -> Self;
}

impl WithExecutionOutcome for Header {
    fn with_execution_outcome(self, outcome: ExecutionOutcome) -> Self {
        Self {
            state_root: outcome.state_root,
            receipts_root: outcome.receipts_root,
            logs_bloom: outcome.logs_bloom,
            gas_used: outcome.gas_used.to_u64(),
            ..self
        }
    }
}

pub(crate) trait ToPayloadIdInput<'a> {
    fn to_payload_id_input(&'a self, head: &'a B256) -> NewPayloadIdInput<'a>;
}

impl<'a> ToPayloadIdInput<'a> for Payload {
    fn to_payload_id_input(&'a self, head: &'a B256) -> NewPayloadIdInput<'a> {
        NewPayloadIdInput::new_v3(
            head,
            self.timestamp.into_limbs()[0],
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

trait ToWithdrawal {
    fn to_withdrawal(&self) -> alloy::eips::eip4895::Withdrawal;
}

impl ToWithdrawal for Withdrawal {
    fn to_withdrawal(&self) -> alloy::eips::eip4895::Withdrawal {
        alloy::eips::eip4895::Withdrawal {
            index: self.index.into_limbs()[0],
            validator_index: self.validator_index.into_limbs()[0],
            address: self.address,
            amount: self.amount.into_limbs()[0],
        }
    }
}

impl From<ExtendedBlock> for PayloadResponse {
    fn from(value: ExtendedBlock) -> Self {
        PayloadResponse {
            parent_beacon_block_root: value.block.header.parent_beacon_block_root,
            block_value: value.value,
            execution_payload: ExecutionPayload::from(value),
            blobs_bundle: Default::default(),
            should_override_builder: false,
        }
    }
}

impl From<ExtendedBlock> for ExecutionPayload {
    fn from(value: ExtendedBlock) -> Self {
        let transactions = value
            .block
            .transactions
            .into_iter()
            .map(|tx| {
                let capacity = tx.length();
                let mut bytes = Vec::with_capacity(capacity);
                tx.encode(&mut bytes);
                bytes.into()
            })
            .collect();

        Self {
            block_hash: value.hash,
            parent_hash: value.block.header.parent_hash,
            fee_recipient: value.block.header.beneficiary,
            state_root: value.block.header.state_root,
            receipts_root: value.block.header.receipts_root,
            logs_bloom: value.block.header.logs_bloom,
            prev_randao: value.block.header.prev_randao,
            block_number: U64::from(value.block.header.number),
            gas_limit: U64::from(value.block.header.gas_limit),
            gas_used: U64::from(value.block.header.gas_used),
            timestamp: U64::from(value.block.header.timestamp),
            extra_data: value.block.header.extra_data,
            base_fee_per_gas: value.block.header.base_fee_per_gas,
            transactions,
            withdrawals: Vec::new(), // TODO: withdrawals
            blob_gas_used: U64::from(value.block.header.blob_gas_used),
            excess_blob_gas: U64::from(value.block.header.excess_blob_gas),
        }
    }
}

pub(crate) trait WithPayloadAttributes {
    fn with_payload_attributes(self, payload: Payload) -> Self;
}

impl WithPayloadAttributes for Header {
    fn with_payload_attributes(self, payload: Payload) -> Self {
        Self {
            beneficiary: payload.suggested_fee_recipient,
            gas_limit: payload.gas_limit.to_u64(),
            timestamp: payload.timestamp.to_u64(),
            prev_randao: payload.prev_randao,
            parent_beacon_block_root: payload.parent_beacon_block_root,
            ..self
        }
    }
}
