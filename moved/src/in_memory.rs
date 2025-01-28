use crate::{block::BlockMemory, transaction::TransactionMemory};

#[derive(Debug, Default)]
pub struct SharedMemory {
    pub block_memory: BlockMemory,
    pub transaction_memory: TransactionMemory,
}

impl SharedMemory {
    pub fn new() -> Self {
        Self::default()
    }
}
