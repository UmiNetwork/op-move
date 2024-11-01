use {
    crate::primitives::{ToEthAddress, ToMoveAddress},
    alloy::{hex::ToHexExt, primitives::map::HashMap},
    aptos_native_interface::{
        safely_pop_arg, SafeNativeBuilder, SafeNativeContext, SafeNativeError, SafeNativeResult,
    },
    aptos_types::vm_status::StatusCode,
    better_any::{Tid, TidAble},
    move_binary_format::errors::PartialVMError,
    move_core_types::{
        account_address::AccountAddress,
        effects::{AccountChangeSet, ChangeSet, Op},
        ident_str,
        identifier::{IdentStr, Identifier},
        language_storage::StructTag,
        resolver::MoveResolver,
        value::{MoveStructLayout, MoveTypeLayout},
    },
    move_vm_runtime::{
        native_extensions::NativeContextExtensions, native_functions::NativeFunctionTable,
    },
    move_vm_types::{
        loaded_data::runtime_types::Type,
        values::{Struct, VMValueCast, Value},
    },
    revm::{
        db::{CacheDB, DatabaseCommit, DatabaseRef},
        primitives::{
            utilities::KECCAK_EMPTY, Account, AccountInfo, Address, Bytecode, EVMError,
            ExecutionResult, Log, TxEnv, TxKind, B256, U256,
        },
        Evm,
    },
    smallvec::SmallVec,
    std::{collections::VecDeque, sync::LazyLock},
};

const EVM_NATIVE_ADDRESS: AccountAddress = AccountAddress::ONE;
const EVM_NATIVE_MODULE: &IdentStr = ident_str!("evm");
const ACCOUNT_STORAGE_LAYOUT: MoveTypeLayout = MoveTypeLayout::U256;
static CODE_LAYOUT: LazyLock<MoveTypeLayout> =
    LazyLock::new(|| MoveTypeLayout::Vector(Box::new(MoveTypeLayout::U8)));
static ACCOUNT_INFO_LAYOUT: LazyLock<MoveTypeLayout> = LazyLock::new(|| {
    MoveTypeLayout::Struct(MoveStructLayout::Runtime(vec![
        MoveTypeLayout::U256,
        MoveTypeLayout::U64,
        MoveTypeLayout::Vector(Box::new(MoveTypeLayout::U8)),
    ]))
});

#[derive(Tid)]
pub struct NativeEVMContext<'a> {
    resolver: &'a dyn MoveResolver<PartialVMError>,
    db: CacheDB<ResolverBackedDB<'a>>,
    state_changes: Vec<HashMap<Address, Account>>,
}

impl<'a> NativeEVMContext<'a> {
    pub fn new(state: &'a impl MoveResolver<PartialVMError>) -> Self {
        let inner_db = ResolverBackedDB { resolver: state };
        let db = CacheDB::new(inner_db);
        Self {
            resolver: state,
            db,
            state_changes: Vec::new(),
        }
    }
}

pub fn append_evm_natives(natives: &mut NativeFunctionTable, builder: &SafeNativeBuilder) {
    type NativeFn = fn(
        &mut SafeNativeContext,
        Vec<Type>,
        VecDeque<Value>,
    ) -> SafeNativeResult<SmallVec<[Value; 1]>>;
    let mut push_native = |name, f: NativeFn| {
        let native = builder.make_native(f);
        natives.push((EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE.into(), name, native));
    };

    push_native(ident_str!("native_evm_call").into(), evm_call);
    push_native(ident_str!("native_evm_create").into(), evm_create);
}

