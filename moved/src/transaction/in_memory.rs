use {
    crate::{
        transaction::{Transaction, TransactionQueries, TransactionRepository},
        types::state::TransactionResponse,
    },
    alloy::eips::eip2718::Encodable2718,
    moved_shared::primitives::B256,
    std::convert::Infallible,
};

#[derive(Debug)]
pub struct TransactionMemory {
    txs: std::collections::HashMap<B256, Transaction>,
}

impl TransactionMemory {
    pub fn add(&mut self, tx: Transaction) {
        self.txs.insert(tx.0.trie_hash(), tx);
    }

    pub fn by_hash(&self, hash: B256) -> Option<Transaction> {
        self.txs.get(&hash).cloned()
    }
}

#[derive(Debug)]
pub struct InMemoryTransactionRepository;

impl Default for InMemoryTransactionRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryTransactionRepository {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug)]
pub struct InMemoryTransactionQueries;

impl TransactionRepository for InMemoryTransactionRepository {
    type Err = Infallible;
    type Storage = TransactionMemory;

    fn add(&mut self, storage: &mut Self::Storage, tx: Transaction) -> Result<(), Self::Err> {
        storage.add(tx);
        Ok(())
    }
}

impl TransactionQueries for InMemoryTransactionQueries {
    type Err = Infallible;
    type Storage = TransactionMemory;

    fn by_hash(
        &self,
        storage: &Self::Storage,
        hash: B256,
    ) -> Result<Option<TransactionResponse>, Self::Err> {
        Ok(storage.by_hash(hash).map(TransactionResponse::from))
    }
}
