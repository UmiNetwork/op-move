use super::*;

#[test]
fn test_execute_counter_contract() {
    let mut ctx = TestContext::new();
    let module_id = ctx.deploy_contract("counter");

    // Call entry function to create the `Counter` resource
    let signer_arg = MoveValue::Signer(ctx.move_address);
    let initial_value = MoveValue::U64(7);
    ctx.execute(&module_id, "publish", vec![&signer_arg, &initial_value]);

    // Calling the function with an incorrect signer causes an error
    let signer_arg = MoveValue::Signer(AccountAddress::ZERO);
    let err = ctx.execute_err(&module_id, "publish", vec![&signer_arg, &initial_value]);
    assert_eq!(
        err.to_string(),
        "Signer does not match transaction signature"
    );
    // Reverse the nonce incrementing done in `create_transaction` because of the error
    ctx.signer.nonce -= 1;

    // Resource was created for a struct for the module in the context
    let resource: u64 = ctx.get_resource("counter", "Counter");
    assert_eq!(resource, 7);

    // Call entry function to increment the counter
    let address_arg = MoveValue::Address(ctx.move_address);
    ctx.execute(&module_id, "increment", vec![&address_arg]);

    // Resource was modified
    let resource: u64 = ctx.get_resource("counter", "Counter");
    assert_eq!(resource, 8);
}
