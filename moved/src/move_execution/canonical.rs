use {
    crate::{
        genesis::config::GenesisConfig,
        move_execution::{
            create_move_vm, create_vm_session,
            execute::{deploy_module, execute_entry_function},
            gas::{new_gas_meter, total_gas_used},
            nonces::check_nonce,
            LogsBloom,
        },
        primitives::ToMoveAddress,
        types::transactions::{NormalizedEthTransaction, TransactionExecutionOutcome},
        Error::{InvalidTransaction, User},
        InvalidTransactionCause,
    },
    alloy_consensus::{Transaction, TxEnvelope},
    alloy_primitives::TxKind,
    aptos_gas_meter::AptosGasMeter,
    aptos_table_natives::TableResolver,
    aptos_types::transaction::{EntryFunction, Module},
    move_binary_format::errors::PartialVMError,
    move_core_types::resolver::MoveResolver,
    move_vm_runtime::module_traversal::{TraversalContext, TraversalStorage},
};

pub(super) fn execute_canonical_transaction(
    tx: &TxEnvelope,
    state: &(impl MoveResolver<PartialVMError> + TableResolver),
    genesis_config: &GenesisConfig,
) -> crate::Result<TransactionExecutionOutcome> {
    if let Some(chain_id) = tx.chain_id() {
        if chain_id != genesis_config.chain_id {
            return Err(InvalidTransactionCause::IncorrectChainId.into());
        }
    }

    let tx = NormalizedEthTransaction::try_from(tx.clone())?;
    let sender_move_address = tx.signer.to_move_address();

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
                tx.signer.to_move_address(),
                &mut session,
                &mut gas_meter,
            )
        }
    };

    let (changes, mut extensions) = session.finish_with_extensions()?;
    let logs_bloom = extensions.logs_bloom();
    let gas_used = total_gas_used(&gas_meter, genesis_config);

    match vm_outcome {
        Ok(_) => Ok(TransactionExecutionOutcome::new(
            Ok(()),
            changes,
            gas_used,
            logs_bloom,
        )),
        // User error still generates a receipt and consumes gas
        Err(User(e)) => Ok(TransactionExecutionOutcome::new(
            Err(e),
            changes,
            gas_used,
            logs_bloom,
        )),
        Err(e) => Err(e),
    }
}
