use {
    crate::{
        genesis::config::GenesisConfig,
        move_execution::signers::check_signer,
        types::transactions::{
            ExtendedTxEnvelope, NormalizedEthTransaction, TransactionExecutionOutcome,
        },
        Error::{InvalidTransaction, User},
        InvalidTransactionCause,
    },
    alloy_consensus::Transaction,
    alloy_primitives::TxKind,
    aptos_framework::natives::event::NativeEventContext,
    aptos_gas_meter::{AptosGasMeter, GasAlgebra, StandardGasAlgebra, StandardGasMeter},
    aptos_gas_schedule::{MiscGasParameters, NativeGasParameters, LATEST_GAS_FEATURE_VERSION},
    aptos_table_natives::{NativeTableContext, TableResolver},
    aptos_types::{
        on_chain_config::{Features, TimedFeaturesBuilder},
        transaction::{EntryFunction, Module},
    },
    aptos_vm::natives::aptos_natives,
    move_binary_format::errors::PartialVMError,
    move_core_types::{account_address::AccountAddress, resolver::MoveResolver},
    move_vm_runtime::{
        module_traversal::{TraversalContext, TraversalStorage},
        move_vm::MoveVM,
        native_extensions::NativeContextExtensions,
        session::Session,
    },
    move_vm_types::{gas::GasMeter, loaded_data::runtime_types::Type, values::Value},
    nonces::check_nonce,
    tag_validation::validate_entry_type_tag,
};

mod eth_token;
mod nonces;
mod signers;
mod tag_validation;

pub fn create_move_vm() -> crate::Result<MoveVM> {
    let natives = aptos_natives(
        LATEST_GAS_FEATURE_VERSION,
        NativeGasParameters::zeros(),
        MiscGasParameters::zeros(),
        TimedFeaturesBuilder::enable_all().build(),
        Features::default(),
    );
    let vm = MoveVM::new(natives)?;
    Ok(vm)
}

pub fn create_vm_session<'l, 'r, S>(vm: &'l MoveVM, state: &'r S) -> Session<'r, 'l>
where
    S: MoveResolver<PartialVMError> + TableResolver,
{
    let mut native_extensions = NativeContextExtensions::default();

    // Events are used in `eth_token` because it depends on `fungible_asset`.
    native_extensions.add(NativeEventContext::default());

    // Tables can be used
    // TODO: what is the right value for txn_hash?
    native_extensions.add(NativeTableContext::new([0; 32], state));

    vm.new_session_with_extensions(state, native_extensions)
}

