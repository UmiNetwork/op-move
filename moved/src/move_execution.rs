mod signers;

use {
    crate::{
        move_execution::signers::check_signer,
        types::transactions::{ExtendedTxEnvelope, TransactionExecutionOutcome},
        InvalidTransactionCause,
    },
    alloy_consensus::TxEnvelope,
    alloy_primitives::TxKind,
    aptos_framework::natives::event::NativeEventContext,
    aptos_gas_schedule::{MiscGasParameters, NativeGasParameters, LATEST_GAS_FEATURE_VERSION},
    aptos_table_natives::{NativeTableContext, TableResolver},
    aptos_types::{
        on_chain_config::{Features, TimedFeaturesBuilder},
        transaction::{EntryFunction, Module},
    },
    aptos_vm::natives::aptos_natives,
    move_binary_format::errors::PartialVMError,
    move_core_types::{account_address::AccountAddress, resolver::MoveResolver, value::MoveValue},
    move_vm_runtime::{
        module_traversal::{TraversalContext, TraversalStorage},
        move_vm::MoveVM,
        native_extensions::NativeContextExtensions,
        session::Session,
    },
    move_vm_test_utils::gas_schedule::GasStatus,
    move_vm_types::{gas::UnmeteredGasMeter, loaded_data::runtime_types::Type, values::Value},
    nonces::check_nonce,
};

mod eth_token;
mod nonces;

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

            eth_token::mint_eth(&to, amount, &mut session, &mut traversal_context)?;

            debug_assert!(
                eth_token::get_eth_balance(&to, &mut session, &mut traversal_context).unwrap()
                    >= amount,
                "tokens were minted"
            );

            let changes = session.finish()?;
            Ok(TransactionExecutionOutcome::new(Ok(()), changes))
        }
        ExtendedTxEnvelope::Canonical(tx) => {
            // TODO: check tx chain_id
            let sender = tx.recover_signer()?;
            let sender_move_address = evm_address_to_move_address(&sender);
            // TODO: use other tx fields (value, gas limit, etc).
            let (to, nonce, payload) = match tx {
                TxEnvelope::Eip1559(tx) => (tx.tx().to, tx.tx().nonce, &tx.tx().input),
                TxEnvelope::Eip2930(tx) => (tx.tx().to, tx.tx().nonce, &tx.tx().input),
                TxEnvelope::Legacy(tx) => (tx.tx().to, tx.tx().nonce, &tx.tx().input),
                TxEnvelope::Eip4844(_) => Err(InvalidTransactionCause::UnsupportedType)?,
                t => Err(InvalidTransactionCause::UnknownType(t.tx_type()))?,
            };

            let move_vm = create_move_vm()?;
            let mut session = create_vm_session(&move_vm, state);
            let traversal_storage = TraversalStorage::new();
            let mut traversal_context = TraversalContext::new(&traversal_storage);

            check_nonce(
                nonce,
                &sender_move_address,
                &mut session,
                &mut traversal_context,
            )?;

            // TODO: How to model script-type transactions?
            let vm_outcome = match to {
                TxKind::Call(_to) => {
                    let entry_fn: EntryFunction = bcs::from_bytes(payload)?;
                    if entry_fn.module().address() != &sender_move_address {
                        Err(InvalidTransactionCause::InvalidDestination)?
                    }
                    execute_entry_function(
                        entry_fn,
                        &sender_move_address,
                        &mut session,
                        &mut traversal_context,
                    )
                }
                TxKind::Create => {
                    // Assume EVM create type transactions are module deployments in Move
                    let module = Module::new(payload.to_vec());
                    deploy_module(module, evm_address_to_move_address(&sender), &mut session)
                }
            };
            let changes = session.finish()?;
            Ok(TransactionExecutionOutcome::new(vm_outcome, changes))
        }
    }
}

