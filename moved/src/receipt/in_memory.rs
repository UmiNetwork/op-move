use {
    crate::receipt::{
        write::ReceiptRepository, ExtendedReceipt, ReceiptQueries, TransactionReceipt,
    },
    alloy::rpc::types::TransactionReceipt as AlloyTxReceipt,
    moved_shared::{primitives, primitives::B256},
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
        let Some(rx) = storage.by_transaction_hash(transaction_hash) else {
            return Ok(None);
        };
        let contract_address = rx.contract_address;
        let logs = rx
            .receipt
            .logs()
            .iter()
            .enumerate()
            .map(|(internal_index, log)| alloy::rpc::types::Log {
                inner: log.clone(),
                block_hash: Some(rx.block_hash),
                block_number: Some(rx.block_number),
                block_timestamp: Some(rx.block_timestamp),
                transaction_hash: Some(transaction_hash),
                transaction_index: Some(rx.transaction_index),
                log_index: Some(rx.logs_offset + (internal_index as u64)),
                removed: false,
            })
            .collect();
        let receipt = primitives::with_rpc_logs(&rx.receipt, logs);
        let result = TransactionReceipt {
            inner: AlloyTxReceipt {
                inner: receipt,
                transaction_hash,
                transaction_index: Some(rx.transaction_index),
                block_hash: Some(rx.block_hash),
                block_number: Some(rx.block_number),
                gas_used: rx.gas_used as u128,
                // TODO: make all gas prices bounded by u128?
                effective_gas_price: rx.l2_gas_price.saturating_to(),
                // Always None because we do not support eip-4844 transactions
                blob_gas_used: None,
                blob_gas_price: None,
                from: rx.from,
                to: rx.to,
                contract_address,
                // EIP-7702 not yet supported
                authorization_list: None,
            },
            l1_block_info: rx.l1_block_info.unwrap_or_default(),
        };
        Ok(Some(result))
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
