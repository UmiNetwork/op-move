use {
    crate::types::transactions::ExtendedTxEnvelope,
    alloy_consensus::TxEnvelope,
    alloy_primitives::TxKind,
    aptos_types::transaction::{EntryFunction, Module},
    move_core_types::{account_address::AccountAddress, effects::ChangeSet},
    move_vm_runtime::{
        module_traversal::{TraversalContext, TraversalStorage},
        move_vm::MoveVM,
    },
    move_vm_test_utils::{gas_schedule::GasStatus, InMemoryStorage},
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
                    // TODO: require `to` be somehow related to `entry_fn.module()`
                    // TODO: if there are Signer types in the function,
                    // make sure they correspond to the `sender` (for security).
                    execute_entry_function(entry_fn, state)?
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

fn execute_entry_function(
    entry_fn: EntryFunction,
    state: &InMemoryStorage,
) -> anyhow::Result<ChangeSet> {
    let move_vm = create_move_vm()?;
    let mut session = move_vm.new_session(state);
    // TODO: gas metering
    let mut gas_meter = GasStatus::new_unmetered();
    let traversal_storage = TraversalStorage::new();
    let mut traversal_context = TraversalContext::new(&traversal_storage);

    let (module, function_name, ty_args, args) = entry_fn.into_inner();

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
    // TODO: natives
    // TODO: error handling
    let vm = MoveVM::new(Vec::new())?;
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
            value::MoveValue,
        },
        std::collections::BTreeSet,
    };

    #[test]
    fn test_execute_transaction() {
        let mut state = InMemoryStorage::new();

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
        let named_address_mapping = [(
            package_name,
            NumericalAddress::new(address.into(), NumberFormat::Hex),
        )]
        .into_iter()
        .collect();
        let compiler = Compiler::from_files(
            vec![format!(
                "src/tests/move_sources/{package_name}/sources/{package_name}.move"
            )],
            Vec::new(),
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
