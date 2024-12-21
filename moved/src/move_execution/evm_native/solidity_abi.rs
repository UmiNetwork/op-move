use {
    super::{EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE},
    crate::{
        primitives::{ToEthAddress, ToMoveAddress, ToMoveU256, ToSaturatedU64, ToU256},
    },
    alloy::dyn_abi::{DynSolType, DynSolValue},
    aptos_native_interface::{
        safely_pop_arg, safely_pop_type_arg, SafeNativeContext, SafeNativeResult,
    },
    move_core_types::{
        ident_str,
        language_storage::StructTag,
        value::{MoveStructLayout, MoveTypeLayout, MoveValue},
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

/// Implementation for `native fun abi_encode_params<T>(prefix: vector<u8>, value: T): vector<u8>;`
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
    debug_assert_eq!(
        args.len(),
        2,
        "abi_encode args: prefix bytes, arg to encode"
    );

    // Safety: unwrap is safe because of the length check above
    let value = args.pop_back().unwrap();
    let prefix = args.pop_back().unwrap().value_as::<Vec<u8>>()?;
    let ty_arg = safely_pop_type_arg!(ty_args);

    // TODO: need to figure out how much gas to charge for these operations.

    let undecorated_layout = context.type_to_type_layout(&ty_arg)?;
    let annotated_layout = context.type_to_fully_annotated_layout(&ty_arg)?;
    let encoding = inner_abi_encode_params(value, &undecorated_layout, &annotated_layout);

    let result = Value::vector_u8(prefix.into_iter().chain(encoding));
    Ok(smallvec![result])
}

/// Implementation for `native fun abi_decode_params<T>(value: vector<u8>): T;`
///
/// Decode the Solidity ABI bytes into move value
/// such that it would be suitable for using Solidity contract's return value.
pub fn abi_decode_params(
    context: &mut SafeNativeContext,
    mut ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> SafeNativeResult<SmallVec<[Value; 1]>> {
    debug_assert_eq!(ty_args.len(), 1, "abi_decode arg type to decode into");
    debug_assert_eq!(args.len(), 1, "abi_decode arg to decode");

    let value = safely_pop_arg!(args, Vec<u8>);
    let ty_arg = safely_pop_type_arg!(ty_args);

    // TODO: need to figure out how much gas to charge for these operations.

    let annotated_layout = context.type_to_fully_annotated_layout(&ty_arg)?;

    let result = sol_value_to_move_value(value, &annotated_layout);
    Ok(smallvec![result])
}

/// Encode the move value into bytes using the Solidity ABI
/// such that it would be suitable for passing to a Solidity contract's function.
fn inner_abi_encode_params(
    value: Value,
    undecorated_layout: &MoveTypeLayout,
    annotated_layout: &MoveTypeLayout,
) -> Vec<u8> {
    // It's not possible to construct a `MoveValue` using the annotated layout
    // (the aptos code panics), so we use the undecorated layout to construct the value
    // and then pass in the annotated layout to make use of when converting to a
    // Solidity value.
    let mv = value.as_move_value(undecorated_layout);
    move_value_to_sol_value(mv, annotated_layout).abi_encode_params()
}

fn move_value_to_sol_value(mv: MoveValue, annotated_layout: &MoveTypeLayout) -> DynSolValue {
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
        MoveValue::U256(x) => DynSolValue::Uint(x.to_u256(), 256),
        MoveValue::Vector(xs) => {
            // Special case for byte arrays
            if let Some(MoveValue::U8(_)) = xs.first() {
                return DynSolValue::Bytes(xs.into_iter().map(force_to_u8).collect());
            }

            let MoveTypeLayout::Vector(inner_layout) = annotated_layout else {
                unreachable!("The annotated layout must match the MoveValue")
            };

            DynSolValue::Array(
                xs.into_iter()
                    .map(|x| move_value_to_sol_value(x, inner_layout))
                    .collect(),
            )
        }
        MoveValue::Struct(inner) => {
            let MoveTypeLayout::Struct(struct_layout) = annotated_layout else {
                unreachable!("The annotated layout must match the MoveValue")
            };
            let (struct_tag, field_layouts) = match struct_layout {
                MoveStructLayout::WithTypes { type_, fields } => (type_, fields),
                _ => unreachable!("Must have type because layout is constructed with `type_to_fully_annotated_layout`"),
            };
            let mut fields = inner.into_fields();

            // Special case data marked as being fixed bytes
            if FIXED_BYTES_TAG.eq(struct_tag) {
                let Some(MoveValue::Vector(data)) = fields.pop() else {
                    unreachable!("SolidityFixedBytes contains a vector")
                };
                let size = data.len();

                // Solidity only supports fixed bytes up to 32.
                debug_assert!(
                    0 < size && size <= 32,
                    "Solidity only supports length between 1 and 32 (inclusive). This condition is enforced by the constructor in the Evm.move module"
                );

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
                let Some(MoveValue::Vector(data)) = fields.pop() else {
                    unreachable!("SolidityFixedArray contains a vector")
                };
                let MoveTypeLayout::Vector(inner_layout) = &field_layouts[0].layout else {
                    unreachable!("The annotated layout must match the MoveValue")
                };
                let data = data
                    .into_iter()
                    .map(|x| move_value_to_sol_value(x, inner_layout))
                    .collect();
                return DynSolValue::FixedArray(data);
            }

            // Assume all other structs are meant to be tuples.
            DynSolValue::Tuple(
                fields
                    .into_iter()
                    .zip(field_layouts)
                    .map(|(x, l)| move_value_to_sol_value(x, &l.layout))
                    .collect(),
            )
        }
    }
}

fn sol_value_to_move_value(sv: Vec<u8>, layout: &MoveTypeLayout) -> Value {
    match layout {
        MoveTypeLayout::Bool => {
            let Ok(DynSolValue::Bool(b)) = DynSolType::Bool.abi_decode(&sv) else {
                unreachable!("Solidity value should be  string");
            };
            Value::bool(b)
        }
        MoveTypeLayout::U8 => {
            let Ok(DynSolValue::Uint(u, 8)) = DynSolType::Uint(8).abi_decode(&sv) else {
                unreachable!("Solidity value should be a u8 number");
            };
            Value::u8(u.to_le_bytes_vec()[0])
        }
        MoveTypeLayout::U16 => {
            let Ok(DynSolValue::Uint(u, 16)) = DynSolType::Uint(16).abi_decode(&sv) else {
                unreachable!("Solidity value should be a u16 number");
            };
            let mut v = [0u8; 2];
            for (i, num) in u.to_le_bytes_vec()[0..2].iter().enumerate() {
                v[i] = *num;
            }
            Value::u16(u16::from_le_bytes(v))
        }
        MoveTypeLayout::U32 => {
            let Ok(DynSolValue::Uint(u, 32)) = DynSolType::Uint(32).abi_decode(&sv) else {
                unreachable!("Solidity value should be a u32 number");
            };
            let mut v = [0u8; 4];
            for (i, num) in u.to_le_bytes_vec()[0..4].iter().enumerate() {
                v[i] = *num;
            }
            Value::u32(u32::from_le_bytes(v))
        }
        MoveTypeLayout::U64 => {
            let Ok(DynSolValue::Uint(u, 64)) = DynSolType::Uint(64).abi_decode(&sv) else {
                unreachable!("Solidity value should be a u64 number");
            };
            Value::u64(u.to_saturated_u64())
        }
        MoveTypeLayout::U128 => {
            let Ok(DynSolValue::Uint(u, 128)) = DynSolType::Uint(128).abi_decode(&sv) else {
                unreachable!("Solidity value should be a u128 number");
            };
            let mut v = [0u8; 16];
            for (i, num) in u.to_le_bytes_vec()[0..16].iter().enumerate() {
                v[i] = *num;
            }
            Value::u128(u128::from_le_bytes(v))
        }
        MoveTypeLayout::U256 => {
            let Ok(DynSolValue::Uint(u, 256)) = DynSolType::Uint(256).abi_decode(&sv) else {
                unreachable!("Solidity value should be a u256 number");
            };
            Value::u256(u.to_move_u256())
        }
        MoveTypeLayout::Signer | MoveTypeLayout::Address => {
            let Ok(DynSolValue::Address(evm_address)) = DynSolType::Address.abi_decode(&sv) else {
                unreachable!("Solidity value should be an address");
            };
            Value::address(evm_address.to_move_address())
        }
        MoveTypeLayout::Vector(_vector_layout) => {
            unimplemented!()
        }
        MoveTypeLayout::Struct(_struct_layout) => {
            unimplemented!()
        }
        MoveTypeLayout::Native(_, _) => {
            unreachable!("Native move layout is not supported")
        }
    }
}

fn force_to_u8(mv: MoveValue) -> u8 {
    if let MoveValue::U8(x) = mv {
        x
    } else {
        unreachable!("Only call force_to_u8 with MoveValue::U8")
    }
}
