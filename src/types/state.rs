//! Module defining types related to the state of op-move.
//! E.g. known block hashes, current head of the chain, etc.
//! Also defines the messages the State Actor (which manages the state)
//! accepts.

use {
    super::engine_api::GetPayloadResponseV3,
    crate::types::engine_api::{PayloadAttributesV3, PayloadId},
    alloy_consensus::transaction::TxEnvelope,
    ethers_core::types::{Bytes, H256, U64},
    tokio::sync::oneshot,
};

#[derive(Debug)]
pub enum StateMessage {
    UpdateHead {
        block_hash: H256,
    },
    StartBlockBuild {
        payload_attributes: PayloadAttributesV3,
        response_channel: oneshot::Sender<PayloadId>,
    },
    // TODO: remove this in favour of generating our own PayloadIds
    SetPayloadId {
        id: PayloadId,
    },
    GetPayload {
        id: PayloadId,
        response_channel: oneshot::Sender<Option<GetPayloadResponseV3>>,
    },
    GetPayloadByBlockHash {
        block_hash: H256,
        response_channel: oneshot::Sender<Option<GetPayloadResponseV3>>,
    },
    AddTransaction {
        tx: TxEnvelope,
    },
    // Tells the state to remember a new block hash/height correspondence.
    // TODO: should be able to remove in the future
    NewBlock {
        block_hash: H256,
        block_height: U64,
    },
}

#[derive(Debug)]
pub struct ExecutionOutcome {
    pub receipts_root: H256,
    pub state_root: H256,
    pub logs_bloom: Bytes,
    pub gas_used: U64,
}
