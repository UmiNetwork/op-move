use {
    aptos_table_natives::{TableChange, TableChangeSet},
    bytes::Bytes,
    core::fmt,
    move_core_types::{
        account_address::AccountAddress,
        effects::{ChangeSet, Op},
        identifier::Identifier,
        language_storage::{ModuleId, StructTag},
    },
    std::{
        collections::{BTreeMap, btree_map::Entry},
        fmt::{Debug, DebugStruct, Formatter},
    },
};

/// Represents the difference between two [`State`]s.
///
/// The difference can be [applied]. If the difference is between two states `S` and `S'`, then it
/// must be true that `S' := apply(S, Changes)`.
///
/// `Changes` are usually produced by running a transaction on state `S`, in which case `S'`
/// represents the state after.
///
/// [`State`]: crate::State
/// [applied]: crate::State::apply
pub struct Changes {
    pub accounts: AllAccountChanges,
    pub tables: TableChangeSet,
}

impl Changes {
    pub fn empty() -> Self {
        Self {
            accounts: AllAccountChanges::default(),
            tables: TableChangeSet::default(),
        }
    }

    pub fn new(accounts: AllAccountChanges, tables: TableChangeSet) -> Self {
        Self { accounts, tables }
    }

    pub fn without_tables(accounts: ChangeSet) -> Self {
        Self::new(
            AllAccountChanges::from_change_set(accounts),
            TableChangeSet::default(),
        )
    }

    pub fn squash(&mut self, other: Self) -> anyhow::Result<()> {
        self.accounts.squash(other.accounts)
    }

    pub fn include(&mut self, other: AllAccountChanges) -> anyhow::Result<()> {
        self.accounts.squash(other)
    }

    pub fn from_account_changes(accounts: AllAccountChanges) -> Self {
        Self {
            accounts,
            tables: TableChangeSet::default(),
        }
    }
}

impl Debug for Changes {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        /// A `Closure` wrapper.
        ///
        /// Implements [`Debug`] if the `Closure` has the same signature as [`Debug::fmt`].
        struct DebugClosure<Closure>(Closure);

        impl<F: Fn(&mut Formatter<'_>) -> fmt::Result> Debug for DebugClosure<F> {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                self.0(f)
            }
        }

        /// An extension trait that provides the closure based debug formatting.
        trait FieldWithStable {
            /// Adds a new field to the generated struct output.
            ///
            /// This method is equivalent to [`DebugStruct::field`], but formats the
            /// value using a provided closure rather than by calling [`Debug::fmt`].
            fn field_with_stable<F>(&mut self, name: &str, value_fmt: F) -> &mut Self
            where
                F: Fn(&mut Formatter<'_>) -> fmt::Result;
        }

        impl FieldWithStable for DebugStruct<'_, '_> {
            fn field_with_stable<F>(&mut self, name: &str, value_fmt: F) -> &mut Self
            where
                F: Fn(&mut Formatter<'_>) -> std::fmt::Result,
            {
                self.field(name, &DebugClosure(value_fmt))
            }
        }

        f.debug_struct("Changes")
            .field("accounts", &self.accounts)
            .field_with_stable("tables", |f| {
                f.debug_struct("TableChangeSet")
                    .field_with_stable("changes", |f| {
                        f.debug_map()
                            .entries(self.tables.changes.iter().map(|(k, v)| (k, &v.entries)))
                            .finish()
                    })
                    .field_with_stable("new_tables", |f| {
                        f.debug_map()
                            .entries(self.tables.new_tables.iter())
                            .finish()
                    })
                    .field_with_stable("removed_tables", |f| {
                        f.debug_set()
                            .entries(self.tables.removed_tables.iter())
                            .finish()
                    })
                    .finish()
            })
            .finish()
    }
}

