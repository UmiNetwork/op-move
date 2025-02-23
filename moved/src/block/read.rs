use {
    crate::{block::ExtendedBlock, transaction::ExtendedTransaction},
    alloy::{eips::eip4895::Withdrawals, network::primitives::BlockTransactions},
    op_alloy::{consensus::OpTxEnvelope, network::eip2718::Encodable2718},
};

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
                // TODO: review fields below
                total_difficulty: None,
                size: None,
            },
            uncles: Vec::new(),
            withdrawals: Some(Withdrawals(Vec::new())),
        })
    }

    pub fn from_block_with_transaction_hashes(block: ExtendedBlock) -> Self {
        Self::new(
            BlockTransactions::Hashes(
                block
                    .block
                    .transactions
                    .iter()
                    .map(OpTxEnvelope::trie_hash)
                    .collect(),
            ),
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
