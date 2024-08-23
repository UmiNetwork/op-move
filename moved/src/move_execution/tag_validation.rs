use {
    crate::{Error, InvalidTransactionCause},
    move_core_types::{
        account_address::AccountAddress,
        ident_str,
        identifier::IdentStr,
        language_storage::{StructTag, TypeTag},
    },
};

const ALLOWED_STRUCTS: [MoveStructInfo<'static>; 5] = [
    MoveStructInfo {
        address: AccountAddress::ONE,
        module: ident_str!("string"),
        name: ident_str!("String"),
    },
    MoveStructInfo {
        address: AccountAddress::ONE,
        module: ident_str!("object"),
        name: ident_str!("Object"),
    },
    MoveStructInfo {
        address: AccountAddress::ONE,
        module: ident_str!("option"),
        name: ident_str!("Option"),
    },
    MoveStructInfo {
        address: AccountAddress::ONE,
        module: ident_str!("fixed_point32"),
        name: ident_str!("FixedPoint32"),
    },
    MoveStructInfo {
        address: AccountAddress::ONE,
        module: ident_str!("fixed_point64"),
        name: ident_str!("FixedPoint64"),
    },
];

/// Only certain types are allowed to be used as arguments for entry functions.
/// This function ensures only allowed types are present.
/// The reason for this restriction is to respect the invariants that Move contracts
/// may have around their types. For example token types and capability types should
/// only be created under specific conditions, not directly deserialized from raw bytes.
pub fn validate_entry_type_tag(tag: &TypeTag) -> crate::Result<()> {
    let mut current_tag = tag;
    loop {
        match current_tag {
            TypeTag::Vector(inner) => {
                // Vectors are allowed iff they hold other allowed types,
                // so we must check the inner type.
                current_tag = inner;
            }
            TypeTag::Struct(struct_tag) => {
                let info = MoveStructInfo::from_tag(struct_tag);

                if !ALLOWED_STRUCTS.contains(&info) {
                    return Err(Error::InvalidTransaction(
                        InvalidTransactionCause::DisallowedEntryFunctionType(tag.clone()),
                    ));
                }

                match struct_tag.type_args.as_slice() {
                    [] => {
                        // No type parameters => nothing more to check
                        return Ok(());
                    }
                    [inner_type] => {
                        // Option is an allowed type, and it has a type parameter.
                        // Option<T> is allowed iff T is allowed.
                        current_tag = inner_type;
                    }
                    _ => unreachable!("No allowed type has more than 1 type parameter"),
                }
            }
            TypeTag::Address
            | TypeTag::Bool
            | TypeTag::Signer
            | TypeTag::U128
            | TypeTag::U16
            | TypeTag::U256
            | TypeTag::U32
            | TypeTag::U64
            | TypeTag::U8 => {
                // Primitive types are allowed.
                // Note that signer is an allowed type because
                // the value is checked in `crate::move_execution::signer`.
                return Ok(());
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
struct MoveStructInfo<'a> {
    pub address: AccountAddress,
    pub module: &'a IdentStr,
    pub name: &'a IdentStr,
}

impl<'a> MoveStructInfo<'a> {
    pub fn from_tag(tag: &'a StructTag) -> Self {
        Self {
            address: tag.address,
            module: &tag.module,
            name: &tag.name,
        }
    }
}

#[test]
fn test_validate_entry_type_tag() {
    struct TestCase {
        typ: TypeTag,
        is_allowed: bool,
    }

    let disallowed_struct = StructTag {
        address: AccountAddress::TWO,
        module: "thing".parse().unwrap(),
        name: "Thing".parse().unwrap(),
        type_args: Vec::new(),
    };

    let disallowed_recursive_struct = StructTag {
        address: AccountAddress::ONE,
        module: "option".parse().unwrap(),
        name: "Option".parse().unwrap(),
        type_args: vec![TypeTag::Struct(Box::new(disallowed_struct.clone()))],
    };

    let allowed_struct = StructTag {
        address: AccountAddress::ONE,
        module: "object".parse().unwrap(),
        name: "Object".parse().unwrap(),
        type_args: Vec::new(),
    };

    let allowed_recursive_struct = StructTag {
        address: AccountAddress::ONE,
        module: "option".parse().unwrap(),
        name: "Option".parse().unwrap(),
        type_args: vec![TypeTag::Struct(Box::new(allowed_struct.clone()))],
    };

    let test_cases = [
        // Primitive types are allowed
        TestCase {
            typ: TypeTag::Address,
            is_allowed: true,
        },
        TestCase {
            typ: TypeTag::Signer,
            is_allowed: true,
        },
        TestCase {
            typ: TypeTag::U64,
            is_allowed: true,
        },
        // Allowed struct
        TestCase {
            typ: TypeTag::Struct(Box::new(allowed_struct)),
            is_allowed: true,
        },
        // Disallowed struct
        TestCase {
            typ: TypeTag::Struct(Box::new(disallowed_struct)),
            is_allowed: false,
        },
        // Allowed recursive struct
        TestCase {
            typ: TypeTag::Struct(Box::new(allowed_recursive_struct)),
            is_allowed: true,
        },
        // Disallowed recursive struct
        TestCase {
            typ: TypeTag::Struct(Box::new(disallowed_recursive_struct)),
            is_allowed: false,
        },
    ];

    for t in test_cases {
        let result = validate_entry_type_tag(&t.typ);
        if t.is_allowed {
            result.unwrap_or_else(|_| panic!("{:?} should be allowed", t.typ));
        } else {
            result.expect_err(&format!("{:?} should NOT be allowed", t.typ));
        }
    }
}
