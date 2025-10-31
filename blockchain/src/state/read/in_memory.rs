use {
    crate::{
        block::ReadBlockMemory,
        in_memory::SharedMemoryReader,
        state::{EthTrieStateQueries, read::model::HashToStateRootIndex},
    },
    std::convert::Infallible,
    umi_shared::primitives::B256,
};

pub type InMemoryStateQueries<R = SharedMemoryReader, D = umi_state::InMemoryTrieDb> =
    EthTrieStateQueries<R, D>;

impl HashToStateRootIndex for SharedMemoryReader {
    type Err = Infallible;

    fn root_by_hash(&self, hash: B256) -> Result<Option<B256>, Self::Err> {
        Ok(self
            .block_memory
            .by_hash(hash)
            .map(|b| b.block.header.state_root))
    }

    fn push_state_root(&self, _block_hash: B256, _state_root: B256) -> Result<(), Self::Err> {
        Ok(())
    }
}
