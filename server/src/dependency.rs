use {
    moved_blockchain::{
        block::{BaseGasFee, BlockHash, BlockRepository, MovedBlockHash},
        payload::NewPayloadId,
    },
    moved_execution::{BaseTokenAccounts, CreateL1GasFee, CreateL2GasFee, MovedBaseTokenAccounts},
    moved_genesis::config::GenesisConfig,
    moved_state::State,
};

#[cfg(feature = "storage")]
pub type SharedStorage = &'static moved_storage::RocksDb;
#[cfg(not(feature = "storage"))]
pub type SharedStorage = moved_blockchain::in_memory::SharedMemory;
#[cfg(feature = "storage")]
pub type ReceiptStorage = &'static moved_storage::RocksDb;
#[cfg(not(feature = "storage"))]
pub type ReceiptStorage = moved_blockchain::receipt::ReceiptMemory;
#[cfg(feature = "storage")]
pub type StateQueries = moved_storage::RocksDbStateQueries<'static>;
#[cfg(not(feature = "storage"))]
pub type StateQueries = moved_blockchain::state::InMemoryStateQueries;
#[cfg(feature = "storage")]
pub type ReceiptRepository = moved_storage::receipt::RocksDbReceiptRepository;
#[cfg(not(feature = "storage"))]
pub type ReceiptRepository = moved_blockchain::receipt::InMemoryReceiptRepository;
#[cfg(feature = "storage")]
pub type ReceiptQueries = moved_storage::receipt::RocksDbReceiptQueries;
#[cfg(not(feature = "storage"))]
pub type ReceiptQueries = moved_blockchain::receipt::InMemoryReceiptQueries;
#[cfg(feature = "storage")]
pub type PayloadQueries = moved_storage::payload::RocksDbPayloadQueries;
#[cfg(not(feature = "storage"))]
pub type PayloadQueries = moved_blockchain::payload::InMemoryPayloadQueries;
#[cfg(feature = "storage")]
pub type TransactionRepository = moved_storage::transaction::RocksDbTransactionRepository;
#[cfg(not(feature = "storage"))]
pub type TransactionRepository = moved_blockchain::transaction::InMemoryTransactionRepository;
#[cfg(feature = "storage")]
pub type TransactionQueries = moved_storage::transaction::RocksDbTransactionQueries;
#[cfg(not(feature = "storage"))]
pub type TransactionQueries = moved_blockchain::transaction::InMemoryTransactionQueries;
#[cfg(feature = "storage")]
pub type BlockQueries = moved_storage::block::RocksDbBlockQueries;
#[cfg(not(feature = "storage"))]
pub type BlockQueries = moved_blockchain::block::InMemoryBlockQueries;

type StateActor<A, B, C, D, E, F, G, H> = moved_app::StateActor<
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    BlockQueries,
    SharedStorage,
    StateQueries,
    TransactionRepository,
    TransactionQueries,
    ReceiptStorage,
    ReceiptRepository,
    ReceiptQueries,
    PayloadQueries,
>;
type OnTxBatch<A, B, C, D, E, F, G, H> = moved_app::OnTxBatch<StateActor<A, B, C, D, E, F, G, H>>;
type OnTx<A, B, C, D, E, F, G, H> = moved_app::OnTx<StateActor<A, B, C, D, E, F, G, H>>;
type OnPayload<A, B, C, D, E, F, G, H> = moved_app::OnPayload<StateActor<A, B, C, D, E, F, G, H>>;

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
        moved_blockchain::in_memory::SharedMemory::new()
    }
}

pub fn block_repository() -> impl BlockRepository<Storage = SharedStorage> + Send + Sync + 'static {
    #[cfg(feature = "storage")]
    {
        moved_storage::block::RocksDbBlockRepository
    }
    #[cfg(not(feature = "storage"))]
    {
        moved_blockchain::block::InMemoryBlockRepository::new()
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
        moved_storage::RocksDbStateQueries::from_genesis(db(), genesis_config.initial_state_root)
    }
    #[cfg(not(feature = "storage"))]
    {
        moved_blockchain::state::InMemoryStateQueries::from_genesis(
            genesis_config.initial_state_root,
        )
    }
}

