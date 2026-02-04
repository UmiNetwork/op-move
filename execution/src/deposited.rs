use {
    crate::{
        ADDRESS_LAYOUT, DepositExecutionInput, Logs, U256_LAYOUT, create_vm_session, eth_token,
        gas::{new_gas_meter, total_gas_used},
        session_id::SessionId,
        transaction::{Changes, TransactionExecutionOutcome},
    },
    alloy::primitives::U256,
    aptos_framework::natives::event::NativeEventContext,
    aptos_table_natives::TableResolver,
    aptos_types::vm_status::StatusCode,
    move_binary_format::errors::PartialVMError,
    move_core_types::language_storage::ModuleId,
    move_vm_types::{
        code::ModuleBytesStorage, resolver::ResourceResolver, value_serde::ValueSerDeContext,
        values::Value,
    },
    umi_evm_ext::{
        self, CODE_LAYOUT, EVM_DEPOSIT_FN_NAME, EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE,
        events::EthTransfersLogger,
        extract_evm_changes, extract_evm_result,
        state::{BlockHashLookup, StorageTrieRepository},
    },
    umi_genesis::vm::{RuntimeContext, UmiVm},
    umi_shared::{
        error::{Error, UserError},
        primitives::{ToMoveAddress, ToMoveU256},
    },
};

#[tracing::instrument(level = "debug", skip(input))]
pub(super) fn execute_deposited_transaction<
    S: ResourceResolver + ModuleBytesStorage + TableResolver,
    ST: StorageTrieRepository,
    H: BlockHashLookup,
>(
    input: DepositExecutionInput<S, ST, H>,
) -> umi_shared::error::Result<TransactionExecutionOutcome> {
    let umi_vm = UmiVm::new(input.genesis_config);
    let runtime_context = RuntimeContext::new(&umi_vm, input.state);
    let session_id = SessionId::new_from_deposited(
        input.tx,
        input.tx_hash,
        input.genesis_config,
        input.block_header,
    );
    let eth_transfers_log = EthTransfersLogger::default();
    let mut session = create_vm_session(
        &runtime_context,
        input.state,
        session_id,
        input.storage_trie,
        &eth_transfers_log,
        input.block_hash_lookup,
    );
    // The type of `tx.gas` is essentially `[u64; 1]` so taking the 0th element
    // is a 1:1 mapping to `u64`.
    let mut gas_meter = new_gas_meter(input.genesis_config, input.tx.gas_limit);

    let module = ModuleId::new(EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE.into());
    let function_name = EVM_DEPOSIT_FN_NAME;
    let to_address = match input.tx.to {
        revm::primitives::TxKind::Call(addr) => addr.to_move_address(),
        _ => unimplemented!("Contract creation through deposit tx not yet supported"),
    };
    let args: Result<Vec<Vec<u8>>, Error> = [
        (
            Value::address(input.tx.from.to_move_address()),
            &ADDRESS_LAYOUT,
        ),
        (Value::address(to_address), &ADDRESS_LAYOUT),
        (Value::u256(input.tx.value.to_move_u256()), &U256_LAYOUT),
        (
            Value::vector_u8(input.tx.input.iter().copied()),
            &CODE_LAYOUT,
        ),
    ]
    .into_iter()
    .map(|(value, layout)| {
        Ok(ValueSerDeContext::new(None)
            .serialize(&value, layout)?
            .ok_or_else(|| {
                PartialVMError::new(StatusCode::VALUE_SERIALIZATION_ERROR)
                    .with_message("Failed to serialize EVM deposit args")
            })?)
    })
    .collect();
    let outcome = args
        .and_then(|args| {
            session
                .load_and_execute_function(&mut gas_meter, &module, function_name, &[], args)
                .map_err(Error::from)
        })
        .and_then(|values| {
            let evm_outcome = extract_evm_result(values)?;
            if !evm_outcome.is_success {
                return Err(UserError::DepositFailure(evm_outcome.output).into());
            }

            // If there is a non-zero mint amount then we start by
            // giving those tokens to the EVM native address.
            // The tokens will then be distributed to the correct
            // accounts according to the transfers that happened
            // during EVM execution.
            let mint_amount = input.tx.mint;
            if mint_amount != 0 {
                eth_token::mint_eth(
                    &EVM_NATIVE_ADDRESS,
                    U256::from(mint_amount),
                    &mut session,
                    &mut gas_meter,
                )?;
            }
            eth_token::replicate_transfers(&eth_transfers_log, &mut session, &mut gas_meter)?;

            Ok(evm_outcome.logs)
        });

    let (evm_logs, vm_outcome) = match outcome {
        Ok(logs) => (logs, Ok(())),
        Err(Error::User(e)) => (Vec::new(), Err(e)),
        Err(e) => {
            return Err(e);
        }
    };

    let (mut changes, mut extensions) = session.into_effects_with_extensions()?;
    let events = extensions
        .remove::<NativeEventContext>()
        .legacy_into_events();
    let mut logs = events.logs();
    logs.extend(evm_logs);
    let gas_used = total_gas_used(&gas_meter, input.genesis_config);
    let evm_changes = extract_evm_changes(&extensions)?;
    changes
        .squash(umi_state::AllAccountChanges::from_change_set(
            evm_changes.accounts,
        ))
        .expect("EVM changes must merge with other session changes");
    let changes = Changes::new(
        umi_state::Changes::from_account_changes(changes),
        evm_changes.storage,
    );

    // L2 gas price is set to 0 since deposit transactions are only
    // executed by the system and therefore do not consume any user gas.
    Ok(TransactionExecutionOutcome::new(
        vm_outcome, changes, gas_used, 0, logs, None,
    ))
}
