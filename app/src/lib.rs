#[cfg(any(feature = "test-doubles", test))]
pub use uninit::Uninitialized;
pub use {
    actor::*,
    block_hash::HybridBlockHashCache,
    dependency::*,
    factory::{create, run},
    input::*,
    queue::CommandQueue,
};

pub mod factory;

pub(crate) mod input;

mod actor;
mod block_hash;
mod command;
mod dependency;
mod mempool;
mod query;
mod queue;
#[cfg(test)]
mod tests;
#[cfg(any(feature = "test-doubles", test))]
mod uninit;

#[cfg(feature = "op-upgrade")]
pub const L2_TO_L1_MESSAGE_PASSER_ADDRESS: alloy::primitives::Address =
    alloy::primitives::address!("0x4200000000000000000000000000000000000016");
