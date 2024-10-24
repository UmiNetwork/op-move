use {
    crate::block::Header,
    alloy::{
        primitives::{Keccak256, B256},
        rlp::Encodable,
    },
};

/// Represents an algorithm that computes the block hash.
pub trait BlockHash {
    /// Computes a block hash.
    fn block_hash(&self, header: &Header) -> B256;
}

/// Computes the block hash following the Ethereum specification.
pub struct MovedBlockHash;

impl BlockHash for MovedBlockHash {
    fn block_hash(&self, header: &Header) -> B256 {
        let mut hasher = Keccak256::new();
        let mut buf = Vec::with_capacity(header.length());
        header.encode(&mut buf);
        hasher.update(buf);
        hasher.finalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl BlockHash for B256 {
        fn block_hash(&self, _header: &Header) -> B256 {
            *self
        }
    }
}
