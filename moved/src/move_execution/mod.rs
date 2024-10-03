use {
    crate::{
        genesis::config::GenesisConfig,
        primitives::B256,
        types::{
            session_id::SessionId,
            transactions::{ExtendedTxEnvelope, ToLog, TransactionExecutionOutcome},
        },
    },
    alloy_primitives::{Bloom, Log},
    aptos_framework::natives::{
        event::NativeEventContext, object::NativeObjectContext,
        transaction_context::NativeTransactionContext,
    },
    aptos_gas_schedule::{MiscGasParameters, NativeGasParameters, LATEST_GAS_FEATURE_VERSION},
    aptos_table_natives::{NativeTableContext, TableResolver},
    aptos_types::{
        account_address::AccountAddress,
        on_chain_config::{Features, TimedFeaturesBuilder},
    },
    aptos_vm::natives::aptos_natives,
    canonical::execute_canonical_transaction,
    deposited::execute_deposited_transaction,
    move_binary_format::errors::{PartialVMError, PartialVMResult},
    move_core_types::{identifier::Identifier, resolver::MoveResolver},
    move_vm_runtime::{
        move_vm::MoveVM,
        native_extensions::NativeContextExtensions,
        native_functions::{NativeContext, NativeFunction},
        session::Session,
    },
    move_vm_types::{
        loaded_data::runtime_types::Type, natives::function::NativeResult, values::Value,
    },
    std::{collections::VecDeque, sync::Arc},
    sui_move_natives_latest::all_natives,
    sui_move_vm_runtime::native_functions::NativeContext as SuiNativeContext,
    sui_move_vm_types::{
        loaded_data::runtime_types::{CachedTypeIndex, Type as SuiType},
        values::Value as SuiValue,
    },
    triomphe::Arc as TriompheArc,
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

fn convert_type(t: Type) -> SuiType {
    match t {
        Type::Address => SuiType::Address,
        Type::Bool => SuiType::Bool,
        Type::MutableReference(t) => SuiType::MutableReference(Box::new(convert_type(*t))),
        Type::Reference(t) => SuiType::Reference(Box::new(convert_type(*t))),
        Type::Signer => SuiType::Signer,
        Type::Struct { idx, .. } => SuiType::Datatype(CachedTypeIndex(idx.0)),
        Type::StructInstantiation { idx, ty_args, .. } => {
            SuiType::DatatypeInstantiation(Box::new((
                CachedTypeIndex(idx.0),
                ty_args
                    .to_vec()
                    .into_iter()
                    .map(convert_type)
                    .collect::<Vec<_>>(),
            )))
        }
        Type::TyParam(u) => SuiType::TyParam(u),
        Type::Vector(t) => SuiType::Vector(Box::new(convert_type(TriompheArc::unwrap_or_clone(t)))),
        Type::U8 => SuiType::U8,
        Type::U16 => SuiType::U16,
        Type::U32 => SuiType::U32,
        Type::U64 => SuiType::U64,
        Type::U128 => SuiType::U128,
        Type::U256 => SuiType::U256,
    }
}

fn convert_value(
    v: Value,
) -> std::result::Result<SuiValue, move_binary_format::errors::PartialVMError> {
    println!("VALUE: {:?}", v.to_string());
    Err(PartialVMError::new(
        aptos_types::vm_status::StatusCode::ABORTED,
    ))
}

pub fn create_move_vm() -> crate::Result<MoveVM> {
    let natives = aptos_natives(
        LATEST_GAS_FEATURE_VERSION,
        NativeGasParameters::zeros(),
        MiscGasParameters::zeros(),
        TimedFeaturesBuilder::enable_all().build(),
        Features::default(),
    );

    let sui_natives = all_natives(true)
        .into_iter()
        .map(|n| {
            let mut address = n.0.into_bytes();
            address[AccountAddress::LENGTH - 1] += 0x20;
            (
                AccountAddress::new(address),
                Identifier::from_utf8(n.1.into_bytes()).expect("Module identifier should exist"),
                Identifier::from_utf8(n.2.into_bytes()).expect("Function identifier should exist"),
                Arc::new(
                    |a: &mut NativeContext,
                     b: Vec<Type>,
                     c: VecDeque<Value>|
                     -> PartialVMResult<NativeResult> {
                        println!("CONTEXT - GAS: {:?}", a.gas_balance());
                        println!("TYPE: {:?}", b);
                        println!("VALUE: {:?}", c);
                        // Convert the native function inputs to Sui variants
                        // let types = b.into_iter().map(convert_type).collect::<Vec<_>>();
                        // let values = c.into_iter().map(convert_value).collect::<Vec<_>>();

                        let natives = aptos_natives(
                            LATEST_GAS_FEATURE_VERSION,
                            NativeGasParameters::zeros(),
                            MiscGasParameters::zeros(),
                            TimedFeaturesBuilder::enable_all().build(),
                            Features::default(),
                        );
                        let hashing = natives.iter().find(|n| {
                            n.0 == AccountAddress::ONE
                                && n.1 == Identifier::from_utf8("hash".as_bytes().to_vec()).unwrap()
                                && n.2
                                    == Identifier::from_utf8("sha3_256".as_bytes().to_vec())
                                        .unwrap()
                        });

                        // Call the Sui native function
                        hashing.unwrap().3(a, b, c)

                        // Err(PartialVMError::new(
                        //     aptos_types::vm_status::StatusCode::ABORTED,
                        // ))
                    },
                ) as NativeFunction,
            )
        })
        .collect::<Vec<_>>();

    let natives: Vec<_> = natives.into_iter().chain(sui_natives.into_iter()).collect();
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
    tx_hash: &B256,
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
    fn logs(&mut self) -> impl Iterator<Item = Log>;
}

impl Logs for NativeContextExtensions<'_> {
    fn logs(&mut self) -> impl Iterator<Item = Log> {
        self.remove::<NativeEventContext>()
            .into_events()
            .into_iter()
            .map(|(event, ..)| event.to_log())
    }
}
