use {
    move_binary_format::errors::PartialVMError,
    move_core_types::{
        account_address::AccountAddress,
        value::{
            MASTER_ADDRESS_FIELD_OFFSET, MoveStruct, MoveStructLayout, MoveTypeLayout, MoveValue,
        },
        vm_status::StatusCode,
    },
    move_vm_types::values::{Struct, VMValueCast, Value, Vector},
};

pub fn value_to_move_value(
    value: Value,
    layout: &MoveTypeLayout,
) -> Result<MoveValue, PartialVMError> {
    match (layout, value) {
        (MoveTypeLayout::Address, Value::Address(x)) => Ok(MoveValue::Address(*x)),
        (MoveTypeLayout::Bool, Value::Bool(x)) => Ok(MoveValue::Bool(x)),
        (MoveTypeLayout::I8, Value::I8(x)) => Ok(MoveValue::I8(x)),
        (MoveTypeLayout::I16, Value::I16(x)) => Ok(MoveValue::I16(x)),
        (MoveTypeLayout::I32, Value::I32(x)) => Ok(MoveValue::I32(x)),
        (MoveTypeLayout::I64, Value::I64(x)) => Ok(MoveValue::I64(x)),
        (MoveTypeLayout::I128, Value::I128(x)) => Ok(MoveValue::I128(x)),
        (MoveTypeLayout::I256, Value::I256(x)) => Ok(MoveValue::I256(*x)),
        (MoveTypeLayout::U8, Value::U8(x)) => Ok(MoveValue::U8(x)),
        (MoveTypeLayout::U16, Value::U16(x)) => Ok(MoveValue::U16(x)),
        (MoveTypeLayout::U32, Value::U32(x)) => Ok(MoveValue::U32(x)),
        (MoveTypeLayout::U64, Value::U64(x)) => Ok(MoveValue::U64(x)),
        (MoveTypeLayout::U128, Value::U128(x)) => Ok(MoveValue::U128(x)),
        (MoveTypeLayout::U256, Value::U256(x)) => Ok(MoveValue::U256(*x)),
        (MoveTypeLayout::Signer, value) => {
            let value: Struct = value.cast()?;
            let address: AccountAddress = value
                .unpack()?
                .nth(MASTER_ADDRESS_FIELD_OFFSET)
                .ok_or(PartialVMError::new_invariant_violation(
                    "Signer address is the second field",
                ))?
                .cast()?;
            Ok(MoveValue::Signer(address))
        }
        (MoveTypeLayout::Struct(layout), value) => {
            let value: Struct = value.cast()?;
            let move_struct = struct_mapping(value, layout)?;
            Ok(MoveValue::Struct(move_struct))
        }
        (MoveTypeLayout::Vector(layout), value) => {
            let value: Vector = value.cast()?;
            Ok(MoveValue::Vector(vector_mapping(value, layout)?))
        }
        // We can't handle this type, so create a placeholder that will raise an error later.
        (MoveTypeLayout::Native(_, _) | MoveTypeLayout::Function, _) => {
            Err(PartialVMError::new(StatusCode::ABORTED)
                .with_message("Function types not supported"))
        }
        _ => Err(PartialVMError::new_invariant_violation(
            "Layout + value mismatch",
        )),
    }
}

fn struct_mapping(value: Struct, layout: &MoveStructLayout) -> Result<MoveStruct, PartialVMError> {
    let fields = value.unpack()?;
    let field_types = layout.fields(None);

    let mapped: Result<Vec<MoveValue>, PartialVMError> = fields
        .zip(field_types)
        .map(|(value, layout)| value_to_move_value(value, layout))
        .collect();

    Ok(MoveStruct::Runtime(mapped?))
}

fn vector_mapping(
    value: Vector,
    layout: &MoveTypeLayout,
) -> Result<Vec<MoveValue>, PartialVMError> {
    let elems = value.unpack_unchecked()?;
    elems
        .into_iter()
        .map(|v| value_to_move_value(v, layout))
        .collect()
}
