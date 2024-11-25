use super::*;

#[test]
fn test_execute_hello_strings_contract() {
    let mut ctx = TestContext::new();
    let module_id = ctx.deploy_contract("hello_strings");

    // Call the contract with valid text; it works.
    let text = "world";
    let input_arg = MoveValue::Struct(MoveStruct::new(vec![MoveValue::Vector(
        text.bytes().map(MoveValue::U8).collect(),
    )]));
    ctx.execute(&module_id, "main", vec![&input_arg]);

    // Try calling the contract with bytes that are not valid UTF-8; get an error.
    let not_utf8: [u8; 2] = [0, 159];
    let input_arg = MoveValue::Struct(MoveStruct::new(vec![MoveValue::Vector(
        not_utf8.into_iter().map(MoveValue::U8).collect(),
    )]));
    let err = ctx.execute_err(&module_id, "main", vec![&input_arg]);
    assert_eq!(err.to_string(), "String must be UTF-8 encoded bytes",);
}

#[test]
fn test_execute_object_playground_contract() {
    let mut ctx = TestContext::new();
    let module_id = ctx.deploy_contract("object_playground");

    // Create the objects
    let move_address = EVM_ADDRESS.to_move_address();
    let signer = MoveValue::Signer(move_address);
    let dest_arg = MoveValue::Address(move_address);
    ctx.execute(&module_id, "create_and_transfer", vec![&signer, &dest_arg]);

    // The object address is deterministic based on the transaction
    let object_address = AccountAddress::new(hex!(
        "81383494fba7aa2410337bc4f16e3d0a196105b22d3317a56d6cbd613c061f5f"
    ));

    // Calls with correct object address work
    let obj_arg = MoveValue::Struct(MoveStruct::new(vec![MoveValue::Address(object_address)]));
    ctx.execute(&module_id, "check_struct1_owner", vec![&signer, &obj_arg]);
    ctx.execute(&module_id, "check_struct1_owner", vec![&signer, &obj_arg]);

    // Calls with a fake object address fail
    let fake_address = AccountAddress::new(hex!(
        "00a1ce00b0b0000deadbeef00ca1100fa1100000000000000000000000000000"
    ));
    let obj_arg = MoveValue::Struct(MoveStruct::new(vec![MoveValue::Address(fake_address)]));
    let err = ctx.execute_err(&module_id, "check_struct2_owner", vec![&signer, &obj_arg]);
    assert_eq!(
        err.to_string(),
        "Object must already exist to pass as an entry function argument",
    );
}
