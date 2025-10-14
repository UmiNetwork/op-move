use crate::{
    block::{BlockMemory, ForkchoiceMemory},
    transaction::{TransactionMemory, TransactionMemoryReader},
};

#[derive(Debug, Clone)]
pub struct SharedMemoryReader {
    pub block_memory: BlockMemory,
    pub transaction_memory: TransactionMemoryReader,
    pub forkchoice_memory: ForkchoiceMemory,
}

impl SharedMemoryReader {
    pub const fn new(
        block_memory: BlockMemory,
        transaction_memory: TransactionMemoryReader,
        forkchoice_memory: ForkchoiceMemory,
    ) -> Self {
        Self {
            block_memory,
            transaction_memory,
            forkchoice_memory,
        }
    }
}

#[derive(Debug)]
pub struct SharedMemory {
    pub block_memory: BlockMemory,
    pub transaction_memory: TransactionMemory,
    pub forkchoice_memory: ForkchoiceMemory,
}

impl SharedMemory {
    pub const fn new(
        block_memory: BlockMemory,
        transaction_memory: TransactionMemory,
        forkchoice_memory: ForkchoiceMemory,
    ) -> Self {
        Self {
            block_memory,
            transaction_memory,
            forkchoice_memory,
        }
    }
}

pub mod shared_memory {
    use crate::{
        block::{BlockMemory, ForkchoiceMemory},
        in_memory::{SharedMemory, SharedMemoryReader},
        transaction::{TransactionMemory, TransactionMemoryReader},
    };

    pub fn new() -> (SharedMemoryReader, SharedMemory) {
        let bm = BlockMemory::new();
        let fm = ForkchoiceMemory::default();
        let (r1, w1) = evmap::new();
        let tw = TransactionMemory::new(w1);
        let tr = TransactionMemoryReader::new(r1);
        let w = SharedMemory::new(bm.clone(), tw, fm.clone());
        let r = SharedMemoryReader::new(bm, tr, fm);

        (r, w)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::block::{ExtendedBlock, ReadBlockMemory},
    };

    #[test]
    fn test_block_reader_is_connected_to_block_writer() {
        let (r, mut w) = shared_memory::new();

        let block = ExtendedBlock::default();
        let block_hash = block.hash;
        w.block_memory.add(block);
        let actual_block = r.block_memory.by_hash(block_hash);
        let expected_block = Some(ExtendedBlock::default());

        assert_eq!(actual_block, expected_block);
    }
}
