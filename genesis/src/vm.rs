use {
    crate::config::GenesisConfig,
    aptos_native_interface::SafeNativeBuilder,
    aptos_table_natives::TableChangeSet,
    aptos_types::on_chain_config::{Features, TimedFeaturesBuilder},
    aptos_vm_environment::natives::aptos_natives_with_builder,
    better_any::TidAble,
    move_binary_format::errors::{PartialVMError, VMError},
    move_core_types::{
        account_address::AccountAddress,
        identifier::IdentStr,
        language_storage::{ModuleId, TypeTag},
    },
    move_vm_runtime::{
        AsUnsyncCodeStorage, InstantiatedFunctionLoader, LazyLoader, LegacyLoaderConfig,
        LoadedFunction, ModuleStorage, RuntimeEnvironment, ScriptLoader, StructDefinitionLoader,
        UnsyncCodeStorage, UnsyncModuleStorage, WithRuntimeEnvironment,
        config::VMConfig,
        data_cache::{MoveVmDataCacheAdapter, NativeContextMoveVmDataCache, TransactionDataCache},
        module_traversal::{TraversalContext, TraversalStorage},
        move_vm::{MoveVM, SerializedReturnValues},
        native_extensions::{NativeContextExtensions, SessionListener},
    },
    move_vm_types::{
        code::ModuleBytesStorage,
        gas::{DependencyGasMeter, GasMeter},
        loaded_data::runtime_types::{StructIdentifier, Type},
        resolver::ResourceResolver,
    },
    std::borrow::Borrow,
    umi_state::AllAccountChanges,
};

pub struct UmiVm {
    env: RuntimeEnvironment,
}

impl UmiVm {
    pub fn new(config: &GenesisConfig) -> Self {
        let mut builder = SafeNativeBuilder::new(
            config.gas_costs.version,
            config.gas_costs.natives.clone(),
            config.gas_costs.vm.misc.clone(),
            TimedFeaturesBuilder::enable_all().build(),
            Features::default(),
            None,
        );
        let mut natives = aptos_natives_with_builder(&mut builder, false);
        umi_evm_ext::append_evm_natives(&mut natives, &builder);
        let config = VMConfig {
            paranoid_type_checks: true,
            ..Default::default()
        };
        let env = RuntimeEnvironment::new_with_config(natives, config);
        Self { env }
    }
}

impl WithRuntimeEnvironment for UmiVm {
    fn runtime_environment(&self) -> &RuntimeEnvironment {
        &self.env
    }
}

impl WithRuntimeEnvironment for &'_ UmiVm {
    fn runtime_environment(&self) -> &RuntimeEnvironment {
        &self.env
    }
}

pub struct RuntimeContext<'a, E, S> {
    env: E,
    storage: &'a S,
    traversal_storage: TraversalStorage,
}

impl<'a, S: ModuleBytesStorage, E: WithRuntimeEnvironment> RuntimeContext<'a, E, S> {
    pub fn new(env: E, storage: &'a S) -> Self {
        Self {
            env,
            storage,
            traversal_storage: TraversalStorage::new(),
        }
    }

    pub fn create_session<'ctx>(&'ctx self) -> Session<'ctx, 'a, 'ctx, E, S> {
        Session {
            runtime_context: self,
            traversal_context: TraversalContext::new(&self.traversal_storage),
            code_storage: self.as_unsync_code_storage(),
            transaction_data_cache: TransactionDataCache::empty(),
            extensions: NativeContextExtensions::default(),
        }
    }
}

impl<E: WithRuntimeEnvironment, S> WithRuntimeEnvironment for RuntimeContext<'_, E, S> {
    fn runtime_environment(&self) -> &RuntimeEnvironment {
        self.env.runtime_environment()
    }
}

impl<S: ModuleBytesStorage, E> ModuleBytesStorage for RuntimeContext<'_, E, S> {
    fn fetch_module_bytes(
        &self,
        address: &AccountAddress,
        module_name: &IdentStr,
    ) -> Result<Option<bytes::Bytes>, VMError> {
        self.storage.fetch_module_bytes(address, module_name)
    }
}

pub struct Session<'ctx, 'state, 'extensions, E, S> {
    runtime_context: &'ctx RuntimeContext<'state, E, S>,
    traversal_context: TraversalContext<'ctx>,
    code_storage: UnsyncCodeStorage<UnsyncModuleStorage<'ctx, RuntimeContext<'state, E, S>>>,
    transaction_data_cache: TransactionDataCache,
    extensions: NativeContextExtensions<'extensions>,
}

