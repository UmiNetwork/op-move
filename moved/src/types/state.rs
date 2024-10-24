//! Module defining types related to the state of op-move.
//! E.g. known block hashes, current head of the chain, etc.
//! Also defines the messages the State Actor (which manages the state)
//! accepts.

use {
    super::engine_api::GetPayloadResponseV3,
    crate::{
        block::Header,
        primitives::{ToU64, B2048, B256, U256, U64},
        types::engine_api::{PayloadAttributesV3, PayloadId},
    },
    alloy::consensus::transaction::TxEnvelope,
    tokio::sync::oneshot,
};

#[derive(Debug)]
pub enum StateMessage {
    UpdateHead {
        block_hash: B256,
    },
    StartBlockBuild {
        payload_attributes: PayloadAttributesV3,
        response_channel: oneshot::Sender<PayloadId>,
    },
    GetPayload {
        id: PayloadId,
        response_channel: oneshot::Sender<Option<GetPayloadResponseV3>>,
    },
    GetPayloadByBlockHash {
        block_hash: B256,
        response_channel: oneshot::Sender<Option<GetPayloadResponseV3>>,
    },
    AddTransaction {
        tx: TxEnvelope,
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
