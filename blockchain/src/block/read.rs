use {
    crate::{block::write::ExtendedBlock, transaction::ExtendedTransaction},
    alloy::{
        eips::eip4895::Withdrawals, network::primitives::BlockTransactions,
        rpc::types::engine::ForkchoiceState,
    },
    std::fmt::Debug,
    umi_shared::primitives::B256,
};
pub trait BlockQueries: Debug {
    /// The associated error type for the backing storage access operation.
    type Err: Debug;
    /// The backing storage access handle type.
    type Storage;

    fn by_hash(
        &self,
        storage: &Self::Storage,
        hash: B256,
        include_transactions: bool,
    ) -> Result<Option<BlockResponse>, Self::Err>;

    fn get_forkchoice_state(&self, storage: &Self::Storage) -> Result<ForkchoiceState, Self::Err>;

    fn height_to_hash(&self, storage: &Self::Storage, height: u64) -> Result<B256, Self::Err>;

    /// This function returns `true` if the given block hash `maybe_ancestor` is reachable
    /// via parent hash links in the blockchain starting from the block hash `head` or
    /// `maybe_ancestor == head`.
    /// Note: the default implementation is rather inefficient (it always walks the whole chain
    /// from `head` to `maybe_ancestor`); implementors of this trait should provide a more streamlined
    /// implementation (e.g. using a skip list).
    fn ancestor_check(
        &self,
        storage: &Self::Storage,
        maybe_ancestor: B256,
        head: B256,
    ) -> Result<bool, Self::Err> {
        if maybe_ancestor == head {
            return Ok(true);
        }

        // The given block hashes being unknown should be an error, but in the default implementation
        // we do not know anything about the error type so we cannot do that here.
        let Some(head_block) = self.by_hash(storage, head, false)? else {
            return Ok(false);
        };
        let Some(ancestor_block) = self.by_hash(storage, maybe_ancestor, false)? else {
            return Ok(false);
        };

        // Follow parent links until we reach the height of the supposed ancestor block.
        // Note: in the edge case that the given head is at a lower height than `maybe_ancestor`
        // the loop will be skipped because the condition will immediately be `false`. In this
        // case we will immediately compare the block hashes and they will be different
        // (because the blocks are at different heights); thus correctly producing `false`
        // as the response.
        let target_height = ancestor_block.0.header.number;
        let mut current_block = head_block;
        while target_height < current_block.0.header.number {
            // Failing to find a parent block should also be an error, but once again the
            // default implementation is not able to create errors.
            let Some(parent_block) =
                self.by_hash(storage, current_block.0.header.parent_hash, false)?
            else {
                return Ok(false);
            };
            current_block = parent_block;
        }

        // Once we know the block at the same height as the supposed ancestor
        // we can simply compare hashes to see if they are the same.
        Ok(current_block.0.header.hash == maybe_ancestor)
    }
}

impl<T: BlockQueries> BlockQueries for &T {
    type Err = T::Err;
    type Storage = T::Storage;

    fn by_hash(
        &self,
        storage: &Self::Storage,
        hash: B256,
        include_transactions: bool,
    ) -> Result<Option<BlockResponse>, Self::Err> {
        (*self).by_hash(storage, hash, include_transactions)
    }

    fn get_forkchoice_state(&self, storage: &Self::Storage) -> Result<ForkchoiceState, Self::Err> {
        (*self).get_forkchoice_state(storage)
    }

    fn height_to_hash(&self, storage: &Self::Storage, height: u64) -> Result<B256, Self::Err> {
        (*self).height_to_hash(storage, height)
    }
}

type RpcBlock = alloy::rpc::types::Block<RpcTransaction>;
type RpcTransaction = op_alloy::rpc_types::Transaction;

#[derive(Debug)]
pub struct BlockResponse(pub RpcBlock);

impl BlockResponse {
    fn new(transactions: BlockTransactions<RpcTransaction>, value: ExtendedBlock) -> Self {
        Self(RpcBlock {
            transactions,
            header: alloy::rpc::types::Header {
                hash: value.hash,
                inner: value.block.header,
                // Deprecated for PoS clients: <https://github.com/ethereum/execution-apis/pull/570>
                total_difficulty: None,
                size: Some(value.size),
            },
            uncles: Vec::new(),
            withdrawals: Some(Withdrawals(Vec::new())),
        })
    }

    pub fn from_block_with_transaction_hashes(block: ExtendedBlock) -> Self {
        Self::new(
            BlockTransactions::Hashes(block.block.transactions.clone()),
            block,
        )
    }

    pub fn from_block_with_transactions(
        block: ExtendedBlock,
        transactions: Vec<ExtendedTransaction>,
    ) -> Self {
        Self::new(
            BlockTransactions::Full(transactions.into_iter().map(RpcTransaction::from).collect()),
            block,
        )
    }
}

pub mod in_memory {
    use {
        crate::{
            block::{BlockResponse, ReadBlockMemory, read::BlockQueries},
            in_memory::SharedMemoryReader,
            transaction::ReadTransactionMemory,
        },
        alloy::rpc::types::engine::ForkchoiceState,
        std::convert::Infallible,
        umi_shared::primitives::B256,
    };

    /// Block query implementation that works with in memory backing store [`BlockMemory`].
    ///
    /// [`BlockMemory`]: crate::block::BlockMemory
    #[derive(Debug, Clone)]
    pub struct InMemoryBlockQueries;

    impl BlockQueries for InMemoryBlockQueries {
        type Err = Infallible;
        type Storage = SharedMemoryReader;

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

        fn get_forkchoice_state(&self, mem: &Self::Storage) -> Result<ForkchoiceState, Self::Err> {
            Ok(mem.forkchoice_memory.get())
        }

        fn height_to_hash(&self, mem: &Self::Storage, height: u64) -> Result<B256, Self::Err> {
            Ok(mem.block_memory.height_to_hash(height))
        }
    }
}

#[cfg(any(feature = "test-doubles", test))]
mod test_doubles {
    use {super::*, std::convert::Infallible};

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

        fn get_forkchoice_state(&self, _: &Self::Storage) -> Result<ForkchoiceState, Self::Err> {
            Ok(ForkchoiceState::default())
        }

        fn height_to_hash(&self, _: &Self::Storage, _: u64) -> Result<B256, Self::Err> {
            Ok(B256::default())
        }
    }
}