pub fn on_tx_batch<
    A: State,
    B: NewPayloadId,
    C: BlockHash,
    D: BlockRepository<Storage = SharedStorage>,
    E: BaseGasFee,
    F: CreateL1GasFee,
    G: CreateL2GasFee,
    H: BaseTokenAccounts,
>() -> OnTxBatch<A, B, C, D, E, F, G, H> {
    #[cfg(feature = "storage")]
    {
        Box::new(|| {
            Box::new(|state| {
                state
                    .state_queries()
                    .push_state_root(state.state().state_root())
                    .unwrap()
            })
        })
    }
    #[cfg(not(feature = "storage"))]
    {
        moved_app::StateActor::on_tx_batch_in_memory()
    }
}

pub fn on_tx<
    A: State,
    B: NewPayloadId,
    C: BlockHash,
    D: BlockRepository<Storage = SharedStorage>,
    E: BaseGasFee,
    F: CreateL1GasFee,
    G: CreateL2GasFee,
    H: BaseTokenAccounts,
>() -> OnTx<A, B, C, D, E, F, G, H> {
    #[cfg(feature = "storage")]
    {
        moved_app::StateActor::on_tx_noop()
    }
    #[cfg(not(feature = "storage"))]
    {
        moved_app::StateActor::on_tx_in_memory()
    }
}

pub fn on_payload<
    A: State,
    B: NewPayloadId,
    C: BlockHash,
    D: BlockRepository<Storage = SharedStorage>,
    E: BaseGasFee,
    F: CreateL1GasFee,
    G: CreateL2GasFee,
    H: BaseTokenAccounts,
>() -> OnPayload<A, B, C, D, E, F, G, H> {
    #[cfg(feature = "storage")]
    {
        Box::new(|| {
            Box::new(|state, id, hash| state.payload_queries().add_block_hash(id, hash).unwrap())
        })
    }
    #[cfg(not(feature = "storage"))]
    {
        moved_app::StateActor::on_payload_in_memory()
    }
}

pub fn transaction_repository() -> TransactionRepository {
    #[cfg(feature = "storage")]
    {
        moved_storage::transaction::RocksDbTransactionRepository
    }
    #[cfg(not(feature = "storage"))]
    {
        moved_blockchain::transaction::InMemoryTransactionRepository::new()
    }
}

pub fn transaction_queries() -> TransactionQueries {
    #[cfg(feature = "storage")]
    {
        moved_storage::transaction::RocksDbTransactionQueries
    }
    #[cfg(not(feature = "storage"))]
    {
        moved_blockchain::transaction::InMemoryTransactionQueries::new()
    }
}

pub fn receipt_repository() -> ReceiptRepository {
    #[cfg(feature = "storage")]
    {
        moved_storage::receipt::RocksDbReceiptRepository
    }
    #[cfg(not(feature = "storage"))]
    {
        moved_blockchain::receipt::InMemoryReceiptRepository::new()
    }
}

pub fn receipt_queries() -> ReceiptQueries {
    #[cfg(feature = "storage")]
    {
        moved_storage::receipt::RocksDbReceiptQueries
    }
    #[cfg(not(feature = "storage"))]
    {
        moved_blockchain::receipt::InMemoryReceiptQueries::new()
    }
}

pub fn receipt_memory() -> ReceiptStorage {
    #[cfg(feature = "storage")]
    {
        db()
    }
    #[cfg(not(feature = "storage"))]
    {
        moved_blockchain::receipt::ReceiptMemory::new()
    }
}

pub fn block_queries() -> BlockQueries {
    #[cfg(feature = "storage")]
    {
        moved_storage::block::RocksDbBlockQueries
    }
    #[cfg(not(feature = "storage"))]
    {
        moved_blockchain::block::InMemoryBlockQueries
    }
}

pub fn payload_queries() -> PayloadQueries {
    #[cfg(feature = "storage")]
    {
        moved_storage::payload::RocksDbPayloadQueries::new(db())
    }
    #[cfg(not(feature = "storage"))]
    {
        moved_blockchain::payload::InMemoryPayloadQueries::new()
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
