use {
    crate::{genesis::FRAMEWORK_ADDRESS, Error, InvalidTransactionCause, NonceChecking},
    move_core_types::{
        account_address::AccountAddress, ident_str, identifier::IdentStr,
        language_storage::ModuleId, value::MoveValue,
    },
    move_vm_runtime::{module_traversal::TraversalContext, session::Session},
    move_vm_types::{gas::GasMeter, values::Value},
};

const ACCOUNT_MODULE_NAME: &IdentStr = ident_str!("account");
const CREATE_ACCOUNT_FUNCTION_NAME: &IdentStr = ident_str!("create_account_if_does_not_exist");
const GET_NONCE_FUNCTION_NAME: &IdentStr = ident_str!("get_sequence_number");
const INCREMENT_NONCE_FUNCTION_NAME: &IdentStr = ident_str!("increment_sequence_number");

pub(super) fn check_nonce<G: GasMeter>(
    tx_nonce: u64,
    signer: &AccountAddress,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
) -> Result<(), Error> {
    let account_module_id = ModuleId::new(FRAMEWORK_ADDRESS, ACCOUNT_MODULE_NAME.into());
    let addr_arg = bcs::to_bytes(signer).expect("address can serialize");

    session
        .execute_function_bypass_visibility(
            &account_module_id,
            CREATE_ACCOUNT_FUNCTION_NAME,
            Vec::new(),
            vec![addr_arg.as_slice()],
            gas_meter,
            traversal_context,
        )
        .map_err(|_| Error::nonce_invariant_violation(NonceChecking::AnyAccountCanBeCreated))?;

    let account_nonce = {
        let return_values = session
            .execute_function_bypass_visibility(
                &account_module_id,
                GET_NONCE_FUNCTION_NAME,
                Vec::new(),
                vec![addr_arg.as_slice()],
                gas_meter,
                traversal_context,
            )
            .map_err(|_| Error::nonce_invariant_violation(NonceChecking::GetNonceAlwaysSucceeds))?
            .return_values;
        let (raw_output, layout) =
            return_values
                .first()
                .ok_or(Error::nonce_invariant_violation(
                    NonceChecking::GetNonceReturnsAValue,
                ))?;
        let value = Value::simple_deserialize(raw_output, layout)
            .ok_or(Error::nonce_invariant_violation(
                NonceChecking::GetNoneReturnDeserializes,
            ))?
            .as_move_value(layout);
        match value {
            MoveValue::U64(nonce) => nonce,
            _ => {
                return Err(Error::nonce_invariant_violation(
                    NonceChecking::GetNonceReturnsU64,
                ));
            }
        }
    };

    if tx_nonce != account_nonce {
        Err(InvalidTransactionCause::IncorrectNonce {
            expected: account_nonce,
            given: tx_nonce,
        })?;
    }
    if account_nonce == u64::MAX {
        Err(InvalidTransactionCause::ExhaustedAccount)?;
    }

    session
        .execute_function_bypass_visibility(
            &account_module_id,
            INCREMENT_NONCE_FUNCTION_NAME,
            Vec::new(),
            vec![addr_arg.as_slice()],
            gas_meter,
            traversal_context,
        )
        .map_err(|_| {
            Error::nonce_invariant_violation(NonceChecking::IncrementNonceAlwaysSucceeds)
        })?;

    Ok(())
}
