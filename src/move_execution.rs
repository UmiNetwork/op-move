use {
    crate::types::transactions::ExtendedTxEnvelope,
    alloy_consensus::TxEnvelope,
    alloy_primitives::TxKind,
    aptos_gas_schedule::{MiscGasParameters, NativeGasParameters, LATEST_GAS_FEATURE_VERSION},
    aptos_types::on_chain_config::{Features, TimedFeaturesBuilder},
    aptos_types::transaction::{EntryFunction, Module},
    aptos_vm::natives::aptos_natives,
    move_core_types::{
        account_address::AccountAddress,
        effects::ChangeSet,
        value::{MoveTypeLayout, MoveValue},
    },
    move_vm_runtime::{
        module_traversal::{TraversalContext, TraversalStorage},
        move_vm::MoveVM,
    },
    move_vm_test_utils::{gas_schedule::GasStatus, InMemoryStorage},
    move_vm_types::{loaded_data::runtime_types::Type, values::Value},
};

// TODO: status return type
// TODO: more careful error type
pub fn execute_transaction(
    tx: &ExtendedTxEnvelope,
    state: &InMemoryStorage,
) -> anyhow::Result<ChangeSet> {
    match tx {
        ExtendedTxEnvelope::DepositedTx(_) => {
            // TODO: handle DepositedTx case
            Ok(ChangeSet::new())
        }
        ExtendedTxEnvelope::Canonical(tx) => {
            // TODO: check tx chain_id
            let sender = tx.recover_signer()?;
            let sender_move_address = evm_address_to_move_address(&sender);
            // TODO: check tx nonce
            let (to, payload) = match tx {
                TxEnvelope::Eip1559(tx) => (tx.tx().to, &tx.tx().input),
                TxEnvelope::Eip2930(tx) => (tx.tx().to, &tx.tx().input),
                TxEnvelope::Legacy(tx) => (tx.tx().to, &tx.tx().input),
                TxEnvelope::Eip4844(_) => anyhow::bail!("Blob transactions not supported"),
                _ => anyhow::bail!("Unknown transaction type"),
            };
            // TODO: use other tx fields (value, gas limit, etc).
            // TODO: How to model script-type transactions?
            let changes = match to {
                TxKind::Call(_to) => {
                    let entry_fn: EntryFunction = bcs::from_bytes(payload)?;
                    if entry_fn.module().address() != &sender_move_address {
                        anyhow::bail!("tx.to must match payload module address");
                    }
                    execute_entry_function(entry_fn, &sender_move_address, state)?
                }
                TxKind::Create => {
                    // Assume EVM create type transactions are module deployments in Move
                    let module = Module::new(payload.to_vec());
                    deploy_module(module, evm_address_to_move_address(&sender), state)?
                }
            };
            Ok(changes)
        }
    }
}

// TODO: more careful error type
fn execute_entry_function(
    entry_fn: EntryFunction,
    signer: &AccountAddress,
    state: &InMemoryStorage,
) -> anyhow::Result<ChangeSet> {
    let move_vm = create_move_vm()?;
    let mut session = move_vm.new_session(state);
    // TODO: gas metering
    let mut gas_meter = GasStatus::new_unmetered();
    let traversal_storage = TraversalStorage::new();
    let mut traversal_context = TraversalContext::new(&traversal_storage);

    let (module, function_name, ty_args, args) = entry_fn.into_inner();

    // Validate signer params match the actual signer
    let function = session.load_function(&module, &function_name, &ty_args)?;
    if function.param_tys.len() != args.len() {
        anyhow::bail!("Incorrect number of arguments");
    }
    for (ty, bytes) in function.param_tys.iter().zip(&args) {
        let ty = strip_reference(ty)?;
        // TODO: need to also handle structs that contain signer types
        if let Type::Signer = ty {
            let arg = Value::simple_deserialize(bytes, &MoveTypeLayout::Signer)
                .ok_or_else(|| anyhow::Error::msg("Wrong param type; expected signer"))?
                .as_move_value(&MoveTypeLayout::Signer);
            if let MoveValue::Signer(given_signer) = arg {
                if &given_signer != signer {
                    anyhow::bail!("Signer does not match transaction signature");
                }
            } else {
                anyhow::bail!("Wrong param type; expected signer");
            }
        }
    }

    // TODO: is this the right way to be using the VM?
    // Maybe there is some higher level entry point we should be using instead?
    session.execute_entry_function(
        &module,
        &function_name,
        ty_args,
        args,
        &mut gas_meter,
        &mut traversal_context,
    )?;
    let changes = session.finish()?;

    Ok(changes)
}