pub fn execute_transaction(
    tx: &ExtendedTxEnvelope,
    state: &(impl MoveResolver<PartialVMError> + TableResolver),
    genesis_config: &GenesisConfig,
) -> crate::Result<TransactionExecutionOutcome> {
    match tx {
        ExtendedTxEnvelope::DepositedTx(tx) => {
            // TODO: handle U256 properly
            let amount = tx.mint.as_limbs()[0].saturating_add(tx.value.as_limbs()[0]);
            let to = evm_address_to_move_address(&tx.to);

            let move_vm = create_move_vm()?;
            let mut session = create_vm_session(&move_vm, state);
            let traversal_storage = TraversalStorage::new();
            let mut traversal_context = TraversalContext::new(&traversal_storage);
            // The type of `tx.gas` is essentially `[u64; 1]` so taking the 0th element
            // is a 1:1 mapping to `u64`.
            let mut gas_meter = new_gas_meter(genesis_config, tx.gas.as_limbs()[0]);

            eth_token::mint_eth(
                &to,
                amount,
                &mut session,
                &mut traversal_context,
                &mut gas_meter,
            )?;

            debug_assert!(
                eth_token::get_eth_balance(
                    &to,
                    &mut session,
                    &mut traversal_context,
                    &mut gas_meter
                )
                .unwrap()
                    >= amount,
                "tokens were minted"
            );

            let changes = session.finish()?;
            let gas_used = total_gas_used(&gas_meter, genesis_config);

            Ok(TransactionExecutionOutcome::new(Ok(()), changes, gas_used))
        }
        ExtendedTxEnvelope::Canonical(tx) => {
            if let Some(chain_id) = tx.chain_id() {
                if chain_id != genesis_config.chain_id {
                    return Err(InvalidTransactionCause::IncorrectChainId.into());
                }
            }

            let tx = NormalizedEthTransaction::try_from(tx.clone())?;
            let sender_move_address = evm_address_to_move_address(&tx.signer);

            let move_vm = create_move_vm()?;
            let mut session = create_vm_session(&move_vm, state);
            let traversal_storage = TraversalStorage::new();
            let mut traversal_context = TraversalContext::new(&traversal_storage);
            let mut gas_meter = new_gas_meter(genesis_config, tx.gas_limit());

            // Charge gas for the transaction itself.
            // Immediately exit if there is not enough.
            let txn_size = (tx.data.len() as u64).into();
            let charge_gas = gas_meter
                .charge_intrinsic_gas_for_transaction(txn_size)
                .and_then(|_| gas_meter.charge_io_gas_for_transaction(txn_size));
            if charge_gas.is_err() {
                return Err(InvalidTransaction(
                    InvalidTransactionCause::InsufficientIntrinsicGas,
                ));
            }

            check_nonce(
                tx.nonce,
                &sender_move_address,
                &mut session,
                &mut traversal_context,
                &mut gas_meter,
            )?;

            // TODO: How to model script-type transactions?
            let vm_outcome = match tx.to {
                TxKind::Call(_to) => {
                    let entry_fn: EntryFunction = bcs::from_bytes(&tx.data)?;
                    if entry_fn.module().address() != &sender_move_address {
                        Err(InvalidTransactionCause::InvalidDestination)?
                    }
                    execute_entry_function(
                        entry_fn,
                        &sender_move_address,
                        &mut session,
                        &mut traversal_context,
                        &mut gas_meter,
                    )
                }
                TxKind::Create => {
                    // Assume EVM create type transactions are module deployments in Move
                    let module = Module::new(tx.data.to_vec());
                    deploy_module(
                        module,
                        evm_address_to_move_address(&tx.signer),
                        &mut session,
                        &mut gas_meter,
                    )
                }
            };

            let changes = session.finish()?;
            let gas_used = total_gas_used(&gas_meter, genesis_config);

            match vm_outcome {
                Ok(_) => Ok(TransactionExecutionOutcome::new(Ok(()), changes, gas_used)),
                // User error still generates a receipt and consumes gas
                Err(User(e)) => Ok(TransactionExecutionOutcome::new(Err(e), changes, gas_used)),
                Err(e) => Err(e),
            }
        }
    }
}

fn execute_entry_function<G: GasMeter>(
    entry_fn: EntryFunction,
    signer: &AccountAddress,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
) -> crate::Result<()> {
    let (module_id, function_name, ty_args, args) = entry_fn.into_inner();

    // Validate signer params match the actual signer
    let function = session.load_function(&module_id, &function_name, &ty_args)?;
    if function.param_tys.len() != args.len() {
        Err(InvalidTransactionCause::MismatchedArgumentCount)?;
    }
    for (ty, bytes) in function.param_tys.iter().zip(&args) {
        // References are ignored in entry function signatures because the
        // values are actualized in the serialized arguments.
        let ty = strip_reference(ty)?;
        // Note: the function is safe even though the `get_type_tag` implementation
        // has unbounded recursion in it because the recursion depth is limited at
        // the time a module is deployed. If a module has been successfully deployed
        // then we know the recursion is bounded to a reasonable degree (less than depth 255).
        // See `test_deeply_nested_type`.
        let tag = session.get_type_tag(ty)?;
        validate_entry_type_tag(&tag)?;
        let layout = session.get_type_layout(&tag)?;
        // TODO: Potential optimization -- could check layout for Signer type
        // and only deserialize if necessary. The tricky part here is we would need
        // to keep track of the recursive path through the type.
        let arg = Value::simple_deserialize(bytes, &layout)
            .ok_or(InvalidTransactionCause::FailedArgumentDeserialization)?
            .as_move_value(&layout);
        // Note: no recursion limit is needed in this function because we have already
        // constructed the recursive types `Type`, `TypeTag`, `MoveTypeLayout` and `MoveValue` so
        // the values must have respected whatever recursion limit is present in MoveVM.
        check_signer(&arg, signer)?;
    }

    // TODO: is this the right way to be using the VM?
    // Maybe there is some higher level entry point we should be using instead?
    session.execute_entry_function(
        &module_id,
        &function_name,
        ty_args,
        args,
        gas_meter,
        traversal_context,
    )?;
    Ok(())
}

