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
        let Some(block) = self.block_memory.by_hash(hash) else {
            return Ok(None);
        };

        if block.block.header.number == 0 {
            // For the genesis block always return the config state root.
            let state_root = umi_genesis::config::GenesisConfig::default().initial_state_root;
            return Ok(Some(state_root));
        }

        Ok(Some(block.block.header.state_root))
    }

    fn push_state_root(&self, _block_hash: B256, _state_root: B256) -> Result<(), Self::Err> {
        Ok(())
    }
}
