//! Module defining types related to the state of op-move.
//! E.g. known block hashes, current head of the chain, etc.
//! Also defines the messages the State Actor (which manages the state)
//! accepts.

use {
    crate::types::engine_api::{PayloadAttributesV3, PayloadId},
    ethers_core::types::H256,
    tokio::sync::oneshot,
};

#[derive(Debug)]
pub enum StateMessage {
    UpdateHead {
        block_hash: H256,
    },
    StartBlockBuild {
        payload: PayloadAttributesV3,
        response_channel: oneshot::Sender<PayloadId>,
    },
    // TODO: remove this in favour of generating our own PayloadIds
    SetPayloadId {
        id: PayloadId,
    },
}
