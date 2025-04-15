use {
    crate::block::BlockResponse,
    moved_shared::primitives::{B256, U256},
    std::fmt::Debug,
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

    fn by_height(
        &self,
        storage: &Self::Storage,
        height: u64,
        include_transactions: bool,
    ) -> Result<Option<BlockResponse>, Self::Err>;
}

pub trait BlockRepository: Debug {
    /// The associated error type for the backing storage access operation.
    type Err: Debug;
    /// The backing storage access handle type.
    type Storage;

    fn add(&mut self, storage: &mut Self::Storage, block: ExtendedBlock) -> Result<(), Self::Err>;

    fn by_hash(
        &self,
        storage: &Self::Storage,
        hash: B256,
    ) -> Result<Option<ExtendedBlock>, Self::Err>;
}

pub type Header = alloy::consensus::Header;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
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

    pub fn transaction_hashes(&self) -> impl Iterator<Item = B256> + use<'_> {
        self.block.transactions.iter().copied()
    }
}

/// TODO: Add withdrawals
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Block {
    pub header: Header,
    pub transactions: Vec<B256>,
}

impl Block {
    pub fn new(header: Header, transactions: Vec<B256>) -> Self {
        Self {
            header,
            transactions,
        }
    }

    pub fn with_hash(self, hash: B256) -> ExtendedBlock {
        ExtendedBlock::new(hash, U256::ZERO, self)
    }
}
