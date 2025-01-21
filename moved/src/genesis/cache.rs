use {
    crate::{
        genesis::{config::GenesisConfig, init_raw},
        storage::State,
    },
    bytes::Bytes,
    move_binary_format::errors::PartialVMError,
    move_core_types::{
        account_address::AccountAddress,
        effects::{AccountChanges, ChangeSet, Op},
        identifier::Identifier,
        language_storage::{ModuleId, StructTag, TypeTag},
    },
    move_table_extension::{TableChange, TableChangeSet, TableHandle, TableInfo},
    std::{
        collections::{BTreeMap, BTreeSet},
        io::{Read, Write},
    },
};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub enum SerdeOp<T> {
    /// Inserts some new data into an empty slot.
    New(T),
    /// Modifies some data that currently exists.
    Modify(T),
    /// Deletes some data that currently exists.
    Delete,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct SerdeAccountChanges<Module, Resource> {
    modules: BTreeMap<Identifier, SerdeOp<Module>>,
    resources: BTreeMap<StructTag, SerdeOp<Resource>>,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct SerdeChanges<Module, Resource> {
    accounts: BTreeMap<AccountAddress, SerdeAccountChanges<Module, Resource>>,
}

impl From<ChangeSet> for SerdeChanges<Bytes, Bytes> {
    fn from(value: ChangeSet) -> Self {
        Self {
            accounts: value
                .into_inner()
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
        }
    }
}

impl From<SerdeChanges<Bytes, Bytes>> for ChangeSet {
    fn from(value: SerdeChanges<Bytes, Bytes>) -> Self {
        let mut set = Self::new();

        for (acc, changes) in value.accounts {
            for (id, op) in changes.modules {
                set.add_module_op(ModuleId::new(acc, id), op.into())
                    .unwrap();
            }
            for (id, op) in changes.resources {
                set.add_resource_op(acc, id, op.into()).unwrap();
            }
        }

        set
    }
}

impl From<AccountChanges<Bytes, Bytes>> for SerdeAccountChanges<Bytes, Bytes> {
    fn from(value: AccountChanges<Bytes, Bytes>) -> Self {
        let (modules, resources) = value.into_inner();

        Self {
            modules: modules.into_iter().map(|(k, v)| (k, v.into())).collect(),
            resources: resources.into_iter().map(|(k, v)| (k, v.into())).collect(),
        }
    }
}

impl From<Op<Bytes>> for SerdeOp<Bytes> {
    fn from(value: Op<Bytes>) -> Self {
        match value {
            Op::New(v) => Self::New(v),
            Op::Modify(v) => Self::Modify(v),
            Op::Delete => Self::Delete,
        }
    }
}

impl From<SerdeOp<Bytes>> for Op<Bytes> {
    fn from(value: SerdeOp<Bytes>) -> Self {
        match value {
            SerdeOp::New(v) => Self::New(v),
            SerdeOp::Modify(v) => Self::Modify(v),
            SerdeOp::Delete => Self::Delete,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct SerdeTableChange {
    pub entries: BTreeMap<Vec<u8>, SerdeOp<Bytes>>,
}

impl From<TableChange> for SerdeTableChange {
    fn from(value: TableChange) -> Self {
        Self {
            entries: value
                .entries
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
        }
    }
}

impl From<SerdeTableChange> for TableChange {
    fn from(value: SerdeTableChange) -> Self {
        Self {
            entries: value
                .entries
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct SerdeTableInfo {
    pub key_type: TypeTag,
    pub value_type: TypeTag,
}

impl From<TableInfo> for SerdeTableInfo {
    fn from(value: TableInfo) -> Self {
        Self {
            key_type: value.key_type,
            value_type: value.value_type,
        }
    }
}

impl From<SerdeTableInfo> for TableInfo {
    fn from(value: SerdeTableInfo) -> Self {
        Self {
            key_type: value.key_type,
            value_type: value.value_type,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct SerdeTableChangeSet {
    pub new_tables: BTreeMap<AccountAddress, SerdeTableInfo>,
    pub removed_tables: BTreeSet<AccountAddress>,
    pub changes: BTreeMap<AccountAddress, SerdeTableChange>,
}

impl From<TableChangeSet> for SerdeTableChangeSet {
    fn from(value: TableChangeSet) -> Self {
        Self {
            new_tables: value
                .new_tables
                .into_iter()
                .map(|(k, v)| (k.0, v.into()))
                .collect(),
            removed_tables: value.removed_tables.into_iter().map(|k| k.0).collect(),
            changes: value
                .changes
                .into_iter()
                .map(|(k, v)| (k.0, v.into()))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct SerdeAllChanges {
    changes: SerdeChanges<Bytes, Bytes>,
    tables: SerdeTableChangeSet,
}

impl SerdeAllChanges {
    pub fn new(changes: SerdeChanges<Bytes, Bytes>, tables: SerdeTableChangeSet) -> Self {
        Self { changes, tables }
    }
}

impl From<SerdeTableChangeSet> for TableChangeSet {
    fn from(value: SerdeTableChangeSet) -> Self {
        Self {
            new_tables: value
                .new_tables
                .into_iter()
                .map(|(k, v)| (TableHandle(k), v.into()))
                .collect(),
            removed_tables: value.removed_tables.into_iter().map(TableHandle).collect(),
            changes: value
                .changes
                .into_iter()
                .map(|(k, v)| (TableHandle(k), v.into()))
                .collect(),
        }
    }
}

pub fn save(
    config: &GenesisConfig,
    state: &impl State<Err = PartialVMError>,
) -> (ChangeSet, TableChangeSet) {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/genesis.bin");
    let (changes, tables) = init_raw(config, state);
    let changes = SerdeChanges::from(changes);
    let tables = SerdeTableChangeSet::from(tables);
    let all_changes = SerdeAllChanges::new(changes, tables);
    let contents = bcs::to_bytes(&all_changes).unwrap();
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(contents.as_slice()).unwrap();
    file.flush().unwrap();

    (all_changes.changes.into(), all_changes.tables.into())
}

pub fn try_load() -> Option<(ChangeSet, TableChangeSet)> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/genesis.bin");
    let mut file = std::fs::File::open(path).ok()?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents).ok()?;
    let contents: SerdeAllChanges = bcs::from_bytes(contents.as_slice()).ok()?;

    Some((contents.changes.into(), contents.tables.into()))
}