fn execute_entry_function(
    entry_fn: EntryFunction,
    signer: &AccountAddress,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
) -> crate::Result<()> {
    // TODO: gas metering
    let mut gas_meter = GasStatus::new_unmetered();

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
        let tag = session.get_type_tag(ty)?;
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
        &mut gas_meter,
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

fn deploy_module(
    code: Module,
    address: AccountAddress,
    session: &mut Session,
) -> crate::Result<()> {
    session.publish_module(code.into_inner(), address, &mut UnmeteredGasMeter)?;

    Ok(())
}

// TODO: is there a way to make Move use 32-byte addresses?
fn evm_address_to_move_address(address: &alloy_primitives::Address) -> AccountAddress {
    let mut bytes = [0; 32];
    bytes[12..32].copy_from_slice(address.as_slice());
    AccountAddress::new(bytes)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            genesis::init_storage,
            tests::{signer::Signer, EVM_ADDRESS, PRIVATE_KEY},
            types::transactions::DepositedTx,
        },
        alloy::network::TxSignerSync,
        alloy_consensus::{transaction::TxEip1559, SignableTransaction},
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
            resolver::{ModuleResolver, MoveResolver},
            value::MoveStruct,
        },
        move_vm_test_utils::InMemoryStorage,
        std::collections::BTreeSet,
    };

    #[test]
    fn test_execute_counter_contract() {
        let module_name = "counter";
        let mut signer = Signer::new(&PRIVATE_KEY);
        let (module_id, mut state) = deploy_contract(module_name, &mut signer);

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

        let outcome = execute_transaction(&signed_tx, &state).unwrap();
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
        let outcome = execute_transaction(&signed_tx, &state).unwrap();
        let err = outcome.vm_outcome.unwrap_err();
        assert_eq!(
            err.to_string(),
            "Signer does not match transaction signature"
        );
        state.apply(outcome.changes).unwrap(); // Still increment the nonce

        // Resource was created
        let struct_tag = StructTag {
            address: move_address,
            module: Identifier::new(module_name).unwrap(),
            name: Identifier::new("Counter").unwrap(),
            type_args: Vec::new(),
        };
        let resource: u64 = bcs::from_bytes(
            &state
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

        let outcome = execute_transaction(&signed_tx, &state).unwrap();
        state.apply(outcome.changes).unwrap();

        // Resource was modified
        let resource: u64 = bcs::from_bytes(
            &state
                .get_resource(&move_address, &struct_tag)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(resource, initial_value + 1);
    }

    #[test]
    fn test_execute_signer_struct_contract() {
        let module_name = "signer_struct";
        let mut signer = Signer::new(&PRIVATE_KEY);
        let (module_id, mut storage) = deploy_contract(module_name, &mut signer);

        // Call main function with correct signer
        let move_address = evm_address_to_move_address(&EVM_ADDRESS);
        let input_arg = MoveValue::Struct(MoveStruct::new(vec![MoveValue::Signer(move_address)]));
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

        let outcome = execute_transaction(&signed_tx, &storage).unwrap();
        assert!(outcome.vm_outcome.is_ok());
        storage.apply(outcome.changes).unwrap();

        // Call main function with incorrect signer (get an error)
        let input_arg = MoveValue::Struct(MoveStruct::new(vec![MoveValue::Signer(
            AccountAddress::new([0x11; 32]),
        )]));
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

        let outcome = execute_transaction(&signed_tx, &storage).unwrap();
        let err = outcome.vm_outcome.unwrap_err();
        assert_eq!(
            err.to_string(),
            "Signer does not match transaction signature"
        );
    }

    #[test]
    fn test_execute_natives_contract() {
        let mut signer = Signer::new(&PRIVATE_KEY);
        let (module_id, state) = deploy_contract("natives", &mut signer);

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

        let changes = execute_transaction(&signed_tx, &state);
        assert!(changes.is_ok());
    }

    /// Deposits can be made to the L2.
    #[test]
    fn test_deposit_tx() {
        let mut signer = Signer::new(&PRIVATE_KEY);
        let (_, state) = deploy_contract("natives", &mut signer);

        let mint_amount = U256::from(123u64);
        let tx = ExtendedTxEnvelope::DepositedTx(DepositedTx {
            to: EVM_ADDRESS,
            value: mint_amount,
            source_hash: FixedBytes::default(),
            from: EVM_ADDRESS,
            mint: U256::ZERO,
            gas: U64::ZERO,
            is_system_tx: false,
            data: Vec::new().into(),
        });

        execute_transaction(&tx, &state)
            .unwrap()
            .vm_outcome
            .unwrap();
    }

    #[test]
    fn test_transaction_replay_is_forbidden() {
        // Transaction replay is forbidden by the nonce checking.

        // Deploy a contract
        let mut signer = Signer::new(&PRIVATE_KEY);
        let (module_id, mut storage) = deploy_contract("natives", &mut signer);

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

        let outcome = execute_transaction(&signed_tx, &storage).unwrap();
        storage.apply(outcome.changes).unwrap();

        // Send the same transaction again; this fails with a nonce error
        let err = execute_transaction(&signed_tx, &storage).unwrap_err();
        assert_eq!(err.to_string(), "Incorrect nonce: given=1 expected=2");
    }

    #[test]
    fn test_execute_tables_contract() {
        let module_name = "tables";
        let mut signer = Signer::new(&PRIVATE_KEY);
        let (module_id, storage) = deploy_contract(module_name, &mut signer);
        let vm = create_move_vm().unwrap();
        let traversal_storage = TraversalStorage::new();

        let mut session = create_vm_session(&vm, &storage);
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
        let (_, storage) = deploy_contract("natives", &mut signer);
        let signed_tx = create_transaction(&mut signer, TxKind::Create, module_bytes);
        let outcome = execute_transaction(&signed_tx, &storage).unwrap();
        let err = outcome.vm_outcome.unwrap_err();
        assert!(format!("{err:?}").contains("RECURSIVE_STRUCT_DEFINITION"));
    }

    #[test]
    fn test_deeply_nested_type() {
        // This test intentionally modifies a module to include a type
        // which is very deeply nested (it is a struct which contains a field that
        // is a struct, which itself contains a different struct, and so on).
        // Then the test tries to run a function with this deeply nested type
        // as an input and the VM returns an error.

        // Load a real module
        let module_name = "signer_struct";
        let move_address = evm_address_to_move_address(&EVM_ADDRESS);
        let mut module_bytes = move_compile(module_name, &move_address).unwrap();
        let mut compiled_module = CompiledModule::deserialize(&module_bytes).unwrap();

        // Define a procedure which includes a new struct which uses the previous
        // struct as its field
        let mut depth: u16 = 1;
        let mut define_new_struct = || {
            let struct_name: Identifier = format!("DeepStruct{depth}").parse().unwrap();
            let struct_name_index = IdentifierIndex::new(compiled_module.identifiers.len() as u16);
            compiled_module.identifiers.push(struct_name);
            let previous_struct_handle_index = StructHandleIndex::new(depth - 1);
            let current_struct_handle_index = StructHandleIndex::new(depth);
            let struct_handle = StructHandle {
                module: ModuleHandleIndex::new(0),
                name: struct_name_index,
                abilities: AbilitySet::FUNCTIONS,
                type_parameters: Vec::new(),
            };
            compiled_module.struct_handles.push(struct_handle);
            let struct_def = StructDefinition {
                struct_handle: current_struct_handle_index,
                field_information: StructFieldInformation::Declared(vec![FieldDefinition {
                    name: struct_name_index,
                    signature: TypeSignature(SignatureToken::Struct(previous_struct_handle_index)),
                }]),
            };
            compiled_module.struct_defs.push(struct_def);
            *compiled_module
                .signatures
                .first_mut()
                .unwrap()
                .0
                .first_mut()
                .unwrap() = SignatureToken::Struct(current_struct_handle_index);
            depth += 1;
        };

        // Run this procedure many times
        for _ in 0..200 {
            define_new_struct();
        }

        // Re-serialize the new module
        module_bytes.clear();
        compiled_module.serialize(&mut module_bytes).unwrap();

        // Deploy the module.
        let mut signer = Signer::new(&PRIVATE_KEY);
        // Deploy some other contract to ensure the state is properly initialized.
        let (_, mut storage) = deploy_contract("natives", &mut signer);
        let signed_tx = create_transaction(&mut signer, TxKind::Create, module_bytes);
        let outcome = execute_transaction(&signed_tx, &storage).unwrap();
        storage.apply(outcome.changes).unwrap();
        let module_id = ModuleId::new(move_address, Identifier::new(module_name).unwrap());

        // Call the main function
        let input_arg = MoveValue::Struct(MoveStruct::new(vec![MoveValue::Signer(move_address)]));
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

        let outcome = execute_transaction(&signed_tx, &storage).unwrap();
        let err = outcome.vm_outcome.unwrap_err();
        assert!(format!("{err:?}").contains("VM_MAX_VALUE_DEPTH_REACHED"));
    }

    fn deploy_contract(module_name: &str, signer: &mut Signer) -> (ModuleId, InMemoryStorage) {
        let mut state = InMemoryStorage::new();
        init_storage(&mut state);

        let move_address = evm_address_to_move_address(&EVM_ADDRESS);

        let module_bytes = move_compile(module_name, &move_address).unwrap();
        let signed_tx = create_transaction(signer, TxKind::Create, module_bytes);

        let outcome = execute_transaction(&signed_tx, &state).unwrap();
        state.apply(outcome.changes).unwrap();

        // Code was deployed
        let module_id = ModuleId::new(move_address, Identifier::new(module_name).unwrap());
        assert!(
            state.get_module(&module_id).unwrap().is_some(),
            "Code should be deployed"
        );
        (module_id, state)
    }

    fn create_transaction(signer: &mut Signer, to: TxKind, input: Vec<u8>) -> ExtendedTxEnvelope {
        let mut tx = TxEip1559 {
            chain_id: 0,
            nonce: signer.nonce,
            gas_limit: 0,
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
