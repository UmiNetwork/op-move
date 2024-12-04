use {crate::block::Header, alloy::primitives::B256};

/// Represents an algorithm that computes the block hash.
pub trait BlockHash {
    /// Computes a block hash.
    fn block_hash(&self, header: &Header) -> B256;
}

/// Computes the block hash following the Ethereum specification.
pub struct MovedBlockHash;

impl BlockHash for MovedBlockHash {
    fn block_hash(&self, header: &Header) -> B256 {
        header.hash_slow()
    }
}

#[cfg(any(feature = "test-doubles", test))]
mod tests {
    use super::*;

    impl BlockHash for B256 {
        fn block_hash(&self, _header: &Header) -> B256 {
            *self
        }
    }
}
