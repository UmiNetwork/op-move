use {
    crate::{
        genesis::config::GenesisConfig,
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
    tag_validation::{validate_entry_type_tag, validate_entry_value},
};

mod eth_token;
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
        validate_entry_value(&tag, &arg, signer)?;
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