pub fn extract_evm_changes(extensions: &NativeContextExtensions) -> ChangeSet {
    let evm_native_ctx = extensions.get::<NativeEVMContext>();
    let mut result = ChangeSet::new();
    let mut account_changes = AccountChangeSet::new();
    for state in &evm_native_ctx.state_changes {
        let mut single_account_changes = AccountChangeSet::new();
        for (address, account) in state {
            // If the account is not touched then there are no changes.
            if !account.is_touched() {
                continue;
            }

            add_account_changes(
                address,
                account,
                evm_native_ctx.resolver,
                &account_changes,
                &mut single_account_changes,
            );
        }
        account_changes
            .squash(single_account_changes)
            .expect("Sequential EVM native changes must merge");
    }
    result
        .add_account_changeset(EVM_NATIVE_ADDRESS, account_changes)
        .expect("EVM native changes must be added");
    result
}

fn add_account_changes(
    address: &Address,
    account: &Account,
    resolver: &dyn MoveResolver<PartialVMError>,
    prior_changes: &AccountChangeSet,
    result: &mut AccountChangeSet,
) {
    debug_assert!(
        account.is_touched(),
        "Untouched accounts are filtered out before calling this function."
    );

    if account.is_selfdestructed() {
        unimplemented!("EVM account self-destruct is not implemented");
    }

    let code_hash = get_account_code_hash(&account.info);

    let resource_exists = |struct_tag: &StructTag| {
        let exists_in_prior_changes = prior_changes.resources().contains_key(struct_tag);
        // Early exit since we don't need to check the resolver if it's in the prior changes.
        if exists_in_prior_changes {
            return exists_in_prior_changes;
        }
        // If not in the prior changes then check the resolver
        resolver
            .get_resource(&EVM_NATIVE_ADDRESS, struct_tag)
            .map(|x| x.is_some())
            .unwrap_or(false)
    };

    // Push AccountInfo resource
    let struct_tag = account_info_struct_tag(address);
    let account_info = account_info_to_move_value(&account.info, code_hash)
        .simple_serialize(&ACCOUNT_INFO_LAYOUT)
        .expect("Account info must serialize");
    let is_created = !resource_exists(&struct_tag);
    let op = if is_created {
        Op::New(account_info.into())
    } else {
        Op::Modify(account_info.into())
    };
    result
        .add_resource_op(struct_tag, op)
        .expect("Resource cannot already exist in result");

    // Push CodeHash resource if needed.
    // We don't need to push anything if the resource already exists.
    let struct_tag = code_hash_struct_tag(&code_hash);
    let code_resource_exists = resource_exists(&struct_tag);
    if !code_resource_exists {
        if let Some(code) = &account.info.code {
            if !code.is_empty() {
                let struct_tag = code_hash_struct_tag(&code_hash);
                let code = Value::vector_u8(code.original_bytes())
                    .simple_serialize(&CODE_LAYOUT)
                    .expect("EVM code must serialize");
                let op = Op::New(code.into());
                // If the same contract is deployed more than once then the same resource
                // could be added twice, but that's ok we can just skip the duplicate.
                result.add_resource_op(struct_tag, op).ok();
            }
        }
    }

    // TODO: If an address self-destructs and then is re-created then its storage
    // must be entirely reset. With the current model we cannot easily delete all the
    // storage for an account (we would need to loop through all the resources for
    // the EVM native). Therefore this may need a redesign if we decide to support
    // EVM self-destruct.
    for (index, value) in account.changed_storage_slots() {
        let struct_tag = account_storage_struct_tag(address, index);
        let op = if value.present_value.is_zero() {
            Op::Delete
        } else {
            let move_value = Value::u256(to_move_u256(&value.present_value))
                .simple_serialize(&ACCOUNT_STORAGE_LAYOUT)
                .expect("EVM storage value must serialize");
            if value.original_value.is_zero() {
                Op::New(move_value.into())
            } else {
                Op::Modify(move_value.into())
            }
        };
        result
            .add_resource_op(struct_tag, op)
            .expect("Cannot have duplicate storage index");
    }
}

fn account_info_struct_tag(address: &Address) -> StructTag {
    let name = format!("Info_{}", address.encode_hex());
    let name = Identifier::new(name).expect("Account info name is valid");
    StructTag {
        address: EVM_NATIVE_ADDRESS,
        module: EVM_NATIVE_MODULE.into(),
        name,
        type_args: Vec::new(),
    }
}

