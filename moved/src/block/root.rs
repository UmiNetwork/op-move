use {
    crate::{
        primitives::{B256, U256},
        types::state::BlockResponse,
    },
    op_alloy::consensus::OpTxEnvelope,
    std::fmt::Debug,
};

pub trait BlockQueries: Debug {
    type Storage;

    fn by_hash(
        &self,
        storage: &Self::Storage,
        hash: B256,
        include_transactions: bool,
    ) -> Option<BlockResponse>;

    fn by_height(
        &self,
        storage: &Self::Storage,
        height: u64,
        include_transactions: bool,
    ) -> Option<BlockResponse>;
}

pub trait BlockRepository: Debug {
    type Storage;
    fn add(&mut self, storage: &mut Self::Storage, block: ExtendedBlock);
    fn by_hash(&self, storage: &Self::Storage, hash: B256) -> Option<ExtendedBlock>;
}

pub type Header = alloy::consensus::Header;

#[derive(Debug, Clone, Default)]
pub struct ExtendedBlock {
    /// The block hash is the output of keccak-256 algorithm with RLP encoded block header as input.
    pub hash: B256,
    /// The block value is total value in Wei expected to be received by the fee recipient. It is
    /// the gas paid on top of the base fee.
    ///
    /// The base fee is burned to prevent malicious behavior.
    ///
    /// Burning the base fee hinders a block producer's ability to manipulate transactions. For
    /// example, if block producers received the base fee, they could include their own transactions
    /// for free and raise the base fee for everyone else. Alternatively, they could refund the base
    /// fee to some users off-chain, leading to a more opaque and complex transaction fee market.
    pub value: U256,
    pub block: Block,
}

impl ExtendedBlock {
    pub fn new(hash: B256, value: U256, block: Block) -> Self {
        Self { hash, value, block }
    }

    pub fn with_value(mut self, value: U256) -> Self {
        self.value = value;
        self
    }
}

/// TODO: Add withdrawals
#[derive(Debug, Clone, Default)]
pub struct Block {
    pub header: Header,
    pub transactions: Vec<OpTxEnvelope>,
}

impl Block {
    pub fn new(header: Header, transactions: Vec<OpTxEnvelope>) -> Self {
        Self {
            header,
            transactions,
        }
    }

    pub fn with_hash(self, hash: B256) -> ExtendedBlock {
        ExtendedBlock::new(hash, U256::ZERO, self)
    }
}

/// A subset of the `Header` fields that are available while the transactions
/// in the block are being executed.
#[derive(Debug, Clone, Default)]
pub struct HeaderForExecution {
    pub number: u64,
    pub timestamp: u64,
    pub prev_randao: B256,
}
