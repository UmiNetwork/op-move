use {
    crate::{
        block::BlockQueries,
        receipt::{
            write::ReceiptRepository, ReceiptQueries, TransactionReceipt, TransactionWithReceipt,
        },
        transaction::{
            ExtendedTransaction, TransactionQueries, TransactionRepository, TransactionResponse,
        },
        types::transactions::NormalizedExtendedTxEnvelope,
    },
    alloy::{primitives::TxKind, rpc::types::TransactionReceipt as AlloyTxReceipt},
    moved_shared::{primitives, primitives::B256},
    std::{collections::HashMap, convert::Infallible},
};

#[derive(Debug)]
pub struct ReceiptMemory {
    receipts: HashMap<B256, (TransactionWithReceipt, B256)>,
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

    pub fn add(&mut self, receipt: TransactionWithReceipt, block_hash: B256) {
        self.receipts.insert(receipt.tx_hash, (receipt, block_hash));
    }

    pub fn by_transaction_hash(
        &self,
        transaction_hash: B256,
    ) -> Option<&(TransactionWithReceipt, B256)> {
        self.receipts.get(&transaction_hash)
    }
}

#[derive(Debug)]
pub struct InMemoryReceiptQueries;

impl InMemoryReceiptQueries {
    pub fn new() -> Self {
        Self
    }
}

impl ReceiptQueries for InMemoryReceiptQueries {
    type Err = Infallible;
    type Storage = ReceiptMemory;

    fn by_transaction_hash<B: BlockQueries>(
        &self,
        storage: &Self::Storage,
        block_queries: &B,
        block_storage: &B::Storage,
        transaction_hash: B256,
    ) -> Result<Option<TransactionReceipt>, Self::Err>
    where
        Self::Err: From<B::Err>,
    {
        let Some((rx, block_hash)) = storage.by_transaction_hash(transaction_hash) else {
            return Ok(None);
        };
        let Some(block) = block_queries.by_hash(block_storage, *block_hash, false)? else {
            return Ok(None);
        };
        let contract_address = rx.contract_address;
        let (to, from) = match &rx.normalized_tx {
            NormalizedExtendedTxEnvelope::Canonical(tx) => {
                let to = match tx.to {
                    TxKind::Call(to) => Some(to),
                    TxKind::Create => None,
                };
                (to, tx.signer)
            }
            NormalizedExtendedTxEnvelope::DepositedTx(tx) => (Some(tx.to), tx.from),
        };
        let logs = rx
            .receipt
            .logs()
            .iter()
            .enumerate()
            .map(|(internal_index, log)| alloy::rpc::types::Log {
                inner: log.clone(),
                block_hash: Some(*block_hash),
                block_number: Some(block.0.header.number),
                block_timestamp: Some(block.0.header.timestamp),
                transaction_hash: Some(transaction_hash),
                transaction_index: Some(rx.tx_index),
                log_index: Some(rx.logs_offset + (internal_index as u64)),
                removed: false,
            })
            .collect();
        let receipt = primitives::with_rpc_logs(&rx.receipt, logs);
        let result = TransactionReceipt {
            inner: AlloyTxReceipt {
                inner: receipt,
                transaction_hash,
                transaction_index: Some(rx.tx_index),
                block_hash: Some(*block_hash),
                block_number: Some(block.0.header.number),
                gas_used: rx.gas_used as u128,
                // TODO: make all gas prices bounded by u128?
                effective_gas_price: rx.l2_gas_price.saturating_to(),
                // Always None because we do not support eip-4844 transactions
                blob_gas_used: None,
                blob_gas_price: None,
                from,
                to,
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

    fn add(
        &self,
        storage: &mut Self::Storage,
        receipt: TransactionWithReceipt,
        block_hash: B256,
    ) -> Result<(), Self::Err> {
        storage.add(receipt, block_hash);
        Ok(())
    }
}

#[cfg(any(feature = "test-doubles", test))]
mod test_doubles {
    use super::*;

    impl ReceiptQueries for () {
        type Err = Infallible;
        type Storage = ();

        fn by_transaction_hash<B: BlockQueries>(
            &self,
            _: &Self::Storage,
            _: &B,
            _: &B::Storage,
            _: B256,
        ) -> Result<Option<TransactionReceipt>, Self::Err>
        where
            Self::Err: From<B::Err>,
        {
            Ok(None)
        }
    }

    impl ReceiptRepository for () {
        type Err = Infallible;
        type Storage = ();

        fn contains(&self, _: &Self::Storage, _: B256) -> Result<bool, Self::Err> {
            Ok(false)
        }

        fn add(
            &self,
            _: &mut Self::Storage,
            _: TransactionWithReceipt,
            _: B256,
        ) -> Result<(), Self::Err> {
            Ok(())
        }
    }
}
