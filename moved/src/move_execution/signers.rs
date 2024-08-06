use {
    crate::InvalidTransactionCause, move_core_types::account_address::AccountAddress,
    move_core_types::value::MoveValue,
};

/// Check that any instances of `MoveValue::Signer` contained within the given `arg`
/// are the `expected_signer`; return an error if not.
pub(super) fn check_signer(arg: &MoveValue, expected_signer: &AccountAddress) -> crate::Result<()> {
    let mut stack = Vec::with_capacity(10);
    stack.push(arg);
    while let Some(arg) = stack.pop() {
        match arg {
            MoveValue::Signer(given_signer) if given_signer != expected_signer => {
                Err(InvalidTransactionCause::InvalidSigner)?
            }
            MoveValue::Vector(values) => {
                for v in values {
                    stack.push(v);
                }
            }
            MoveValue::Struct(s) => {
                for v in s.fields() {
                    stack.push(v);
                }
            }
            _ => (),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*, crate::move_execution::evm_address_to_move_address, crate::tests::EVM_ADDRESS,
        alloy_primitives::address, move_core_types::value::MoveStruct,
    };

    #[test]
    fn test_check_signer() {
        let correct_signer = evm_address_to_move_address(&EVM_ADDRESS);
        let incorrect_signer =
            evm_address_to_move_address(&address!("c104f4840573bed437190daf5d2898c2bdf928ac"));
        type CheckSignerOutcome = Result<(), ()>;

        let test_cases: &[(MoveValue, CheckSignerOutcome)] = &[
            (MoveValue::Address(incorrect_signer), Ok(())),
            (MoveValue::Address(correct_signer), Ok(())),
            (MoveValue::Signer(incorrect_signer), Err(())),
            (MoveValue::Signer(correct_signer), Ok(())),
            (MoveValue::Vector(vec![]), Ok(())),
            (
                MoveValue::Vector(vec![
                    MoveValue::Signer(correct_signer),
                    MoveValue::Signer(correct_signer),
                ]),
                Ok(()),
            ),
            (
                MoveValue::Vector(vec![
                    MoveValue::Signer(correct_signer),
                    MoveValue::Signer(correct_signer),
                    MoveValue::Signer(correct_signer),
                ]),
                Ok(()),
            ),
            (
                MoveValue::Vector(vec![
                    MoveValue::Signer(incorrect_signer),
                    MoveValue::Signer(correct_signer),
                ]),
                Err(()),
            ),
            (
                MoveValue::Vector(vec![
                    MoveValue::Signer(correct_signer),
                    MoveValue::Signer(incorrect_signer),
                ]),
                Err(()),
            ),
            (
                MoveValue::Vector(vec![
                    MoveValue::Signer(correct_signer),
                    MoveValue::Signer(correct_signer),
                    MoveValue::Signer(incorrect_signer),
                ]),
                Err(()),
            ),
            (
                MoveValue::Vector(vec![
                    MoveValue::U32(0),
                    MoveValue::U32(1),
                    MoveValue::U32(2),
                    MoveValue::U32(3),
                ]),
                Ok(()),
            ),
            (MoveValue::Struct(MoveStruct::new(vec![])), Ok(())),
            (
                MoveValue::Struct(MoveStruct::new(vec![
                    MoveValue::U8(0),
                    MoveValue::U16(1),
                    MoveValue::U32(2),
                    MoveValue::U64(3),
                    MoveValue::U128(4),
                    MoveValue::U256(5u64.into()),
                ])),
                Ok(()),
            ),
            (
                MoveValue::Struct(MoveStruct::new(vec![
                    MoveValue::Bool(true),
                    MoveValue::Bool(false),
                    MoveValue::Signer(correct_signer),
                ])),
                Ok(()),
            ),
            (
                MoveValue::Struct(MoveStruct::new(vec![
                    MoveValue::Signer(correct_signer),
                    MoveValue::Vector(vec![MoveValue::Struct(MoveStruct::new(vec![
                        MoveValue::Vector(vec![
                            MoveValue::Struct(MoveStruct::new(vec![
                                MoveValue::Signer(correct_signer),
                                MoveValue::Address(correct_signer),
                            ])),
                            MoveValue::Struct(MoveStruct::new(vec![
                                MoveValue::Signer(correct_signer),
                                MoveValue::Address(incorrect_signer),
                            ])),
                        ]),
                    ]))]),
                ])),
                Ok(()),
            ),
            (
                MoveValue::Struct(MoveStruct::new(vec![
                    MoveValue::Signer(correct_signer),
                    MoveValue::Vector(vec![MoveValue::Struct(MoveStruct::new(vec![
                        MoveValue::Vector(vec![
                            MoveValue::Signer(correct_signer),
                            MoveValue::Signer(incorrect_signer),
                        ]),
                    ]))]),
                ])),
                Err(()),
            ),
        ];

        for (test_case, expected_outcome) in test_cases {
            let actual_outcome = check_signer(test_case, &correct_signer).map_err(|_| ());
            assert_eq!(
                &actual_outcome,
                expected_outcome,
                "check_signer test case {test_case:?} failed. Expected={expected_outcome:?} Actual={actual_outcome:?}"
            );
        }
    }
}