impl Clone for Changes {
    fn clone(&self) -> Self {
        Self {
            accounts: self.accounts.clone(),
            tables: TableChangeSet {
                new_tables: self.tables.new_tables.clone(),
                removed_tables: self.tables.removed_tables.clone(),
                changes: self
                    .tables
                    .changes
                    .iter()
                    .map(|(k, v)| {
                        (
                            *k,
                            TableChange {
                                entries: v.entries.clone(),
                            },
                        )
                    })
                    .collect(),
            },
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct AllAccountChanges {
    accounts: BTreeMap<AccountAddress, SingleAccountChanges>,
}

impl AllAccountChanges {
    pub fn from_change_set(accounts: ChangeSet) -> Self {
        let accounts = accounts
            .into_inner()
            .into_iter()
            .map(|(k, v)| {
                let changes = SingleAccountChanges {
                    modules: BTreeMap::new(),
                    resources: v.into_resources(),
                };
                (k, changes)
            })
            .collect();
        Self { accounts }
    }

    pub fn accounts(&self) -> &BTreeMap<AccountAddress, SingleAccountChanges> {
        &self.accounts
    }

    pub fn add_account_changeset(
        &mut self,
        address: AccountAddress,
        changes: SingleAccountChanges,
    ) -> anyhow::Result<()> {
        match self.accounts.entry(address) {
            Entry::Occupied(_) => {
                anyhow::bail!("Failed to add account change set. Account {address} already exists.")
            }
            Entry::Vacant(entry) => {
                entry.insert(changes);
            }
        }
        Ok(())
    }

    pub fn into_inner(self) -> BTreeMap<AccountAddress, SingleAccountChanges> {
        self.accounts
    }

    pub fn add_module_op(&mut self, module_id: ModuleId, op: Op<Bytes>) -> anyhow::Result<()> {
        let account = self.get_or_insert_account_changeset(*module_id.address());
        account.add_module_op(module_id.name().to_owned(), op)
    }

    pub fn add_resource_op(
        &mut self,
        address: AccountAddress,
        tag: StructTag,
        op: Op<Bytes>,
    ) -> anyhow::Result<()> {
        let account = self.get_or_insert_account_changeset(address);
        account.add_resource_op(tag, op)
    }

    fn get_or_insert_account_changeset(
        &mut self,
        addr: AccountAddress,
    ) -> &mut SingleAccountChanges {
        match self.accounts.entry(addr) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(SingleAccountChanges::default()),
        }
    }

    pub fn resources(&self) -> impl Iterator<Item = (&AccountAddress, &StructTag, Op<&Bytes>)> {
        self.accounts.iter().flat_map(|(addr, a)| {
            a.resources()
                .iter()
                .map(move |(k, v)| (addr, k, v.as_ref()))
        })
    }

    pub fn modules(&self) -> impl Iterator<Item = (&AccountAddress, &Identifier, Op<&Bytes>)> {
        self.accounts
            .iter()
            .flat_map(|(addr, a)| a.modules().iter().map(move |(k, v)| (addr, k, v.as_ref())))
    }

    pub fn squash(&mut self, other: Self) -> anyhow::Result<()> {
        for (addr, other_account_changes) in other.accounts {
            match self.accounts.entry(addr) {
                Entry::Occupied(mut entry) => {
                    entry.get_mut().squash(other_account_changes)?;
                }
                Entry::Vacant(entry) => {
                    entry.insert(other_account_changes);
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct SingleAccountChanges {
    modules: BTreeMap<Identifier, Op<Bytes>>,
    resources: BTreeMap<StructTag, Op<Bytes>>,
}

impl SingleAccountChanges {
    pub fn modules(&self) -> &BTreeMap<Identifier, Op<Bytes>> {
        &self.modules
    }

    pub fn resources(&self) -> &BTreeMap<StructTag, Op<Bytes>> {
        &self.resources
    }

    pub fn into_inner(
        self,
    ) -> (
        BTreeMap<Identifier, Op<Bytes>>,
        BTreeMap<StructTag, Op<Bytes>>,
    ) {
        (self.modules, self.resources)
    }

    pub fn add_module_op(&mut self, id: Identifier, op: Op<Bytes>) -> anyhow::Result<()> {
        match self.modules.entry(id) {
            Entry::Occupied(entry) => anyhow::bail!(
                "Failed to add account changes module op. Identifier {} already exists.",
                entry.key()
            ),
            Entry::Vacant(entry) => {
                entry.insert(op);
            }
        }
        Ok(())
    }

    pub fn add_resource_op(&mut self, tag: StructTag, op: Op<Bytes>) -> anyhow::Result<()> {
        match self.resources.entry(tag) {
            Entry::Occupied(entry) => anyhow::bail!(
                "Failed to add account changes resource op. StructTag {:?} already exists.",
                entry.key()
            ),
            Entry::Vacant(entry) => {
                entry.insert(op);
            }
        }
        Ok(())
    }

    pub fn squash(&mut self, other: Self) -> anyhow::Result<()> {
        squash(&mut self.modules, other.modules)?;
        squash(&mut self.resources, other.resources)
    }
}

fn squash<K, V>(map: &mut BTreeMap<K, Op<V>>, other: BTreeMap<K, Op<V>>) -> anyhow::Result<()>
where
    K: Ord,
{
    for (key, op) in other.into_iter() {
        match map.entry(key) {
            Entry::Occupied(mut entry) => {
                let r = entry.get_mut();
                match (&r, op) {
                    (Op::Modify(_), modified @ Op::Modify(_)) => *r = modified,
                    (Op::New(_), Op::Modify(modified)) => *r = Op::New(modified),
                    (Op::Modify(_), deleted @ Op::Delete) => *r = deleted,
                    (Op::Delete, Op::New(data)) => *r = Op::Modify(data),
                    (Op::New(_), Op::Delete) => {
                        entry.remove();
                    }
                    (Op::Modify(_) | Op::New(_), Op::New(_))
                    | (Op::Delete, Op::Delete | Op::Modify(_)) => {
                        anyhow::bail!("The given change sets cannot be squashed");
                    }
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(op);
            }
        }
    }

    Ok(())
}
