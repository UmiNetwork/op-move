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

#[cfg(test)]
use {
    alloy::network::TxSignerSync,
    alloy::signers::local::PrivateKeySigner,
    alloy_consensus::{transaction::TxEip1559, SignableTransaction},
    move_core_types::{
        identifier::Identifier, language_storage::ModuleId, resolver::ModuleResolver,
    },
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

#[test]
fn test_execute_transaction() {
    let mut state = InMemoryStorage::new();

    // The address corresponding to this private key is 0x8fd379246834eac74B8419FfdA202CF8051F7A03
    let sk = [0xaa; 32].into();
    let address = evm_address_to_move_address(&alloy_primitives::address!(
        "8fd379246834eac74b8419ffda202cf8051f7a03"
    ));
    let signer = PrivateKeySigner::from_bytes(&sk).unwrap();

    let code_hex = "a11ceb0b06000000060100020302050507010708090811200c3107000000010000000003616464046d61696e0000000000000000000000008fd379246834eac74b8419ffda202cf8051f7a030001040000010200";
    let module_bytes = hex::decode(code_hex).unwrap();
    let mut tx = TxEip1559 {
        chain_id: 0,
        nonce: 0,
        gas_limit: 0,
        max_fee_per_gas: 0,
        max_priority_fee_per_gas: 0,
        to: TxKind::Create,
        value: Default::default(),
        access_list: Default::default(),
        input: module_bytes.into(),
    };
    let signature = signer.sign_transaction_sync(&mut tx).unwrap();
    let signed_tx = ExtendedTxEnvelope::Canonical(TxEnvelope::Eip1559(tx.into_signed(signature)));

    let changes = execute_transaction(&signed_tx, &state).unwrap();
    state.apply(changes).unwrap();

    // Code was deployed
    let module_id = ModuleId::new(address, Identifier::new("add").unwrap());
    assert!(
        state.get_module(&module_id).unwrap().is_some(),
        "Code should be deployed"
    );

    // TODO: test calling entry function
}
