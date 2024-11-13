use {
    super::{type_utils::from_move_u256, EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE},
    crate::primitives::ToEthAddress,
    alloy::dyn_abi::DynSolValue,
    aptos_native_interface::{safely_pop_type_arg, SafeNativeContext, SafeNativeResult},
    move_core_types::{
        ident_str,
        language_storage::StructTag,
        value::{MoveStruct, MoveTypeLayout, MoveValue},
    },
    move_vm_types::{loaded_data::runtime_types::Type, values::Value},
    revm::primitives::U256,
    smallvec::{smallvec, SmallVec},
    std::{collections::VecDeque, sync::LazyLock},
};

/// Marker struct defined in our framework for marking data as FixedBytes in the Solidity ABI.
static FIXED_BYTES_TAG: LazyLock<StructTag> = LazyLock::new(|| StructTag {
    address: EVM_NATIVE_ADDRESS,
    module: EVM_NATIVE_MODULE.into(),
    name: ident_str!("SolidityFixedBytes").into(),
    type_args: Vec::new(),
});

/// Marker struct defined in our framework for marking data as FixedArray in the Solidity ABI.
static FIXED_ARRAY_TAG: LazyLock<StructTag> = LazyLock::new(|| StructTag {
    address: EVM_NATIVE_ADDRESS,
    module: EVM_NATIVE_MODULE.into(),
    name: ident_str!("SolidityFixedArray").into(),
    type_args: Vec::new(),
});

/// Implementation for `native fun abi_encode_params<T>(value: &T): vector<u8>;`
///
/// Encode the move value into bytes using the Solidity ABI
/// such that it would be suitable for passing to a Solidity contract's function.
pub fn abi_encode_params(
    context: &mut SafeNativeContext,
    mut ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> SafeNativeResult<SmallVec<[Value; 1]>> {
    debug_assert_eq!(
        ty_args.len(),
        1,
        "abi_encode arg includes the type of the thing to encode"
    );
    debug_assert_eq!(args.len(), 1, "abi_encode arg is only the value to encode");

    // Safety: unwrap is safe because of the length check above
    let value = args.pop_back().unwrap();
    let ty_arg = safely_pop_type_arg!(ty_args);

    // TODO: need to figure out how much gas to charge for these operations.
    let layout = context.type_to_fully_annotated_layout(&ty_arg)?;
    let result = inner_abi_encode_params(value, &layout);

    let result = Value::vector_u8(result);
    Ok(smallvec![result])
}

/// Encode the move value into bytes using the Solidity ABI
/// such that it would be suitable for passing to a Solidity contract's function.
fn inner_abi_encode_params(value: Value, layout: &MoveTypeLayout) -> Vec<u8> {
    let mv = value.as_move_value(layout);
    move_value_to_sol_value(mv).abi_encode_params()
}

fn move_value_to_sol_value(mv: MoveValue) -> DynSolValue {
    match mv {
        MoveValue::Signer(move_address) | MoveValue::Address(move_address) => {
            let evm_address = move_address.to_eth_address();
            DynSolValue::Address(evm_address)
        }
        MoveValue::Bool(b) => DynSolValue::Bool(b),
        MoveValue::U8(x) => DynSolValue::Uint(U256::from(x), 8),
        MoveValue::U16(x) => DynSolValue::Uint(U256::from(x), 16),
        MoveValue::U32(x) => DynSolValue::Uint(U256::from(x), 32),
        MoveValue::U64(x) => DynSolValue::Uint(U256::from(x), 64),
        MoveValue::U128(x) => DynSolValue::Uint(U256::from(x), 128),
        MoveValue::U256(x) => DynSolValue::Uint(from_move_u256(x), 256),
        MoveValue::Vector(xs) => {
            // Special case for byte arrays
            if let Some(MoveValue::U8(_)) = xs.first() {
                return DynSolValue::Bytes(xs.into_iter().map(force_to_u8).collect());
            }

            DynSolValue::Array(xs.into_iter().map(move_value_to_sol_value).collect())
        }
        MoveValue::Struct(inner) => {
            let (struct_tag, mut fields) = match inner {
                MoveStruct::WithTypes { type_, fields } => (type_, fields),
                _ => unreachable!("Must have type because layout is constructed with `type_to_fully_annotated_layout`"),
            };

            // Special case data marked as being fixed bytes
            if FIXED_BYTES_TAG.eq(&struct_tag) {
                let Some((_, MoveValue::Vector(data))) = fields.pop() else {
                    unreachable!("SolidityFixedBytes contains a vector")
                };
                // Solidity only supports fixed bytes up to 32.
                let size = std::cmp::min(data.len(), 32);
                // Fill the fixed-sized buffer with the given bytes
                let mut buf = [0u8; 32];
                for (b, x) in buf
                    .iter_mut()
                    .skip(32 - size)
                    .zip(data.into_iter().map(force_to_u8))
                {
                    *b = x;
                }
                return DynSolValue::FixedBytes(buf.into(), size);
            }

            // Special case data marked as being fixed-sized array.
            // We intentionally do not compare the type args because they
            // are only known at runtime.
            if FIXED_ARRAY_TAG.module_id() == struct_tag.module_id()
                && FIXED_ARRAY_TAG.name == struct_tag.name
            {
                let Some((_, MoveValue::Vector(data))) = fields.pop() else {
                    unreachable!("SolidityFixedArray contains a vector")
                };
                let data = data.into_iter().map(move_value_to_sol_value).collect();
                return DynSolValue::FixedArray(data);
            }

            // Assume all other structs are meant to be tuples.
            let fields = fields.into_iter().map(|(_, mv)| mv);
            DynSolValue::Tuple(fields.map(move_value_to_sol_value).collect())
        }
    }
}

fn force_to_u8(mv: MoveValue) -> u8 {
    if let MoveValue::U8(x) = mv {
        x
    } else {
        0
    }
}