// If `t` is wrapped in `Type::Reference` or `Type::MutableReference`,
// return the inner type
fn strip_reference(t: &Type) -> crate::Result<&Type> {
    match t {
        Type::Reference(inner) | Type::MutableReference(inner) => {
            match inner.as_ref() {
                Type::Reference(_) | Type::MutableReference(_) => {
                    // Based on Aptos code, it looks like references are not allowed to be nested.
                    // TODO: check this assumption.
                    Err(InvalidTransactionCause::UnsupportedNestedReference)?
                }
                other => Ok(other),
            }
        }
        other => Ok(other),
    }
}

fn deploy_module<G: GasMeter>(
    code: Module,
    address: AccountAddress,
    session: &mut Session,
    gas_meter: &mut G,
) -> crate::Result<()> {
    session.publish_module(code.into_inner(), address, gas_meter)?;

    Ok(())
}

// TODO: is there a way to make Move use 32-byte addresses?
fn evm_address_to_move_address(address: &alloy_primitives::Address) -> AccountAddress {
    let mut bytes = [0; 32];
    bytes[12..32].copy_from_slice(address.as_slice());
    AccountAddress::new(bytes)
}

fn new_gas_meter(
    genesis_config: &GenesisConfig,
    gas_limit: u64,
) -> StandardGasMeter<StandardGasAlgebra> {
    StandardGasMeter::new(StandardGasAlgebra::new(
        genesis_config.gas_costs.version,
        genesis_config.gas_costs.vm.clone(),
        genesis_config.gas_costs.storage.clone(),
        false,
        gas_limit,
    ))
}