// If `t` is wrapped in `Type::Reference` or `Type::MutableReference`,
// return the inner type
fn strip_reference(t: &Type) -> anyhow::Result<&Type> {
    match t {
        Type::Reference(inner) | Type::MutableReference(inner) => {
            match inner.as_ref() {
                Type::Reference(_) | Type::MutableReference(_) => {
                    // Based on Aptos code, it looks like references are not allowed to be nested.
                    // TODO: check this assumption.
                    anyhow::bail!("Invalid nested references");
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
    state: &InMemoryStorage,
) -> anyhow::Result<ChangeSet> {
    let move_vm = create_move_vm()?;
    let mut session = move_vm.new_session(state);
    let mut gas_meter = GasStatus::new_unmetered();

    session.publish_module(code.into_inner(), address, &mut gas_meter)?;
    let change_set = session.finish()?;
    Ok(change_set)
}

fn create_move_vm() -> anyhow::Result<MoveVM> {
    // TODO: error handling
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
        crate::state_actor::head_release_bundle,
        alloy::{network::TxSignerSync, signers::local::PrivateKeySigner},
        alloy_consensus::{transaction::TxEip1559, SignableTransaction},
        anyhow::Context,
        move_compiler::{
            shared::{NumberFormat, NumericalAddress},
            Compiler, Flags,
        },
        move_core_types::{
            identifier::Identifier,
            language_storage::{ModuleId, StructTag},
            resolver::{ModuleResolver, MoveResolver},
        },
        std::collections::BTreeSet,
    };

    #[test]
    fn test_execute_transaction() {
        let mut state = InMemoryStorage::new();
        // TODO: Also inject the created resource and table data
        for (bytes, module) in head_release_bundle().code_and_compiled_modules() {
            state.publish_or_overwrite_module(module.self_id(), bytes.to_vec());
        }

        // The address corresponding to this private key is 0x8fd379246834eac74B8419FfdA202CF8051F7A03
        let sk = [0xaa; 32].into();
        let evm_address = alloy_primitives::address!("8fd379246834eac74b8419ffda202cf8051f7a03");
        let move_address = evm_address_to_move_address(&evm_address);
        let signer = PrivateKeySigner::from_bytes(&sk).unwrap();
        let module_name = "counter";

        let module_bytes = move_compile(module_name, &move_address).unwrap();
        let signed_tx = create_transaction(&signer, TxKind::Create, module_bytes);

        let changes = execute_transaction(&signed_tx, &state).unwrap();
        state.apply(changes).unwrap();

        // Code was deployed
        let module_id = ModuleId::new(move_address, Identifier::new(module_name).unwrap());
        assert!(
            state.get_module(&module_id).unwrap().is_some(),
            "Code should be deployed"
        );

        // Call entry function to create the `Counter` resource
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
            &signer,
            TxKind::Call(evm_address),
            bcs::to_bytes(&entry_fn).unwrap(),
        );

        let changes = execute_transaction(&signed_tx, &state).unwrap();
        state.apply(changes).unwrap();

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
            &signer,
            TxKind::Call(evm_address),
            bcs::to_bytes(&entry_fn).unwrap(),
        );
        let err = execute_transaction(&signed_tx, &state).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Signer does not match transaction signature"
        );

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
            &signer,
            TxKind::Call(evm_address),
            bcs::to_bytes(&entry_fn).unwrap(),
        );

        let changes = execute_transaction(&signed_tx, &state).unwrap();
        state.apply(changes).unwrap();

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

    fn create_transaction(
        signer: &PrivateKeySigner,
        to: TxKind,
        input: Vec<u8>,
    ) -> ExtendedTxEnvelope {
        let mut tx = TxEip1559 {
            chain_id: 0,
            nonce: 0,
            gas_limit: 0,
            max_fee_per_gas: 0,
            max_priority_fee_per_gas: 0,
            to,
            value: Default::default(),
            access_list: Default::default(),
            input: input.into(),
        };
        let signature = signer.sign_transaction_sync(&mut tx).unwrap();
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

        let base_dir = format!("src/tests/res/{package_name}");
        let compiler = Compiler::from_files(
            vec![format!("{base_dir}/sources/{package_name}.move")],
            // Project is compiled with the move tool to have these dependencies available
            vec![
                format!("{base_dir}/build/{package_name}/sources/dependencies/AptosFramework"),
                format!("{base_dir}/build/{package_name}/sources/dependencies/AptosStdlib"),
                format!("{base_dir}/build/{package_name}/sources/dependencies/MoveStdlib"),
            ],
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
