//! The block module is responsible for the concerns of blocks such that it:
//!
//! * Defines the structure of Ethereum blocks.
//! * Implements an algorithm for producing its hashes.
//! * Declares a collection of blocks in the node.

mod hash;
mod root;

pub use {
    hash::{BlockHash, MovedBlockHash},
    root::{Block, BlockWithHash, Header},
};