impl<'ctx, 'state, 'extensions, E, S> Session<'ctx, 'state, 'extensions, E, S>
where
    E: WithRuntimeEnvironment,
    S: ModuleBytesStorage + ResourceResolver,
{
    pub fn with_native_extensions(&mut self, extensions: NativeContextExtensions<'extensions>) {
        self.extensions = extensions;
    }

    pub fn add_native_extension<T>(&mut self, ext: T)
    where
        T: SessionListener + TidAble<'extensions>,
    {
        self.extensions.add(ext);
    }

    pub fn module_storage(&self) -> &impl ModuleStorage {
        &self.code_storage
    }

    pub fn get_type_tag(&self, ty: &Type) -> Result<TypeTag, PartialVMError> {
        self.runtime_context
            .env
            .runtime_environment()
            .ty_to_ty_tag(ty)
    }

    pub fn load_ty_arg(
        &mut self,
        tag: &TypeTag,
        gas_meter: &mut impl DependencyGasMeter,
    ) -> Result<Type, PartialVMError> {
        let loader = LazyLoader::new(&self.code_storage);
        let traversal_context = &mut self.traversal_context;
        let env = self.runtime_context.runtime_environment();

        env.vm_config().ty_builder.create_ty(tag, |struct_tag| {
            let module_id = struct_tag.module_id();
            let struct_map = env.struct_name_index_map();
            let pool = env.module_id_pool();
            let struct_name = StructIdentifier::new(pool, module_id, struct_tag.name.clone());
            let struct_idx = struct_map.struct_name_to_idx(&struct_name)?;
            loader.load_struct_definition(gas_meter, traversal_context, &struct_idx)
        })
    }

    pub fn check_resource_exists(
        &mut self,
        gas_meter: &mut impl DependencyGasMeter,
        addr: &AccountAddress,
        ty: &Type,
    ) -> Result<bool, PartialVMError> {
        let loader = LazyLoader::new(&self.code_storage);
        let mut data_cache = MoveVmDataCacheAdapter::new(
            &mut self.transaction_data_cache,
            self.runtime_context.storage,
            &loader,
        );
        let (exists, _) = data_cache.native_check_resource_exists(
            gas_meter,
            &mut self.traversal_context,
            addr,
            ty,
        )?;
        Ok(exists)
    }

    pub fn load_script(
        &mut self,
        gas_meter: &mut impl DependencyGasMeter,
        code: &[u8],
        ty_args: &[TypeTag],
    ) -> Result<LoadedFunction, VMError> {
        let loader = LazyLoader::new(&self.code_storage);
        loader.load_script(
            &LegacyLoaderConfig::unmetered(),
            gas_meter,
            &mut self.traversal_context,
            code,
            ty_args,
        )
    }

    pub fn load_function(
        &mut self,
        gas_meter: &mut impl DependencyGasMeter,
        module_id: &ModuleId,
        function_name: &IdentStr,
        ty_args: &[TypeTag],
    ) -> Result<LoadedFunction, VMError> {
        let loader = LazyLoader::new(&self.code_storage);
        loader.load_instantiated_function(
            &LegacyLoaderConfig::unmetered(),
            gas_meter,
            &mut self.traversal_context,
            module_id,
            function_name,
            ty_args,
        )
    }

    pub fn execution_function(
        &mut self,
        gas_meter: &mut impl GasMeter,
        function: LoadedFunction,
        serialized_args: Vec<impl Borrow<[u8]>>,
    ) -> Result<SerializedReturnValues, VMError> {
        let loader = LazyLoader::new(&self.code_storage);
        let mut data_cache = MoveVmDataCacheAdapter::new(
            &mut self.transaction_data_cache,
            self.runtime_context.storage,
            &loader,
        );
        MoveVM::execute_loaded_function(
            function,
            serialized_args,
            &mut data_cache,
            gas_meter,
            &mut self.traversal_context,
            &mut self.extensions,
            &loader,
        )
    }

    pub fn load_and_execute_function(
        &mut self,
        gas_meter: &mut impl GasMeter,
        module_id: &ModuleId,
        function_name: &IdentStr,
        ty_args: &[TypeTag],
        serialized_args: Vec<impl Borrow<[u8]>>,
    ) -> Result<SerializedReturnValues, VMError> {
        let function = self.load_function(gas_meter, module_id, function_name, ty_args)?;
        self.execution_function(gas_meter, function, serialized_args)
    }

    pub fn extract_table_changes(&mut self) -> Result<TableChangeSet, PartialVMError> {
        crate::table_changes::extract_table_changes(&mut self.extensions, &self.code_storage)
    }

    pub fn into_effects(self) -> Result<AllAccountChanges, PartialVMError> {
        let (changes, _) = self.into_effects_with_extensions()?;
        Ok(changes)
    }

    pub fn into_effects_with_extensions(
        self,
    ) -> Result<(AllAccountChanges, NativeContextExtensions<'extensions>), PartialVMError> {
        let change_set = self
            .transaction_data_cache
            .into_effects(&self.code_storage)?;
        let changes = AllAccountChanges::from_change_set(change_set);
        Ok((changes, self.extensions))
    }
}