fn code_hash_struct_tag(code_hash: &B256) -> StructTag {
    let name = format!("CodeHash_{}", code_hash.encode_hex());
    let name = Identifier::new(name).expect("Code hash name is valid");
    StructTag {
        address: EVM_NATIVE_ADDRESS,
        module: EVM_NATIVE_MODULE.into(),
        name,
        type_args: Vec::new(),
    }
}

fn account_storage_struct_tag(address: &Address, index: &U256) -> StructTag {
    let name = format!("Storage_{}_{:x}", address.encode_hex(), index);
    let name = Identifier::new(name).expect("Account storage name is valid");
    StructTag {
        address: EVM_NATIVE_ADDRESS,
        module: EVM_NATIVE_MODULE.into(),
        name,
        type_args: Vec::new(),
    }
}

fn get_account_code_hash(info: &AccountInfo) -> B256 {
    if let Some(code) = &info.code {
        if code.is_empty() {
            KECCAK_EMPTY
        } else {
            code.hash_slow()
        }
    } else if info.code_hash.is_zero() {
        KECCAK_EMPTY
    } else {
        info.code_hash
    }
}

fn to_move_u256(x: &U256) -> move_core_types::u256::U256 {
    move_core_types::u256::U256::from_le_bytes(&x.to_le_bytes())
}

fn from_move_u256(x: move_core_types::u256::U256) -> U256 {
    U256::from_le_bytes(x.to_le_bytes())
}

fn account_info_to_move_value(info: &AccountInfo, code_hash: B256) -> Value {
    let fields = [
        Value::u256(to_move_u256(&info.balance)),
        Value::u64(info.nonce),
        Value::vector_u8(code_hash),
    ];
    Value::struct_(Struct::pack(fields))
}

fn move_value_to_account_info(value: Value) -> Result<AccountInfo, PartialVMError> {
    let s: Struct = value.cast()?;
    let mut fields = s.unpack()?;
    // Safety: Unwrap is safe because AccountInfo has 3 fields (see `account_info_to_move_value`)
    let balance: move_core_types::u256::U256 = fields.next().unwrap().cast()?;
    let nonce: u64 = fields.next().unwrap().cast()?;
    let code_hash: Vec<u8> = fields.next().unwrap().cast()?;
    let code_hash: B256 = B256::from_slice(&code_hash);
    Ok(AccountInfo {
        balance: from_move_u256(balance),
        nonce,
        code_hash,
        code: None,
    })
}

fn evm_log_to_move_value(log: Log) -> Value {
    let fields = [
        Value::address(log.address.to_move_address()),
        Value::vector_u256(
            log.data
                .topics()
                .iter()
                .map(|x| to_move_u256(&U256::from_le_bytes(x.0))),
        ),
        Value::vector_u8(log.data.data),
    ];
    Value::struct_(Struct::pack(fields))
}

fn evm_result_to_move_value(result: ExecutionResult) -> Value {
    let fields = [
        Value::bool(result.is_success()),
        Value::vector_u8(result.output().cloned().unwrap_or_default()),
        // TODO: this method says it's for testing only, but it seems
        // to be the only way to make a Vector of Structs.
        Value::vector_for_testing_only(result.into_logs().into_iter().map(evm_log_to_move_value)),
    ];
    Value::struct_(Struct::pack(fields))
}

