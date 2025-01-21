pub use {
    eth_token::{mint_eth, quick_get_eth_balance, BaseTokenAccounts, MovedBaseTokenAccounts},
    evm_native::genesis_state_changes,
    gas::{
        CreateEcotoneL1GasFee, CreateL1GasFee, CreateL2GasFee, CreateMovedL2GasFee, EcotoneGasFee,
        L1GasFee, L1GasFeeInput, L2GasFee, L2GasFeeInput, MovedGasFee,
    },
    nonces::{check_nonce, quick_get_nonce},
};

use {
    self::evm_native::events::{evm_logs_event_to_log, EVM_LOGS_EVENT_LAYOUT, EVM_LOGS_EVENT_TAG},
    crate::{
        block::HeaderForExecution,
        genesis::config::GenesisConfig,
        primitives::{ToEthAddress, B256},
        types::{
            session_id::SessionId,
            transactions::{DepositedTx, NormalizedEthTransaction, TransactionExecutionOutcome},
        },
    },
    alloy::primitives::{Bloom, Keccak256, Log, LogData},
    aptos_framework::natives::{
        event::NativeEventContext, object::NativeObjectContext,
        transaction_context::NativeTransactionContext,
    },
    aptos_gas_schedule::{MiscGasParameters, NativeGasParameters, LATEST_GAS_FEATURE_VERSION},
    aptos_native_interface::SafeNativeBuilder,
    aptos_table_natives::{NativeTableContext, TableResolver},
    aptos_types::{
        contract_event::ContractEvent,
        on_chain_config::{Features, TimedFeaturesBuilder},
    },
    aptos_vm::natives::aptos_natives_with_builder,
    canonical::execute_canonical_transaction,
    deposited::execute_deposited_transaction,
    move_binary_format::errors::PartialVMError,
    move_core_types::{
        language_storage::TypeTag,
        resolver::MoveResolver,
        value::{MoveTypeLayout, MoveValue},
    },
    move_vm_runtime::{
        move_vm::MoveVM, native_extensions::NativeContextExtensions, session::Session,
    },
    std::ops::Deref,
};

mod canonical;
mod deposited;
mod eth_token;
pub mod evm_native;
mod execute;
mod gas;
mod nonces;
pub(crate) mod simulate;
mod tag_validation;

#[cfg(test)]
mod tests;

const ADDRESS_LAYOUT: MoveTypeLayout = MoveTypeLayout::Address;
const U256_LAYOUT: MoveTypeLayout = MoveTypeLayout::U256;

pub fn create_move_vm() -> crate::Result<MoveVM> {
    let mut builder = SafeNativeBuilder::new(
        LATEST_GAS_FEATURE_VERSION,
        NativeGasParameters::zeros(),
        MiscGasParameters::zeros(),
        TimedFeaturesBuilder::enable_all().build(),
        Features::default(),
    );
    let mut natives = aptos_natives_with_builder(&mut builder);
    evm_native::append_evm_natives(&mut natives, &builder);
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

    // EVM native extension
    native_extensions.add(evm_native::NativeEVMContext::new(
        state,
        session_id.block_header,
    ));

    vm.new_session_with_extensions(state, native_extensions)
}

#[derive(Debug)]
pub enum TransactionExecutionInput<'input, S, F, B> {
    Deposit(DepositExecutionInput<'input, S>),
    Canonical(CanonicalExecutionInput<'input, S, F, B>),
}

#[derive(Debug)]
pub struct DepositExecutionInput<'input, S> {
    pub tx: &'input DepositedTx,
    pub tx_hash: &'input B256,
    pub state: &'input S,
    pub genesis_config: &'input GenesisConfig,
    pub block_header: HeaderForExecution,
}

impl<'input, S, F, B> From<DepositExecutionInput<'input, S>>
    for TransactionExecutionInput<'input, S, F, B>
{
    fn from(value: DepositExecutionInput<'input, S>) -> Self {
        Self::Deposit(value)
    }
}

#[derive(Debug)]
pub struct CanonicalExecutionInput<'input, S, F, B> {
    pub tx: &'input NormalizedEthTransaction,
    pub tx_hash: &'input B256,
    pub state: &'input S,
    pub genesis_config: &'input GenesisConfig,
    pub l1_cost: u64,
    pub l2_fee: F,
    pub l2_input: L2GasFeeInput,
    pub base_token: &'input B,
    pub block_header: HeaderForExecution,
}

impl<'input, S, F, B> From<CanonicalExecutionInput<'input, S, F, B>>
    for TransactionExecutionInput<'input, S, F, B>
{
    fn from(value: CanonicalExecutionInput<'input, S, F, B>) -> Self {
        Self::Canonical(value)
    }
}

pub fn execute_transaction<
    S: MoveResolver<PartialVMError> + TableResolver,
    F: L2GasFee,
    B: BaseTokenAccounts,
>(
    input: TransactionExecutionInput<S, F, B>,
) -> crate::Result<TransactionExecutionOutcome> {
    match input {
        TransactionExecutionInput::Deposit(input) => execute_deposited_transaction(input),
        TransactionExecutionInput::Canonical(input) => execute_canonical_transaction(input),
    }
}

pub trait LogsBloom {
    fn logs_bloom(&mut self) -> Bloom;
}

impl<'a, I: Iterator<Item = &'a Log>> LogsBloom for I {
    fn logs_bloom(&mut self) -> Bloom {
        self.fold(Bloom::ZERO, |mut bloom, log| {
            bloom.accrue_log(log);
            bloom
        })
    }
}

trait Logs {
    fn logs(&mut self) -> Vec<Log>;
}

impl Logs for NativeContextExtensions<'_> {
    fn logs(&mut self) -> Vec<Log> {
        let mut result = Vec::new();
        let events = self.remove::<NativeEventContext>().into_events();
        for (event, _) in events {
            push_logs(&event, &mut result);
        }
        result
    }
}

fn push_logs(event: &ContractEvent, dest: &mut Vec<Log<LogData>>) {
    let (type_tag, event_data) = match event {
        ContractEvent::V1(v1) => (v1.type_tag(), v1.event_data()),
        ContractEvent::V2(v2) => (v2.type_tag(), v2.event_data()),
    };

    let struct_tag = match type_tag {
        TypeTag::Struct(struct_tag) => struct_tag,
        _ => unreachable!("This would break move event extension invariant"),
    };

    // Special case for events coming from EVM native
    if struct_tag.as_ref() == EVM_LOGS_EVENT_TAG.deref() {
        return MoveValue::simple_deserialize(event_data, &EVM_LOGS_EVENT_LAYOUT)
            .ok()
            .and_then(|value| evm_logs_event_to_log(value, dest))
            .expect("EVM logs must deserialize correctly");
    }

    let address = struct_tag.address.to_eth_address();

    let mut hasher = Keccak256::new();
    let type_string = type_tag.to_canonical_string();
    hasher.update(type_string.as_bytes());
    let type_hash = hasher.finalize();

    let topics = vec![type_hash];

    let data = event_data.to_vec();
    let data = data.into();

    let log = Log::new_unchecked(address, topics, data);
    dest.push(log);
}
