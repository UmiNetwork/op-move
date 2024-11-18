use {
    crate::{
        block::HeaderForExecution,
        genesis::config::GenesisConfig,
        move_execution::{
            create_move_vm, create_vm_session, eth_token,
            gas::{new_gas_meter, total_gas_used},
            Logs,
        },
        primitives::{ToMoveAddress, B256},
        types::{
            session_id::SessionId,
            transactions::{DepositedTx, TransactionExecutionOutcome},
        },
    },
    aptos_table_natives::TableResolver,
    move_binary_format::errors::PartialVMError,
    move_core_types::resolver::MoveResolver,
    move_vm_runtime::module_traversal::{TraversalContext, TraversalStorage},
};

pub(super) fn execute_deposited_transaction(
    tx: &DepositedTx,
    tx_hash: &B256,
    state: &(impl MoveResolver<PartialVMError> + TableResolver),
    genesis_config: &GenesisConfig,
    block_header: HeaderForExecution,
) -> crate::Result<TransactionExecutionOutcome> {
    let amount = tx.mint.saturating_add(tx.value);
    let to = tx.to.to_move_address();

    let move_vm = create_move_vm()?;
    let session_id = SessionId::new_from_deposited(tx, tx_hash, genesis_config, block_header);
    let mut session = create_vm_session(&move_vm, state, session_id);
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
        eth_token::get_eth_balance(&to, &mut session, &mut traversal_context, &mut gas_meter)?
            >= amount,
        "tokens were minted"
    );

    let (changes, mut extensions) = session.finish_with_extensions()?;
    let gas_used = total_gas_used(&gas_meter, genesis_config);
    let logs = extensions.logs().collect();

    Ok(TransactionExecutionOutcome::new(
        Ok(()),
        changes,
        gas_used,
        logs,
    ))
}
