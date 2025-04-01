use {
    crate::receipt::{
        ExtendedReceipt, ReceiptQueries, TransactionReceipt, write::ReceiptRepository,
    },
    moved_shared::primitives::B256,
    std::{collections::HashMap, convert::Infallible},
};

#[derive(Debug)]
pub struct ReceiptMemory {
    receipts: HashMap<B256, ExtendedReceipt>,
}

impl Default for ReceiptMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl ReceiptMemory {
    pub fn new() -> Self {
        Self {
            receipts: HashMap::new(),
        }
    }

    pub fn contains(&self, transaction_hash: B256) -> bool {
        self.receipts.contains_key(&transaction_hash)
    }

    pub fn extend(&mut self, receipts: impl IntoIterator<Item = ExtendedReceipt>) {
        self.receipts.extend(
            receipts
                .into_iter()
                .map(|receipt| (receipt.transaction_hash, receipt)),
        );
    }

    pub fn by_transaction_hash(&self, transaction_hash: B256) -> Option<&ExtendedReceipt> {
        self.receipts.get(&transaction_hash)
    }
}

#[derive(Debug)]
pub struct InMemoryReceiptQueries;

impl Default for InMemoryReceiptQueries {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryReceiptQueries {
    pub fn new() -> Self {
        Self
    }
}

impl ReceiptQueries for InMemoryReceiptQueries {
    type Err = Infallible;
    type Storage = ReceiptMemory;

    fn by_transaction_hash(
        &self,
        storage: &Self::Storage,
        transaction_hash: B256,
    ) -> Result<Option<TransactionReceipt>, Self::Err> {
        Ok(storage
            .by_transaction_hash(transaction_hash)
            .cloned()
            .map(TransactionReceipt::from))
    }
}

#[derive(Debug)]
pub struct InMemoryReceiptRepository;

impl Default for InMemoryReceiptRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryReceiptRepository {
    pub fn new() -> Self {
        Self
    }
}

impl ReceiptRepository for InMemoryReceiptRepository {
    type Err = Infallible;
    type Storage = ReceiptMemory;

    fn contains(&self, storage: &Self::Storage, transaction_hash: B256) -> Result<bool, Self::Err> {
        Ok(storage.contains(transaction_hash))
    }

    fn extend(
        &self,
        storage: &mut Self::Storage,
        receipts: impl IntoIterator<Item = ExtendedReceipt>,
    ) -> Result<(), Self::Err> {
        storage.extend(receipts);
        Ok(())
    }
}

#[cfg(any(feature = "test-doubles", test))]
mod test_doubles {
    use super::*;

    impl ReceiptQueries for () {
        type Err = Infallible;
        type Storage = ();

        fn by_transaction_hash(
            &self,
            _: &Self::Storage,
            _: B256,
        ) -> Result<Option<TransactionReceipt>, Self::Err> {
            Ok(None)
        }
    }

    impl ReceiptRepository for () {
        type Err = Infallible;
        type Storage = ();

        fn contains(&self, _: &Self::Storage, _: B256) -> Result<bool, Self::Err> {
            Ok(false)
        }

        fn extend(
            &self,
            _: &mut Self::Storage,
            _: impl IntoIterator<Item = ExtendedReceipt>,
        ) -> Result<(), Self::Err> {
            Ok(())
        }
    }
}
