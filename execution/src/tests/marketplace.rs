use super::*;

#[test]
fn test_marketplace() {
    // Shows an example of users spending base tokens via a contract.
    // In the EVM this would be done via the value field of a transaction,
    // but in MoveVM we need to use a script which creates the `FungibleAsset`
    // object and passes it to the function as an argument.

    let mut ctx = TestContext::new();
    let module_id = ctx.deploy_contract("marketplace");

    // Initialize marketplace
    let market_address = EVM_ADDRESS.to_move_address();
    let signer = MoveValue::Signer(market_address);
    ctx.execute(&module_id, "init", vec![&signer]);

    // List an item for sale
    let seller_address = EVM_ADDRESS;
    let item_price = U256::from(123);
    let market = MoveValue::Address(market_address);
    let price = MoveValue::U256(item_price.to_move_u256());
    let thing = MoveValue::vector_u8(b"Something valuable".to_vec());
    ctx.execute(&module_id, "list", vec![&market, &price, &thing, &signer]);

    // Mint tokens for the buyer to spend
    let buyer_address = ALT_EVM_ADDRESS;
    let mint_amount = U256::from(567);
    ctx.deposit_eth(buyer_address, mint_amount);

    // Buy the item from the marketplace using the script
    ctx.signer = Signer::new(&ALT_PRIVATE_KEY);
    ctx.run_script(
        "marketplace_script",
        vec![
            TransactionArgument::Address(market_address),
            TransactionArgument::U64(0),
            TransactionArgument::U256(U256::from(item_price).to_move_u256()),
        ],
    );
    assert_eq!(ctx.get_balance(buyer_address), mint_amount - item_price);
    assert_eq!(ctx.get_balance(seller_address), item_price);
}
