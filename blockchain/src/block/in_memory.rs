use {
    crate::{
        block::{ExtendedBlock, ForkchoiceState},
        payload::PayloadId,
    },
    std::{
        collections::HashMap,
        sync::{Arc, RwLock},
    },
    umi_shared::primitives::B256,
};

#[derive(Debug, Default, Clone)]
pub struct ForkchoiceMemory {
    state: Arc<RwLock<ForkchoiceState>>,
}

impl ForkchoiceMemory {
    pub fn get(&self) -> ForkchoiceState {
        *self.state.read().unwrap()
    }

    pub fn set(&self, mut new_state: ForkchoiceState) {
        let mut old_state = self.state.write().unwrap();
        std::mem::swap(&mut new_state, &mut old_state);
    }
}

/// A storage for blocks that keeps data in memory.
///
/// The repository keeps data stored locally and its memory is not shared outside the struct. It
/// maintains a set of indices for efficient lookup.
#[derive(Debug, Default, Clone)]
pub struct BlockMemory {
    hashes: Arc<RwLock<HashMap<B256, Arc<ExtendedBlock>>>>,
    heights: Arc<RwLock<HashMap<u64, B256>>>,
    payload_ids: Arc<RwLock<HashMap<PayloadId, Arc<ExtendedBlock>>>>,
}

impl BlockMemory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, block: ExtendedBlock) {
        let block = Arc::new(block);
        self.hashes
            .write()
            .unwrap()
            .insert(block.hash, block.clone());
        self.heights
            .write()
            .unwrap()
            .insert(block.block.header.number, block.hash);
        self.payload_ids
            .write()
            .unwrap()
            .insert(block.payload_id, block.clone());
    }
}

pub trait ReadBlockMemory {
    fn by_hash(&self, hash: B256) -> Option<ExtendedBlock>;
    fn by_payload_id(&self, payload_id: PayloadId) -> Option<ExtendedBlock>;
    fn height_to_hash(&self, height: u64) -> B256;
}

impl ReadBlockMemory for BlockMemory {
    fn by_hash(&self, hash: B256) -> Option<ExtendedBlock> {
        self.hashes
            .read()
            .unwrap()
            .get(&hash)
            .map(|b| ExtendedBlock::clone(b))
    }

    fn by_payload_id(&self, payload_id: PayloadId) -> Option<ExtendedBlock> {
        self.payload_ids
            .read()
            .unwrap()
            .get(&payload_id)
            .map(|b| ExtendedBlock::clone(b))
    }

    fn height_to_hash(&self, height: u64) -> B256 {
        self.heights
            .read()
            .unwrap()
            .get(&height)
            .copied()
            .expect("Should only ask for heights that exist")
    }
}