fn evm_call(
    context: &mut SafeNativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> SafeNativeResult<SmallVec<[Value; 1]>> {
    debug_assert!(ty_args.is_empty(), "No ty_args in EVM native");
    debug_assert_eq!(
        args.len(),
        4,
        "EVM native args should be from, to, value, data"
    );

    // Safety: unwrap is safe because of the length check above
    // Note: the `safely_pop_vec_arg` macro does not work well for `Vec<u8>`
    // because it has a special runtime representation.
    let data = args.pop_back().unwrap().value_as::<Vec<u8>>()?;
    let value = safely_pop_arg!(args, move_core_types::u256::U256);
    let transact_to = safely_pop_arg!(args, AccountAddress);
    let caller = safely_pop_arg!(args, AccountAddress);

    evm_transact_inner(
        context,
        caller.to_eth_address(),
        TxKind::Call(transact_to.to_eth_address()),
        from_move_u256(value),
        data,
    )
}

fn evm_create(
    context: &mut SafeNativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> SafeNativeResult<SmallVec<[Value; 1]>> {
    debug_assert!(ty_args.is_empty(), "No ty_args in EVM native");
    debug_assert_eq!(args.len(), 3, "EVM native args should be from, value, data");

    // Safety: unwrap is safe because of the length check above
    // Note: the `safely_pop_vec_arg` macro does not work well for `Vec<u8>`
    // because it has a special runtime representation.
    let data = args.pop_back().unwrap().value_as::<Vec<u8>>()?;
    let value = safely_pop_arg!(args, move_core_types::u256::U256);
    let caller = safely_pop_arg!(args, AccountAddress);

    evm_transact_inner(
        context,
        caller.to_eth_address(),
        TxKind::Create,
        from_move_u256(value),
        data,
    )
}

fn evm_transact_inner(
    context: &mut SafeNativeContext,
    caller: Address,
    transact_to: TxKind,
    value: U256,
    data: Vec<u8>,
) -> SafeNativeResult<SmallVec<[Value; 1]>> {
    // TODO: does it make sense for EVM gas to be 1:1 with MoveVM gas?
    let gas_limit: u64 = context.gas_balance().into();

    let evm_native_ctx = context.extensions_mut().get_mut::<NativeEVMContext>();
    // TODO: also need to set block env context
    let mut evm = Evm::builder()
        .with_db(&mut evm_native_ctx.db)
        .with_tx_env(TxEnv {
            caller,
            gas_limit,
            // Gas price can be zero here because fee is charged in the MoveVM
            gas_price: U256::ZERO,
            transact_to,
            value,
            data: data.into(),
            // Nonce and chain id can be None because replay attacks
            // are prevented at the MoveVM level. I.e. replay will
            // never occur because the MoveVM will not accept a duplicate
            // transaction
            nonce: None,
            chain_id: None,
            // TODO: could maybe construct something based on the values that
            // have already been accessed in `context.traversal_context()`.
            access_list: Vec::new(),
            gas_priority_fee: None,
            blob_hashes: Vec::new(),
            max_fee_per_blob_gas: None,
            authorization_list: None,
        })
        .build();

    let outcome = evm.transact().map_err(|e| match e {
        EVMError::Database(e) => SafeNativeError::InvariantViolation(e),
        other => SafeNativeError::InvariantViolation(
            PartialVMError::new(StatusCode::ABORTED).with_message(format!("EVM Error: {other:?}")),
        ),
    })?;
    drop(evm);

    // TODO: need to figure out how to charge gas using the SafeNativeContext.
    // context.charge(outcome.result.gas_used())?;

    // Capture changes in native context so that they can be
    // converted into Move changes when the session is finalized
    evm_native_ctx.state_changes.push(outcome.state.clone());

    // Commit the changes to the DB so that future Move transactions using
    // the same session will see them.
    evm_native_ctx.db.commit(outcome.state);

    let result = outcome.result;
    Ok(smallvec::smallvec![evm_result_to_move_value(result)])
}

struct ResolverBackedDB<'a> {
    resolver: &'a dyn MoveResolver<PartialVMError>,
}

impl<'a> DatabaseRef for ResolverBackedDB<'a> {
    type Error = PartialVMError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        let struct_tag = account_info_struct_tag(&address);
        let resource = self
            .resolver
            .get_resource(&EVM_NATIVE_ADDRESS, &struct_tag)?;
        let value = resource.map(|bytes| {
            Value::simple_deserialize(&bytes, &ACCOUNT_INFO_LAYOUT)
                .expect("EVM account info must deserialize correctly.")
        });
        let info = value.map(move_value_to_account_info).transpose()?;
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
        let struct_tag = account_storage_struct_tag(&address, &index);
        let value = match self
            .resolver
            .get_resource(&EVM_NATIVE_ADDRESS, &struct_tag)?
        {
            Some(bytes) => {
                let value = Value::simple_deserialize(&bytes, &ACCOUNT_STORAGE_LAYOUT)
                    .expect("EVM account storage must deserialize correctly");
                from_move_u256(value.cast()?)
            }
            None => {
                // Zero is the default value when there is no entry
                return Ok(U256::ZERO);
            }
        };
        Ok(value)
    }

    fn block_hash_ref(&self, _number: u64) -> Result<B256, Self::Error> {
        // Complication: Move doesn't support this API out of the box.
        // We could build it out ourselves, but maybe it's not needed
        // for the contracts we want to support?

        unimplemented!("EVM block hash API not implemented")
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            genesis::config::GenesisConfig,
            move_execution::{create_move_vm, create_vm_session, execute_transaction, tests::*},
            storage::State,
            tests::{signer::Signer, ALT_EVM_ADDRESS, EVM_ADDRESS, PRIVATE_KEY},
            types::session_id::SessionId,
        },
        alloy::{
            primitives::utils::parse_ether,
            providers::{self, network::AnyNetwork},
            sol,
        },
        aptos_table_natives::TableResolver,
        aptos_types::transaction::EntryFunction,
        move_core_types::{language_storage::ModuleId, value::MoveValue},
        move_vm_runtime::{
            module_traversal::{TraversalContext, TraversalStorage},
            session::SerializedReturnValues,
        },
        move_vm_types::{gas::UnmeteredGasMeter, values::Vector},
    };

    sol!(
        #[sol(rpc)]
        ERC20,
        "../server/src/tests/res/ERC20.json"
    );

    #[test]
    fn test_account_info_round_trip() {
        let bytecode = Bytecode::new();
        let account_info = AccountInfo {
            balance: U256::from(1234),
            nonce: 7,
            code_hash: bytecode.hash_slow(),
            code: None,
        };
        let value = account_info_to_move_value(&account_info, account_info.code_hash);
        let info_rt = move_value_to_account_info(value).unwrap();
        assert_eq!(account_info, info_rt);
    }

    /// Tests that EVM native works by deploying an ERC-20 contract and
    /// then having a user transfer some tokens between accounts.
    #[test]
    fn test_evm() {
        // -------- Initialize state
        let genesis_config = GenesisConfig::default();
        let mut signer = Signer::new(&PRIVATE_KEY);
        let (_, mut state) = deploy_contract("natives", &mut signer, &genesis_config);

        // -------- Setup ERC-20 interface
        let mint_amount = parse_ether("1").unwrap();
        let provider = providers::builder::<AnyNetwork>()
            .with_recommended_fillers()
            .on_http("http://localhost:1234".parse().unwrap());
        let deploy = ERC20::deploy_builder(
            &provider,
            "Gold".into(),
            "AU".into(),
            EVM_ADDRESS,
            mint_amount,
        );

        // -------- Deploy ERC-20 token
        let (outcome, mut changes, extensions) =
            evm_quick_create(deploy.calldata().to_vec(), state.resolver());

        assert!(outcome.is_success, "Contract deploy must succeed");

        // The ERC-20 contract produces a log because it minted some tokens.
        // We can use this log to get the address of the newly deployed contract.
        let contract_address = outcome.logs[0].address;
        let deployed_contract = ERC20::new(contract_address, &provider);

        let evm_changes = extract_evm_changes(&extensions);
        changes.squash(evm_changes).unwrap();
        drop(extensions);

        state.apply(changes).unwrap();

        // -------- Transfer ERC-20 tokens
        let transfer_amount = parse_ether("0.25").unwrap();
        let user_address = EVM_ADDRESS.to_move_address();
        let signer_input_arg = MoveValue::Signer(user_address);
        let to_input_arg = MoveValue::Address(contract_address.to_move_address());
        let transfer_call = deployed_contract.transfer(ALT_EVM_ADDRESS, transfer_amount);
        let data_input_arg = Value::vector_u8(transfer_call.calldata().clone());
        let entry_fn = EntryFunction::new(
            ModuleId::new(EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE.into()),
            ident_str!("entry_evm_call").into(),
            Vec::new(),
            vec![
                bcs::to_bytes(&signer_input_arg).unwrap(),
                bcs::to_bytes(&to_input_arg).unwrap(),
                data_input_arg.simple_serialize(&CODE_LAYOUT).unwrap(),
            ],
        );
        let (tx_hash, tx) = create_transaction(
            &mut signer,
            TxKind::Call(EVM_NATIVE_ADDRESS.to_eth_address()),
            bcs::to_bytes(&entry_fn).unwrap(),
        );

        let outcome =
            execute_transaction(&tx, &tx_hash, state.resolver(), &genesis_config, 0, &()).unwrap();
        outcome.vm_outcome.unwrap();
        state.apply(outcome.changes).unwrap();

        // -------- Validate ERC-20 balances
        let contract_move_address = contract_address.to_move_address();
        let balance_of = |address| {
            let balance_of_call = deployed_contract.balanceOf(address);
            let (outcome, _, _) = evm_quick_call(
                EVM_NATIVE_ADDRESS,
                contract_move_address,
                balance_of_call.calldata().to_vec(),
                state.resolver(),
            );
            U256::from_be_slice(&outcome.output)
        };
        let sender_balance = balance_of(EVM_ADDRESS);
        let receiver_balance = balance_of(ALT_EVM_ADDRESS);

        assert_eq!(sender_balance, mint_amount - transfer_amount);
        assert_eq!(receiver_balance, transfer_amount);
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct EvmNativeOutcome {
        is_success: bool,
        output: Vec<u8>,
        logs: Vec<Log>,
    }

    /// Create MoveVM instance and invoke EVM create native.
    /// For tests only since it does not use an existing session or charge gas.
    fn evm_quick_create(
        contract_bytecode: Vec<u8>,
        resolver: &(impl MoveResolver<PartialVMError> + TableResolver),
    ) -> (EvmNativeOutcome, ChangeSet, NativeContextExtensions) {
        let move_vm = create_move_vm().unwrap();
        let session_id = SessionId::default();
        let mut session = create_vm_session(&move_vm, resolver, session_id);
        let traversal_storage = TraversalStorage::new();
        let mut traversal_context = TraversalContext::new(&traversal_storage);
        let mut gas_meter = UnmeteredGasMeter;

        let module_id = ModuleId::new(EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE.into());
        let args = vec![
            // From
            Value::address(EVM_NATIVE_ADDRESS)
                .simple_serialize(&MoveTypeLayout::Address)
                .unwrap(),
            // Value
            serialize_fungible_asset_value(0),
            // Data (code to deploy)
            Value::vector_u8(contract_bytecode)
                .simple_serialize(&CODE_LAYOUT)
                .unwrap(),
        ];

        let outcome = session
            .execute_function_bypass_visibility(
                &module_id,
                ident_str!("evm_create"),
                Vec::new(),
                args,
                &mut gas_meter,
                &mut traversal_context,
            )
            .unwrap();

        let outcome = extract_evm_result(outcome);
        let (changes, extensions) = session.finish_with_extensions().unwrap();
        (outcome, changes, extensions)
    }

    /// Create MoveVM instance and invoke EVM call native.
    /// For tests only since it does not use an existing session or charge gas.
    fn evm_quick_call(
        from: AccountAddress,
        to: AccountAddress,
        data: Vec<u8>,
        resolver: &(impl MoveResolver<PartialVMError> + TableResolver),
    ) -> (EvmNativeOutcome, ChangeSet, NativeContextExtensions) {
        let move_vm = create_move_vm().unwrap();
        let session_id = SessionId::default();
        let mut session = create_vm_session(&move_vm, resolver, session_id);
        let traversal_storage = TraversalStorage::new();
        let mut traversal_context = TraversalContext::new(&traversal_storage);
        let mut gas_meter = UnmeteredGasMeter;

        let module_id = ModuleId::new(EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE.into());
        let args = vec![
            // From
            Value::address(from)
                .simple_serialize(&MoveTypeLayout::Address)
                .unwrap(),
            // to
            Value::address(to)
                .simple_serialize(&MoveTypeLayout::Address)
                .unwrap(),
            // Value
            serialize_fungible_asset_value(0),
            // Data (code to deploy)
            Value::vector_u8(data)
                .simple_serialize(&CODE_LAYOUT)
                .unwrap(),
        ];

        let outcome = session
            .execute_function_bypass_visibility(
                &module_id,
                ident_str!("evm_call"),
                Vec::new(),
                args,
                &mut gas_meter,
                &mut traversal_context,
            )
            .unwrap();

        let outcome = extract_evm_result(outcome);
        let (changes, extensions) = session.finish_with_extensions().unwrap();
        (outcome, changes, extensions)
    }

    fn extract_evm_result(outcome: SerializedReturnValues) -> EvmNativeOutcome {
        let mut return_values = outcome
            .return_values
            .into_iter()
            .map(|(bytes, layout)| Value::simple_deserialize(&bytes, &layout).unwrap());

        let mut evm_result_fields = return_values
            .next()
            .unwrap()
            .value_as::<Struct>()
            .unwrap()
            .unpack()
            .unwrap();

        assert!(
            return_values.next().is_none(),
            "There is only one return value."
        );

        let is_success: bool = evm_result_fields.next().unwrap().value_as().unwrap();
        let output: Vec<u8> = evm_result_fields.next().unwrap().value_as().unwrap();
        let logs: Vec<Value> = evm_result_fields.next().unwrap().value_as().unwrap();
        let logs = logs
            .into_iter()
            .map(|value| {
                let mut fields = value.value_as::<Struct>().unwrap().unpack().unwrap();

                let address = fields.next().unwrap().value_as::<AccountAddress>().unwrap();
                let topics = fields
                    .next()
                    .unwrap()
                    .value_as::<Vector>()
                    .unwrap()
                    .unpack_unchecked()
                    .unwrap();
                let data = fields.next().unwrap().value_as::<Vec<u8>>().unwrap();

                Log::new(
                    address.to_eth_address(),
                    topics
                        .into_iter()
                        .map(|value| {
                            value
                                .value_as::<move_core_types::u256::U256>()
                                .unwrap()
                                .to_le_bytes()
                                .into()
                        })
                        .collect(),
                    data.into(),
                )
                .unwrap()
            })
            .collect();

        assert!(
            evm_result_fields.next().is_none(),
            "There are only 3 field in EVM return value."
        );

        EvmNativeOutcome {
            is_success,
            output,
            logs,
        }
    }

    /// Serialize a number as a Move fungible asset type.
    /// This is needed to directly call the EVM natives which
    /// take `value` as a fungible asset.
    fn serialize_fungible_asset_value(value: u64) -> Vec<u8> {
        // Fungible asset Move type is a struct with two fields:
        // 1. another struct with a single address field,
        // 2. a u64 value.
        let fungible_asset_layout = MoveTypeLayout::Struct(MoveStructLayout::Runtime(vec![
            MoveTypeLayout::Struct(MoveStructLayout::Runtime(vec![MoveTypeLayout::Address])),
            MoveTypeLayout::U64,
        ]));

        Value::struct_(Struct::pack([
            Value::struct_(Struct::pack([Value::address(AccountAddress::ZERO)])),
            Value::u64(value),
        ]))
        .simple_serialize(&fungible_asset_layout)
        .unwrap()
    }
}
