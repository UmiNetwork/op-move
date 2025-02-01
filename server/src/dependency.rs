use {
    moved::{
        block::{BlockHash, BlockQueries, BlockRepository, MovedBlockHash},
        move_execution::{BaseTokenAccounts, MovedBaseTokenAccounts},
        receipt::{ReceiptQueries, ReceiptRepository},
        transaction::{TransactionQueries, TransactionRepository},
    },
    moved_genesis::config::GenesisConfig,
    moved_state::State,
};

#[cfg(feature = "storage")]
pub type SharedStorage = &'static moved_storage::RocksDb;
#[cfg(not(feature = "storage"))]
pub type SharedStorage = moved::in_memory::SharedMemory;
#[cfg(feature = "storage")]
pub type ReceiptStorage = &'static moved_storage::RocksDb;
#[cfg(not(feature = "storage"))]
pub type ReceiptStorage = moved::receipt::ReceiptMemory;
#[cfg(feature = "storage")]
pub type StateQueries = moved::state_actor::InMemoryStateQueries;
#[cfg(not(feature = "storage"))]
pub type StateQueries = moved::state_actor::InMemoryStateQueries;

pub fn block_hash() -> impl BlockHash + Send + Sync + 'static {
    MovedBlockHash
}

pub fn base_token(
    genesis_config: &GenesisConfig,
) -> impl BaseTokenAccounts + Send + Sync + 'static {
    MovedBaseTokenAccounts::new(genesis_config.treasury)
}

pub fn memory() -> SharedStorage {
    #[cfg(feature = "storage")]
    {
        db()
    }
    #[cfg(not(feature = "storage"))]
    {
        moved::in_memory::SharedMemory::new()
    }
}

pub fn block_repository() -> impl BlockRepository<Storage = SharedStorage> + Send + Sync + 'static {
    #[cfg(feature = "storage")]
    {
        moved_storage::block::RocksDbBlockRepository
    }
    #[cfg(not(feature = "storage"))]
    {
        moved::block::InMemoryBlockRepository::new()
    }
}

pub fn state() -> impl State + Send + Sync + 'static {
    #[cfg(feature = "storage")]
    {
        moved_storage::RocksDbState::new(std::sync::Arc::new(moved_storage::RocksEthTrieDb::new(
            db(),
        )))
    }
    #[cfg(not(feature = "storage"))]
    {
        moved_state::InMemoryState::new()
    }
}

pub fn state_query(genesis_config: &GenesisConfig) -> StateQueries {
    #[cfg(feature = "storage")]
    {
        moved::state_actor::InMemoryStateQueries::from_genesis(genesis_config.initial_state_root)
    }
    #[cfg(not(feature = "storage"))]
    {
        moved::state_actor::InMemoryStateQueries::from_genesis(genesis_config.initial_state_root)
    }
}

pub fn transaction_repository(
) -> impl TransactionRepository<Storage = SharedStorage> + Send + Sync + 'static {
    #[cfg(feature = "storage")]
    {
        moved_storage::transaction::RocksDbTransactionRepository
    }
    #[cfg(not(feature = "storage"))]
    {
        moved::transaction::InMemoryTransactionRepository::new()
    }
}

pub fn transaction_queries(
) -> impl TransactionQueries<Storage = SharedStorage> + Send + Sync + 'static {
    #[cfg(feature = "storage")]
    {
        moved_storage::transaction::RocksDbTransactionQueries
    }
    #[cfg(not(feature = "storage"))]
    {
        moved::transaction::InMemoryTransactionQueries::new()
    }
}

pub fn receipt_repository(
) -> impl ReceiptRepository<Storage = ReceiptStorage> + Send + Sync + 'static {
    #[cfg(feature = "storage")]
    {
        moved_storage::receipt::RocksDbReceiptRepository
    }
    #[cfg(not(feature = "storage"))]
    {
        moved::receipt::InMemoryReceiptRepository::new()
    }
}

pub fn receipt_queries() -> impl ReceiptQueries<Storage = ReceiptStorage> + Send + Sync + 'static {
    #[cfg(feature = "storage")]
    {
        moved_storage::receipt::RocksDbReceiptQueries
    }
    #[cfg(not(feature = "storage"))]
    {
        moved::receipt::InMemoryReceiptQueries::new()
    }
}

pub fn receipt_memory() -> ReceiptStorage {
    #[cfg(feature = "storage")]
    {
        db()
    }
    #[cfg(not(feature = "storage"))]
    {
        moved::receipt::ReceiptMemory::new()
    }
}

pub fn block_queries() -> impl BlockQueries<Storage = SharedStorage> + Send + Sync + 'static {
    #[cfg(feature = "storage")]
    {
        moved_storage::block::RocksDbBlockQueries
    }
    #[cfg(not(feature = "storage"))]
    {
        moved::block::InMemoryBlockQueries
    }
}

#[cfg(feature = "storage")]
lazy_static::lazy_static! {
    static ref Database: moved_storage::RocksDb = {
        create_db()
    };
}

#[cfg(feature = "storage")]
fn db() -> &'static moved_storage::RocksDb {
    &Database
}

#[cfg(feature = "storage")]
fn create_db() -> moved_storage::RocksDb {
    let path = "db";

    if std::fs::exists(path).unwrap() {
        std::fs::remove_dir_all(path)
            .expect("Removing non-empty database directory should succeed");
    }

    let mut options = moved_storage::rocksdb::Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);

    moved_storage::RocksDb::open_cf(&options, path, moved_storage::COLUMN_FAMILIES)
        .expect("Database should open in db dir")
}
