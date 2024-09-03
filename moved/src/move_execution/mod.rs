use {
    crate::{
        genesis::config::GenesisConfig,
        types::{
            session_id::SessionId,
            transactions::{ExtendedTxEnvelope, ToLog, TransactionExecutionOutcome},
        },
    },
    alloy_primitives::Bloom,
    aptos_framework::natives::{
        event::NativeEventContext, object::NativeObjectContext,
        transaction_context::NativeTransactionContext,
    },
    aptos_gas_schedule::{MiscGasParameters, NativeGasParameters, LATEST_GAS_FEATURE_VERSION},
    aptos_table_natives::{NativeTableContext, TableResolver},
    aptos_types::on_chain_config::{Features, TimedFeaturesBuilder},
    aptos_vm::natives::aptos_natives,
    canonical::execute_canonical_transaction,
    deposited::execute_deposited_transaction,
    ethers_core::types::H256,
    move_binary_format::errors::PartialVMError,
    move_core_types::resolver::MoveResolver,
    move_vm_runtime::{
        move_vm::MoveVM, native_extensions::NativeContextExtensions, session::Session,
    },
};

mod canonical;
mod deposited;
mod eth_token;
mod execute;
mod gas;
mod nonces;
mod tag_validation;

#[cfg(test)]
mod tests;

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

pub fn create_vm_session<'l, 'r, S>(
    vm: &'l MoveVM,
    state: &'r S,
    session_id: SessionId,
) -> Session<'r, 'l>
where
    S: MoveResolver<PartialVMError> + TableResolver,
{
    let txn_hash = session_id.txn_hash;
    let mut native_extensions = NativeContextExtensions::default();

    // Events are used in `eth_token` because it depends on `fungible_asset`.
    native_extensions.add(NativeEventContext::default());

    // Objects are part of the standard library
    native_extensions.add(NativeObjectContext::default());

    // Objects require transaction_context to work
    native_extensions.add(NativeTransactionContext::new(
        txn_hash.to_vec(),
        session_id
            .script_hash
            .map(|h| h.to_vec())
            .unwrap_or_default(),
        session_id.chain_id,
        session_id.user_txn_context,
    ));

    // Tables can be used
    native_extensions.add(NativeTableContext::new(txn_hash, state));

    vm.new_session_with_extensions(state, native_extensions)
}

pub fn execute_transaction(
    tx: &ExtendedTxEnvelope,
    tx_hash: &H256,
    state: &(impl MoveResolver<PartialVMError> + TableResolver),
    genesis_config: &GenesisConfig,
) -> crate::Result<TransactionExecutionOutcome> {
    match tx {
        ExtendedTxEnvelope::DepositedTx(tx) => {
            execute_deposited_transaction(tx, tx_hash, state, genesis_config)
        }
        ExtendedTxEnvelope::Canonical(tx) => {
            execute_canonical_transaction(tx, tx_hash, state, genesis_config)
        }
    }
}

trait LogsBloom {
    fn logs_bloom(&mut self) -> Bloom;
}

impl LogsBloom for NativeContextExtensions<'_> {
    fn logs_bloom(&mut self) -> Bloom {
        self.remove::<NativeEventContext>()
            .into_events()
            .into_iter()
            .map(|(event, ..)| event.to_log())
            .fold(Bloom::ZERO, |mut bloom, log| {
                bloom.accrue_log(&log);
                bloom
            })
    }
}
