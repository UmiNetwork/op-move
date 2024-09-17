use crate::block::{root::BlockRepository, BlockWithHash};

#[derive(Debug)]
pub struct InMemoryBlockRepository {
    blocks: Vec<BlockWithHash>,
}

impl InMemoryBlockRepository {
    pub fn new() -> Self {
        Self { blocks: Vec::new() }
    }
}

impl BlockRepository for InMemoryBlockRepository {
    fn add(&mut self, block: BlockWithHash) {
        self.blocks.push(block);
    }
}
