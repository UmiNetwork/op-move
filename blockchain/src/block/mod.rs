//! The block module is responsible for the concerns of blocks such that it:
//!
//! * Defines the structure of Ethereum blocks.
//! * Implements an algorithm for producing its hashes.
//! * Declares a collection of blocks in the node.

mod gas;
mod hash;

// Safety: Unwraps allowed here because
// (1) in-memory backend is only used in tests
// (2) all unwraps come from `RwLock` poisoning, which should never happen
// if the rest of the code does not panic.
#[allow(clippy::unwrap_used)]
mod in_memory;

mod read;
mod write;

pub use {
    alloy::rpc::types::engine::ForkchoiceState,
    gas::{
        BaseFeeParameters, BaseGasFee, DEFAULT_EIP1559_BASE_FEE_MAX_CHANGE_DENOMINATOR,
        DEFAULT_EIP1559_ELASTICITY_MULTIPLIER, EIP1559FeeParameters, JovianGasFee,
    },
    hash::{BlockHash, UmiBlockHash},
    in_memory::{BlockMemory, ForkchoiceMemory, ReadBlockMemory},
    read::{BlockQueries, BlockResponse, in_memory::InMemoryBlockQueries},
    write::{Block, BlockRepository, ExtendedBlock, Header, in_memory::InMemoryBlockRepository},
};