fn total_gas_used<G: AptosGasMeter>(gas_meter: &G, genesis_config: &GenesisConfig) -> u64 {
    let gas_algebra = gas_meter.algebra();
    // Note: this sum is overflow safe because it uses saturating addition
    // by default in the implementation of `GasQuantity`.
    let total = gas_algebra.execution_gas_used()
        + gas_algebra.io_gas_used()
        + gas_algebra.storage_fee_used_in_gas_units();
    let total: u64 = total.into();
    // Aptos scales up the input gas limit for some reason,
    // so we need to reverse that scaling when we return.
    let scaling_factor: u64 = genesis_config.gas_costs.vm.txn.scaling_factor().into();
    total / scaling_factor
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            genesis::{config::CHAIN_ID, init_storage},
            storage::{InMemoryState, State},
            tests::{signer::Signer, EVM_ADDRESS, PRIVATE_KEY},
            types::transactions::DepositedTx,
        },
        alloy::network::TxSignerSync,
        alloy_consensus::{transaction::TxEip1559, SignableTransaction, TxEnvelope},
        alloy_primitives::{FixedBytes, U256, U64},
        anyhow::Context,
        move_binary_format::{
            file_format::{
                AbilitySet, FieldDefinition, IdentifierIndex, ModuleHandleIndex, SignatureToken,
                StructDefinition, StructFieldInformation, StructHandle, StructHandleIndex,
                TypeSignature,
            },
            CompiledModule,
        },
        move_compiler::{
            shared::{NumberFormat, NumericalAddress},
            Compiler, Flags,
        },
        move_core_types::{
            identifier::Identifier,
            language_storage::{ModuleId, StructTag},
            resolver::ModuleResolver,
            value::{MoveStruct, MoveValue},
        },
        move_vm_types::gas::UnmeteredGasMeter,
        std::{collections::BTreeSet, u64},
    };

    #[test]
    fn test_execute_counter_contract() {
        let genesis_config = GenesisConfig::default();
        let module_name = "counter";
        let mut signer = Signer::new(&PRIVATE_KEY);
        let (module_id, mut state) = deploy_contract(module_name, &mut signer, &genesis_config);

        // Call entry function to create the `Counter` resource
        let move_address = evm_address_to_move_address(&EVM_ADDRESS);
        let initial_value: u64 = 7;
        let signer_arg = MoveValue::Signer(move_address);
        let entry_fn = EntryFunction::new(
            module_id.clone(),
            Identifier::new("publish").unwrap(),
            Vec::new(),
            vec![
                bcs::to_bytes(&signer_arg).unwrap(),
                bcs::to_bytes(&initial_value).unwrap(),
            ],
        );
        let signed_tx = create_transaction(
            &mut signer,
            TxKind::Call(EVM_ADDRESS),
            bcs::to_bytes(&entry_fn).unwrap(),
        );

        let outcome = execute_transaction(&signed_tx, state.resolver(), &genesis_config).unwrap();
        state.apply(outcome.changes).unwrap();

        // Calling the function with an incorrect signer causes an error
        let signer_arg = MoveValue::Signer(AccountAddress::new([0x00; 32]));
        let entry_fn = EntryFunction::new(
            module_id.clone(),
            Identifier::new("publish").unwrap(),
            Vec::new(),
            vec![
                bcs::to_bytes(&signer_arg).unwrap(),
                bcs::to_bytes(&initial_value).unwrap(),
            ],
        );
        let signed_tx = create_transaction(
            &mut signer,
            TxKind::Call(EVM_ADDRESS),
            bcs::to_bytes(&entry_fn).unwrap(),
        );
        let err = execute_transaction(&signed_tx, state.resolver(), &genesis_config).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Signer does not match transaction signature"
        );
        // Reverse the nonce incrementing done in `create_transaction` because of the error
        signer.nonce -= 1;

        // Resource was created
        let struct_tag = StructTag {
            address: move_address,
            module: Identifier::new(module_name).unwrap(),
            name: Identifier::new("Counter").unwrap(),
            type_args: Vec::new(),
        };
        let resource: u64 = bcs::from_bytes(
            &state
                .resolver()
                .get_resource(&move_address, &struct_tag)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(resource, initial_value);

        // Call entry function to increment the counter
        let address_arg = MoveValue::Address(move_address);
        let entry_fn = EntryFunction::new(
            module_id,
            Identifier::new("increment").unwrap(),
            Vec::new(),
            vec![bcs::to_bytes(&address_arg).unwrap()],
        );
        let signed_tx = create_transaction(
            &mut signer,
            TxKind::Call(EVM_ADDRESS),
            bcs::to_bytes(&entry_fn).unwrap(),
        );

        let outcome = execute_transaction(&signed_tx, state.resolver(), &genesis_config).unwrap();
        state.apply(outcome.changes).unwrap();

        // Resource was modified
        let resource: u64 = bcs::from_bytes(
            &state
                .resolver()
                .get_resource(&move_address, &struct_tag)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(resource, initial_value + 1);
    }

    #[test]
    fn test_execute_signer_struct_contract() {
        let genesis_config = GenesisConfig::default();
        let module_name = "signer_struct";
        let mut signer = Signer::new(&PRIVATE_KEY);
        let (module_id, mut state) = deploy_contract(module_name, &mut signer, &genesis_config);

        // Call main function with correct signer
        let move_address = evm_address_to_move_address(&EVM_ADDRESS);
        let input_arg = MoveValue::Struct(MoveStruct::new(vec![MoveValue::Vector(vec![
            MoveValue::Signer(move_address),
        ])]));
        let entry_fn = EntryFunction::new(
            module_id.clone(),
            Identifier::new("main").unwrap(),
            Vec::new(),
            vec![bcs::to_bytes(&input_arg).unwrap()],
        );
        let signed_tx = create_transaction(
            &mut signer,
            TxKind::Call(EVM_ADDRESS),
            bcs::to_bytes(&entry_fn).unwrap(),
        );

        let outcome = execute_transaction(&signed_tx, state.resolver(), &genesis_config).unwrap();
        assert!(outcome.vm_outcome.is_ok());
        state.apply(outcome.changes).unwrap();

        // Call main function with incorrect signer (get an error)
        let input_arg = MoveValue::Struct(MoveStruct::new(vec![MoveValue::Vector(vec![
            MoveValue::Signer(AccountAddress::new([0x11; 32])),
        ])]));
        let entry_fn = EntryFunction::new(
            module_id.clone(),
            Identifier::new("main").unwrap(),
            Vec::new(),
            vec![bcs::to_bytes(&input_arg).unwrap()],
        );
        let signed_tx = create_transaction(
            &mut signer,
            TxKind::Call(EVM_ADDRESS),
            bcs::to_bytes(&entry_fn).unwrap(),
        );

        let err = execute_transaction(&signed_tx, state.resolver(), &genesis_config).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Signer does not match transaction signature"
        );
    }

    #[test]
    fn test_execute_natives_contract() {
        let genesis_config = GenesisConfig::default();
        let mut signer = Signer::new(&PRIVATE_KEY);
        let (module_id, state) = deploy_contract("natives", &mut signer, &genesis_config);

        // Call entry function to run the internal native hashing methods
        let entry_fn = EntryFunction::new(
            module_id,
            Identifier::new("hashing").unwrap(),
            Vec::new(),
            vec![],
        );
        let signed_tx = create_transaction(
            &mut signer,
            TxKind::Call(EVM_ADDRESS),
            bcs::to_bytes(&entry_fn).unwrap(),
        );

        let changes = execute_transaction(&signed_tx, state.resolver(), &genesis_config);
        assert!(changes.is_ok());
    }

    /// Deposits can be made to the L2.
    #[test]
    fn test_deposit_tx() {
        let genesis_config = GenesisConfig::default();
        let mut signer = Signer::new(&PRIVATE_KEY);
        let (_, state) = deploy_contract("natives", &mut signer, &genesis_config);

        let mint_amount = U256::from(123u64);
        let tx = ExtendedTxEnvelope::DepositedTx(DepositedTx {
            to: EVM_ADDRESS,
            value: mint_amount,
            source_hash: FixedBytes::default(),
            from: EVM_ADDRESS,
            mint: U256::ZERO,
            gas: U64::from(u64::MAX),
            is_system_tx: false,
            data: Vec::new().into(),
        });

        execute_transaction(&tx, state.resolver(), &genesis_config)
            .unwrap()
            .vm_outcome
            .unwrap();
    }

    #[test]
    fn test_transaction_replay_is_forbidden() {
        // Transaction replay is forbidden by the nonce checking.

        let genesis_config = GenesisConfig::default();

        // Deploy a contract
        let mut signer = Signer::new(&PRIVATE_KEY);
        let (module_id, mut state) = deploy_contract("natives", &mut signer, &genesis_config);

        // Use a transaction to call a function; this passes
        let entry_fn = EntryFunction::new(
            module_id,
            Identifier::new("hashing").unwrap(),
            Vec::new(),
            vec![],
        );
        let signed_tx = create_transaction(
            &mut signer,
            TxKind::Call(EVM_ADDRESS),
            bcs::to_bytes(&entry_fn).unwrap(),
        );

        let outcome = execute_transaction(&signed_tx, state.resolver(), &genesis_config).unwrap();
        state.apply(outcome.changes).unwrap();

        // Send the same transaction again; this fails with a nonce error
        let err = execute_transaction(&signed_tx, state.resolver(), &genesis_config).unwrap_err();
        assert_eq!(err.to_string(), "Incorrect nonce: given=1 expected=2");
    }

    #[test]
    fn test_transaction_chain_id() {
        let genesis_config = GenesisConfig::default();

        // Deploy a contract
        let mut signer = Signer::new(&PRIVATE_KEY);
        let (module_id, state) = deploy_contract("natives", &mut signer, &genesis_config);

        // Use a transaction to call a function but pass the wrong chain id
        let entry_fn = EntryFunction::new(
            module_id,
            Identifier::new("hashing").unwrap(),
            Vec::new(),
            vec![],
        );
        let mut tx = TxEip1559 {
            // Intentionally setting the wrong chain id
            chain_id: genesis_config.chain_id + 1,
            nonce: signer.nonce,
            gas_limit: u64::MAX.into(),
            max_fee_per_gas: 0,
            max_priority_fee_per_gas: 0,
            to: TxKind::Call(EVM_ADDRESS),
            value: Default::default(),
            access_list: Default::default(),
            input: bcs::to_bytes(&entry_fn).unwrap().into(),
        };
        let signature = signer.inner.sign_transaction_sync(&mut tx).unwrap();
        let signed_tx =
            ExtendedTxEnvelope::Canonical(TxEnvelope::Eip1559(tx.into_signed(signature)));

        let err = execute_transaction(&signed_tx, state.resolver(), &genesis_config).unwrap_err();
        assert_eq!(err.to_string(), "Incorrect chain id");
    }

    #[test]
    fn test_out_of_gas() {
        let genesis_config = GenesisConfig::default();

        // Deploy a contract
        let mut signer = Signer::new(&PRIVATE_KEY);
        let (module_id, state) = deploy_contract("natives", &mut signer, &genesis_config);

        // Use a transaction to call a function but pass in too little gas
        let entry_fn = EntryFunction::new(
            module_id,
            Identifier::new("hashing").unwrap(),
            Vec::new(),
            vec![],
        );
        let mut tx = TxEip1559 {
            chain_id: genesis_config.chain_id,
            nonce: signer.nonce,
            // Intentionally pass a small amount of gas
            gas_limit: 1,
            max_fee_per_gas: 0,
            max_priority_fee_per_gas: 0,
            to: TxKind::Call(EVM_ADDRESS),
            value: Default::default(),
            access_list: Default::default(),
            input: bcs::to_bytes(&entry_fn).unwrap().into(),
        };
        let signature = signer.inner.sign_transaction_sync(&mut tx).unwrap();
        let signed_tx =
            ExtendedTxEnvelope::Canonical(TxEnvelope::Eip1559(tx.into_signed(signature)));

        let err = execute_transaction(&signed_tx, state.resolver(), &genesis_config).unwrap_err();
        assert_eq!(err.to_string(), "Insufficient intrinsic gas");
    }

    #[test]
    fn test_execute_tables_contract() {
        let genesis_config = GenesisConfig::default();
        let module_name = "tables";
        let mut signer = Signer::new(&PRIVATE_KEY);
        let (module_id, state) = deploy_contract(module_name, &mut signer, &genesis_config);
        let vm = create_move_vm().unwrap();
        let traversal_storage = TraversalStorage::new();

        let mut session = create_vm_session(&vm, state.resolver());
        let mut traversal_context = TraversalContext::new(&traversal_storage);

        let move_address = evm_address_to_move_address(&EVM_ADDRESS);
        let signer_arg = MoveValue::Signer(move_address);
        let entry_fn = EntryFunction::new(
            module_id.clone(),
            Identifier::new("make_test_tables").unwrap(),
            Vec::new(),
            vec![bcs::to_bytes(&signer_arg).unwrap()],
        );
        let (module_id, function_name, ty_args, args) = entry_fn.into_inner();

        session
            .execute_entry_function(
                &module_id,
                &function_name,
                ty_args,
                args,
                &mut UnmeteredGasMeter,
                &mut traversal_context,
            )
            .unwrap();

        let (_change_set, mut extensions) = session.finish_with_extensions().unwrap();
        let table_change_set = extensions
            .remove::<NativeTableContext>()
            .into_change_set()
            .unwrap();

        // tables.move creates 11 new tables and makes 11 changes
        const TABLE_CHANGE_SET_NEW_TABLES_LEN: usize = 11;
        const TABLE_CHANGE_SET_CHANGES_LEN: usize = 11;

        assert_eq!(
            table_change_set.new_tables.len(),
            TABLE_CHANGE_SET_NEW_TABLES_LEN
        );
        assert_eq!(table_change_set.changes.len(), TABLE_CHANGE_SET_CHANGES_LEN);
    }

    #[test]
    fn test_recursive_struct() {
        // This test intentionally modifies a module to have a cycle in a struct definition
        // then tries to deploy it. The MoveVM returns an error in this case.

        let genesis_config = GenesisConfig::default();

        // Load a real module
        let module_name = "signer_struct";
        let move_address = evm_address_to_move_address(&EVM_ADDRESS);
        let mut module_bytes = move_compile(module_name, &move_address).unwrap();
        let mut compiled_module = CompiledModule::deserialize(&module_bytes).unwrap();

        // Modify to include a recursive struct (it has one field which has type
        // equal to itself).
        let struct_name: Identifier = "RecursiveStruct".parse().unwrap();
        let struct_name_index = IdentifierIndex::new(compiled_module.identifiers.len() as u16);
        compiled_module.identifiers.push(struct_name);
        let struct_handle_index =
            StructHandleIndex::new(compiled_module.struct_handles.len() as u16);
        let struct_handle = StructHandle {
            module: ModuleHandleIndex::new(0),
            name: struct_name_index,
            abilities: AbilitySet::EMPTY,
            type_parameters: Vec::new(),
        };
        compiled_module.struct_handles.push(struct_handle);
        let struct_def = StructDefinition {
            struct_handle: struct_handle_index,
            field_information: StructFieldInformation::Declared(vec![FieldDefinition {
                name: struct_name_index,
                signature: TypeSignature(SignatureToken::Struct(struct_handle_index)),
            }]),
        };
        compiled_module.struct_defs.push(struct_def);
        *compiled_module
            .signatures
            .first_mut()
            .unwrap()
            .0
            .first_mut()
            .unwrap() = SignatureToken::Struct(struct_handle_index);

        // Re-serialize the new module
        module_bytes.clear();
        compiled_module.serialize(&mut module_bytes).unwrap();

        // Attempt to deploy the module, but get an error.
        let mut signer = Signer::new(&PRIVATE_KEY);
        // Deploy some other contract to ensure the state is properly initialized.
        let (_, state) = deploy_contract("natives", &mut signer, &genesis_config);
        let signed_tx = create_transaction(&mut signer, TxKind::Create, module_bytes);
        let outcome = execute_transaction(&signed_tx, state.resolver(), &genesis_config).unwrap();
        let err = outcome.vm_outcome.unwrap_err();
        assert!(format!("{err:?}").contains("RECURSIVE_STRUCT_DEFINITION"));
    }

    #[test]
    fn test_deeply_nested_type() {
        // This test intentionally modifies a module to include a type
        // which is very deeply nested (Option<Option<Option<...>>>).
        // Then the test tries to deploy the module and it fails due to
        // the Move recursion limit.

        let genesis_config = GenesisConfig::default();

        // Load a real module
        let module_name = "signer_struct";
        let move_address = evm_address_to_move_address(&EVM_ADDRESS);
        let mut module_bytes = move_compile(module_name, &move_address).unwrap();
        let mut compiled_module = CompiledModule::deserialize(&module_bytes).unwrap();

        // Define a procedure which wraps the argument to the function `main` in an
        // additional `Option`, e.g. `Option<signer>` -> `Option<Option<Signer>>`.
        fn wrap_with_option(compiled_module: &mut CompiledModule, module_bytes: &mut Vec<u8>) {
            let signature = compiled_module.signatures.first_mut().unwrap();
            let inner = signature.0.clone();
            signature.0 = vec![SignatureToken::StructInstantiation(
                StructHandleIndex(0),
                inner,
            )];

            // Re-serialize the new module
            module_bytes.clear();
            compiled_module.serialize(module_bytes).unwrap();
        }

        // This function does the same thing as `wrap_with_option` except it
        // acts directly on the module bytes instead of on the `CompiledModule`
        // data type. This allows us to continue wrapping with Option even once
        // the module serialization would fail due to the recursion limit.
        fn byte_level_wrap_with_option(module_bytes: &[u8]) -> Vec<u8> {
            // Helper function for this procedure
            fn update_byte(x: u8) -> (u8, u8) {
                let (y, overflow) = x.overflowing_add(3);
                if overflow {
                    (y + 128, 1)
                } else {
                    (y, 0)
                }
            }

            let mut result = Vec::with_capacity(module_bytes.len() + 3);

            // Copy first 20 bytes
            for b in &module_bytes[0..20] {
                result.push(*b);
            }

            // Update next 2 bytes
            let (x, y) = update_byte(module_bytes[20]);
            result.push(x);
            result.push(module_bytes[21] + y);

            // Copy next byte
            result.push(module_bytes[22]);

            // Update next 2 bytes
            let (x, y) = update_byte(module_bytes[23]);
            result.push(x);
            result.push(module_bytes[24] + y);

            // Copy next 2 bytes
            result.push(module_bytes[25]);
            result.push(module_bytes[26]);

            // Update next 2 bytes
            let (x, y) = update_byte(module_bytes[27]);
            result.push(x);
            result.push(module_bytes[28] + y);

            // Copy next 2 bytes
            result.push(module_bytes[29]);
            result.push(module_bytes[30]);

            // Update next 2 bytes
            let (x, y) = update_byte(module_bytes[31]);
            result.push(x);
            result.push(module_bytes[32] + y);

            // Copy next 16 bytes
            for b in &module_bytes[33..49] {
                result.push(*b);
            }

            // Push 3 new bytes
            result.push(1);
            result.push(11);
            result.push(0);

            // Copy remaining bytes
            for b in &module_bytes[49..] {
                result.push(*b);
            }

            result
        }

        // Run the `wrap_with_option` procedure many times to make a deep nesting
        // of `Option<Option<Option<...>>>`.
        for _ in 0..41 {
            wrap_with_option(&mut compiled_module, &mut module_bytes);
        }

        let mut computed_module_bytes = module_bytes.clone();

        // Continue wrapping up to the recursion limit.
        // Also now also act on a separate copy of the module bytes directly
        // and validate the changes are identical. We couldn't use the byte-level
        // procedure on iterations 0 to 40 because the byte sequence is a little
        // different for some reason.
        for _ in 41..254 {
            wrap_with_option(&mut compiled_module, &mut module_bytes);

            computed_module_bytes = byte_level_wrap_with_option(&computed_module_bytes);

            assert_eq!(computed_module_bytes, module_bytes);
        }

        // Do one extra iteration beyond the serialization recursion limit
        module_bytes = byte_level_wrap_with_option(&computed_module_bytes);

        // Try to deploy the module
        let mut signer = Signer::new(&PRIVATE_KEY);
        // Deploy some other contract to ensure the state is properly initialized.
        let (_, state) = deploy_contract("natives", &mut signer, &genesis_config);
        let signed_tx = create_transaction(&mut signer, TxKind::Create, module_bytes);
        let outcome = execute_transaction(&signed_tx, state.resolver(), &genesis_config).unwrap();
        // The deployment fails because the Aptos code refuses to deserialize
        // the module with too deep recursion.
        let err = outcome.vm_outcome.unwrap_err();
        assert!(
            format!("{err:?}").contains("Maximum recursion depth reached"),
            "Actual error: {err:?}"
        );
    }

    fn deploy_contract(
        module_name: &str,
        signer: &mut Signer,
        genesis_config: &GenesisConfig,
    ) -> (ModuleId, InMemoryState) {
        let mut state = InMemoryState::new();
        init_storage(genesis_config, &mut state);

        let move_address = evm_address_to_move_address(&EVM_ADDRESS);

        let module_bytes = move_compile(module_name, &move_address).unwrap();
        let signed_tx = create_transaction(signer, TxKind::Create, module_bytes);

        let outcome = execute_transaction(&signed_tx, state.resolver(), genesis_config).unwrap();
        state.apply(outcome.changes).unwrap();

        // Code was deployed
        let module_id = ModuleId::new(move_address, Identifier::new(module_name).unwrap());
        assert!(
            state.resolver().get_module(&module_id).unwrap().is_some(),
            "Code should be deployed"
        );
        (module_id, state)
    }

    fn create_transaction(signer: &mut Signer, to: TxKind, input: Vec<u8>) -> ExtendedTxEnvelope {
        let mut tx = TxEip1559 {
            chain_id: CHAIN_ID,
            nonce: signer.nonce,
            gas_limit: u64::MAX.into(),
            max_fee_per_gas: 0,
            max_priority_fee_per_gas: 0,
            to,
            value: Default::default(),
            access_list: Default::default(),
            input: input.into(),
        };
        signer.nonce += 1;
        let signature = signer.inner.sign_transaction_sync(&mut tx).unwrap();
        ExtendedTxEnvelope::Canonical(TxEnvelope::Eip1559(tx.into_signed(signature)))
    }

    fn move_compile(package_name: &str, address: &AccountAddress) -> anyhow::Result<Vec<u8>> {
        let known_attributes = BTreeSet::new();
        let named_address_mapping: std::collections::BTreeMap<_, _> = [(
            package_name.to_string(),
            NumericalAddress::new(address.into(), NumberFormat::Hex),
        )]
        .into_iter()
        .chain(aptos_framework::named_addresses().clone())
        .collect();

        let base_dir = format!("src/tests/res/{package_name}").replace('_', "-");
        let compiler = Compiler::from_files(
            vec![format!("{base_dir}/sources/{package_name}.move")],
            // Project needs access to the framework source files to compile
            aptos_framework::testnet_release_bundle()
                .files()
                .context(format!("Failed to compile {package_name}.move"))?,
            named_address_mapping,
            Flags::empty(),
            &known_attributes,
        );
        let (_, result) = compiler
            .build()
            .context(format!("Failed to compile {package_name}.move"))?;
        let compiled_unit = result.unwrap().0.pop().unwrap().into_compiled_unit();
        let bytes = compiled_unit.serialize(None);
        Ok(bytes)
    }
}
