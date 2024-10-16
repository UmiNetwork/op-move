use {
    super::tag_validation::{validate_entry_type_tag, validate_entry_value},
    crate::{error::Error, InvalidTransactionCause, ScriptTransaction},
    aptos_types::transaction::{EntryFunction, Module, Script},
    move_core_types::{
        account_address::AccountAddress, language_storage::TypeTag, value::MoveValue,
    },
    move_vm_runtime::{module_traversal::TraversalContext, session::Session},
    move_vm_types::{gas::GasMeter, loaded_data::runtime_types::Type, values::Value},
};

pub(super) fn execute_entry_function<G: GasMeter>(
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
        validate_entry_value(&tag, &arg, signer, session)?;
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

pub(super) fn execute_script<G: GasMeter>(
    script: Script,
    signer: &AccountAddress,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
) -> crate::Result<()> {
    let function = session.load_script(script.code(), script.ty_args().to_vec())?;
    let serialized_signer = MoveValue::Signer(*signer).simple_serialize().ok_or(
        Error::script_tx_invariant_violation(ScriptTransaction::ArgsMustSerialize),
    )?;
    let args = {
        let mut result = Vec::with_capacity(function.param_tys.len());
        let mut given_args = script.args().iter();
        for ty in &function.param_tys {
            let ty = strip_reference(ty)?;
            let tag = session.get_type_tag(ty)?;

            // Script arguments cannot encode signers so we implicitly
            // insert the known signer to all script parameters that take
            // a Signer type.
            if let TypeTag::Signer = tag {
                result.push(serialized_signer.clone());
                continue;
            }

            let arg = given_args
                .next()
                .ok_or(InvalidTransactionCause::MismatchedArgumentCount)?;
            let serialized_value = MoveValue::from(arg.clone()).simple_serialize().ok_or(
                Error::script_tx_invariant_violation(ScriptTransaction::ArgsMustSerialize),
            )?;
            result.push(serialized_value);
        }

        // All the args should have been used up.
        if given_args.next().is_some() {
            return Err(InvalidTransactionCause::MismatchedArgumentCount.into());
        }

        result
    };
    session.execute_script(
        script.code(),
        script.ty_args().to_vec(),
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

pub(super) fn deploy_module<G: GasMeter>(
    code: Module,
    address: AccountAddress,
    session: &mut Session,
    gas_meter: &mut G,
) -> crate::Result<()> {
    session.publish_module(code.into_inner(), address, gas_meter)?;

    Ok(())
}
