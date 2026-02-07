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
mod faucet_deposit;
mod mempool;
mod query;
mod queue;
#[cfg(test)]
mod tests;
#[cfg(any(feature = "test-doubles", test))]
mod uninit;

pub const L2_TO_L1_MESSAGE_PASSER_ADDRESS: alloy::primitives::Address =
    alloy::primitives::address!("0x4200000000000000000000000000000000000016");

pub const EMPTY_REQUESTS_HASH: alloy::primitives::B256 =
    alloy::primitives::b256!("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
