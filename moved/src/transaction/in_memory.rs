use {
    crate::{
        in_memory::SharedMemory,
        transaction::{Transaction, TransactionQueries, TransactionRepository},
        types::state::TransactionResponse,
    },
    alloy::eips::eip2718::Encodable2718,
    moved_shared::primitives::B256,
    std::{collections::HashMap, convert::Infallible},
};

#[derive(Debug, Default)]
pub struct TransactionMemory {
    txs: HashMap<B256, Transaction>,
}

impl TransactionMemory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, tx: Transaction) {
        self.txs.insert(tx.0.trie_hash(), tx);
    }

    pub fn extend(&mut self, tx: impl IntoIterator<Item = Transaction>) {
        self.txs
            .extend(tx.into_iter().map(|tx| (tx.0.trie_hash(), tx)));
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
    type Storage = SharedMemory;

    fn add(
        &mut self,
        storage: &mut Self::Storage,
        transaction: Transaction,
    ) -> Result<(), Self::Err> {
        storage.transaction_memory.add(transaction);
        Ok(())
    }

    fn extend(
        &mut self,
        storage: &mut Self::Storage,
        transactions: impl IntoIterator<Item = Transaction>,
    ) -> Result<(), Self::Err> {
        storage.transaction_memory.extend(transactions);
        Ok(())
    }
}

impl TransactionQueries for InMemoryTransactionQueries {
    type Err = Infallible;
    type Storage = SharedMemory;

    fn by_hash(
        &self,
        storage: &Self::Storage,
        hash: B256,
    ) -> Result<Option<TransactionResponse>, Self::Err> {
        Ok(storage
            .transaction_memory
            .by_hash(hash)
            .map(TransactionResponse::from))
    }
}

#[cfg(any(feature = "test-doubles", test))]
mod test_doubles {
    use super::*;

    impl TransactionQueries for () {
        type Err = Infallible;
        type Storage = ();

        fn by_hash(
            &self,
            _: &Self::Storage,
            _: B256,
        ) -> Result<Option<TransactionResponse>, Self::Err> {
            Ok(None)
        }
    }

    impl TransactionRepository for () {
        type Err = Infallible;
        type Storage = ();

        fn add(&mut self, _: &mut Self::Storage, _: Transaction) -> Result<(), Self::Err> {
            Ok(())
        }

        fn extend(
            &mut self,
            _: &mut Self::Storage,
            _: impl IntoIterator<Item = Transaction>,
        ) -> Result<(), Self::Err> {
            Ok(())
        }
    }
}
