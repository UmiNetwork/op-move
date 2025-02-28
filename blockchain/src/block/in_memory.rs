use {
    crate::{
        block::{
            root::{BlockQueries, BlockRepository},
            BlockResponse, ExtendedBlock,
        },
        in_memory::SharedMemory,
    },
    moved_shared::primitives::B256,
    std::{collections::HashMap, convert::Infallible},
};

/// A storage for blocks that keeps data in memory.
///
/// The repository keeps data stored locally and its memory is not shared outside the struct. It
/// maintains a set of indices for efficient lookup.
#[derive(Debug)]
pub struct BlockMemory {
    /// Collection of blocks ordered by insertion.
    blocks: Vec<ExtendedBlock>,
    /// Map where key is a block hash and value is a position in the `blocks` vector.
    hashes: HashMap<B256, usize>,
    /// Map where key is a block height and value is a position in the `blocks` vector.
    heights: HashMap<u64, usize>,
}

impl Default for BlockMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockMemory {
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            hashes: HashMap::new(),
            heights: HashMap::new(),
        }
    }
}

impl BlockMemory {
    pub fn add(&mut self, block: ExtendedBlock) {
        let index = self.blocks.len();
        self.hashes.insert(block.hash, index);
        self.heights.insert(block.block.header.number, index);
        self.blocks.push(block);
    }

    pub fn by_hash(&self, hash: B256) -> Option<ExtendedBlock> {
        let index = *self.hashes.get(&hash)?;
        self.blocks.get(index).cloned()
    }

    pub fn by_height(&self, height: u64) -> Option<ExtendedBlock> {
        let index = *self.heights.get(&height)?;
        self.blocks.get(index).cloned()
    }
}

/// Block repository that works with in memory backing store [`BlockMemory`].
#[derive(Debug)]
pub struct InMemoryBlockRepository;

impl Default for InMemoryBlockRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryBlockRepository {
    pub fn new() -> Self {
        Self
    }
}

impl BlockRepository for InMemoryBlockRepository {
    type Err = Infallible;
    type Storage = SharedMemory;

    fn add(&mut self, mem: &mut Self::Storage, block: ExtendedBlock) -> Result<(), Self::Err> {
        mem.block_memory.add(block);
        Ok(())
    }

    fn by_hash(&self, mem: &Self::Storage, hash: B256) -> Result<Option<ExtendedBlock>, Self::Err> {
        Ok(mem.block_memory.by_hash(hash))
    }
}

/// Block query implementation that works with in memory backing store [`BlockMemory`].
#[derive(Debug)]
pub struct InMemoryBlockQueries;

impl BlockQueries for InMemoryBlockQueries {
    type Err = Infallible;
    type Storage = SharedMemory;

    fn by_hash(
        &self,
        mem: &Self::Storage,
        hash: B256,
        include_transactions: bool,
    ) -> Result<Option<BlockResponse>, Self::Err> {
        Ok(if include_transactions {
            mem.block_memory.by_hash(hash).map(|block| {
                let transactions = mem.transaction_memory.by_hashes(block.transaction_hashes());

                BlockResponse::from_block_with_transactions(block, transactions)
            })
        } else {
            mem.block_memory
                .by_hash(hash)
                .map(BlockResponse::from_block_with_transaction_hashes)
        })
    }

    fn by_height(
        &self,
        mem: &Self::Storage,
        height: u64,
        include_transactions: bool,
    ) -> Result<Option<BlockResponse>, Self::Err> {
        Ok(if include_transactions {
            mem.block_memory.by_height(height).map(|block| {
                let transactions = mem.transaction_memory.by_hashes(block.transaction_hashes());

                BlockResponse::from_block_with_transactions(block, transactions)
            })
        } else {
            mem.block_memory
                .by_height(height)
                .map(BlockResponse::from_block_with_transaction_hashes)
        })
    }
}

#[cfg(any(feature = "test-doubles", test))]
mod test_doubles {
    use super::*;

    impl BlockQueries for () {
        type Err = Infallible;
        type Storage = ();

        fn by_hash(
            &self,
            _: &Self::Storage,
            _: B256,
            _: bool,
        ) -> Result<Option<BlockResponse>, Self::Err> {
            Ok(None)
        }

        fn by_height(
            &self,
            _: &Self::Storage,
            _: u64,
            _: bool,
        ) -> Result<Option<BlockResponse>, Self::Err> {
            Ok(None)
        }
    }

    impl BlockRepository for () {
        type Err = ();
        type Storage = ();

        fn add(&mut self, _: &mut Self::Storage, _: ExtendedBlock) -> Result<(), Self::Err> {
            Ok(())
        }

        fn by_hash(&self, _: &Self::Storage, _: B256) -> Result<Option<ExtendedBlock>, Self::Err> {
            Ok(None)
        }
    }
}
