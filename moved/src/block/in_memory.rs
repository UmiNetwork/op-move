use {
    crate::{
        block::{root::BlockRepository, BlockWithHash},
        primitives::B256,
    },
    std::collections::HashMap,
};

/// Block repository that keeps data in memory.
///
/// The repository keeps data stored locally and its memory is not shared outside the struct.
#[derive(Debug)]
pub struct InMemoryBlockRepository {
    /// Collection of blocks ordered by insertion.
    blocks: Vec<BlockWithHash>,
    /// Map where key is a block hash and value is a position in the `blocks` vector.
    hashes: HashMap<B256, usize>,
}

impl Default for InMemoryBlockRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryBlockRepository {
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            hashes: HashMap::new(),
        }
    }
}

impl BlockRepository for InMemoryBlockRepository {
    fn add(&mut self, block: BlockWithHash) {
        let index = self.blocks.len();
        self.hashes.insert(block.hash, index);
        self.blocks.push(block);
    }

    fn by_hash(&self, hash: B256) -> Option<BlockWithHash> {
        let index = *self.hashes.get(&hash)?;
        self.blocks.get(index).cloned()
    }
}
