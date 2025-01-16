use {
    super::{
        trie_types,
        type_utils::{account_info_struct_tag, account_storage_struct_tag, code_hash_struct_tag},
        CODE_LAYOUT, EVM_NATIVE_ADDRESS,
    },
    crate::block::HeaderForExecution,
    alloy::primitives::map::HashMap,
    aptos_types::vm_status::StatusCode,
    better_any::{Tid, TidAble},
    move_binary_format::errors::PartialVMError,
    move_core_types::resolver::MoveResolver,
    move_vm_types::values::{VMValueCast, Value},
    revm::{
        db::{CacheDB, DatabaseRef},
        primitives::{
            utilities::KECCAK_EMPTY, Account, AccountInfo, Address, Bytecode, B256, U256,
        },
    },
    std::sync::RwLock,
};

#[derive(Tid)]
pub struct NativeEVMContext<'a> {
    pub resolver: &'a dyn MoveResolver<PartialVMError>,
    pub db: CacheDB<ResolverBackedDB<'a>>,
    pub state_changes: Vec<HashMap<Address, Account>>,
    pub block_header: HeaderForExecution,
}

impl<'a> NativeEVMContext<'a> {
    pub fn new(
        state: &'a impl MoveResolver<PartialVMError>,
        block_header: HeaderForExecution,
    ) -> Self {
        let inner_db = ResolverBackedDB::new(state);
        let db = CacheDB::new(inner_db);
        Self {
            resolver: state,
            db,
            state_changes: Vec::new(),
            block_header,
        }
    }
}

pub struct ResolverBackedDB<'a> {
    resolver: &'a dyn MoveResolver<PartialVMError>,
    storage_cache: RwLock<HashMap<Address, trie_types::AccountStorage>>,
}

impl<'a> ResolverBackedDB<'a> {
    pub fn new(resolver: &'a impl MoveResolver<PartialVMError>) -> Self {
        Self {
            resolver,
            storage_cache: RwLock::new(HashMap::default()),
        }
    }

    pub fn storage_for(
        &self,
        address: &Address,
    ) -> Result<trie_types::AccountStorage, PartialVMError> {
        let struct_tag = account_storage_struct_tag(address);
        match self
            .resolver
            .get_resource(&EVM_NATIVE_ADDRESS, &struct_tag)?
        {
            Some(bytes) => {
                let storage = trie_types::AccountStorage::try_deserialize(&bytes)
                    .expect("EVM account storage must deserialize correctly");
                Ok(storage)
            }
            None => Ok(trie_types::AccountStorage::default()),
        }
    }
}

impl<'a> DatabaseRef for ResolverBackedDB<'a> {
    type Error = PartialVMError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        let struct_tag = account_info_struct_tag(&address);
        let resource = self
            .resolver
            .get_resource(&EVM_NATIVE_ADDRESS, &struct_tag)?;
        let value = resource.map(|bytes| {
            trie_types::Account::try_deserialize(&bytes)
                .expect("EVM account info must deserialize correctly.")
        });
        let info = value.map(Into::into);
        Ok(info)
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        if code_hash == KECCAK_EMPTY {
            return Ok(Bytecode::new_legacy(Vec::new().into()));
        }

        let struct_tag = code_hash_struct_tag(&code_hash);
        let resource = self
            .resolver
            .get_resource(&EVM_NATIVE_ADDRESS, &struct_tag)?
            .ok_or_else(|| {
                PartialVMError::new(StatusCode::MISSING_DATA).with_message(format!(
                    "Missing EVM code corresponding to code hash {}",
                    struct_tag.name
                ))
            })?;
        let value = Value::simple_deserialize(&resource, &CODE_LAYOUT)
            .expect("EVM account info must deserialize correctly.");
        let bytes: Vec<u8> = value.cast()?;
        Ok(Bytecode::new_legacy(bytes.into()))
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        let mut cache_lock = self
            .storage_cache
            .write()
            .expect("ResolverBackedDB::storage_cache not poisoned");

        if let Some(storage) = cache_lock.get(&address) {
            return Ok(storage.get(index));
        }

        let storage = self.storage_for(&address)?;
        let value = storage.get(index);
        cache_lock.insert(address, storage);
        Ok(value)
    }

    fn block_hash_ref(&self, _number: u64) -> Result<B256, Self::Error> {
        // Complication: Move doesn't support this API out of the box.
        // We could build it out ourselves, but maybe it's not needed
        // for the contracts we want to support?

        unimplemented!("EVM block hash API not implemented")
    }
}
