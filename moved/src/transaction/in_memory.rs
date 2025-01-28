use {
    crate::{
        in_memory::SharedMemory,
        transaction::{ExtendedTransaction, TransactionQueries, TransactionRepository},
        types::state::TransactionResponse,
    },
    moved_shared::primitives::B256,
    std::{collections::HashMap, convert::Infallible},
};

#[derive(Debug, Default)]
pub struct TransactionMemory {
    transactions: HashMap<B256, ExtendedTransaction>,
}

impl TransactionMemory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn extend(&mut self, tx: impl IntoIterator<Item = ExtendedTransaction>) {
        self.transactions
            .extend(tx.into_iter().map(|tx| (tx.hash(), tx)));
    }

    pub fn by_hash(&self, hash: B256) -> Option<ExtendedTransaction> {
        self.transactions.get(&hash).cloned()
    }
}

#[derive(Debug, Default)]
pub struct InMemoryTransactionRepository;

impl InMemoryTransactionRepository {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Default)]
pub struct InMemoryTransactionQueries;

impl InMemoryTransactionQueries {
    pub fn new() -> Self {
        Self
    }
}

impl TransactionRepository for InMemoryTransactionRepository {
    type Err = Infallible;
    type Storage = SharedMemory;

    fn extend(
        &mut self,
        storage: &mut Self::Storage,
        transactions: impl IntoIterator<Item = ExtendedTransaction>,
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

        fn extend(
            &mut self,
            _: &mut Self::Storage,
            _: impl IntoIterator<Item = ExtendedTransaction>,
        ) -> Result<(), Self::Err> {
            Ok(())
        }
    }
}
