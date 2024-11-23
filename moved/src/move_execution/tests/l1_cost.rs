use super::*;

#[test]
fn test_treasury_charges_l1_cost_to_sender_account_on_success() {
    let mut ctx = TestContext::new();

    // Mint tokens in sender account
    let sender = EVM_ADDRESS;
    let mint_amount = 123;
    ctx.deposit_eth(sender, mint_amount);

    // Transfer to receiver account
    let l1_cost = 1;
    let receiver = ALT_EVM_ADDRESS;
    let transfer_amount = mint_amount.wrapping_shr(1);
    ctx.transfer(receiver, transfer_amount, l1_cost, false);

    let sender_balance = ctx.get_balance(sender);
    let receiver_balance = ctx.get_balance(receiver);
    assert_eq!(sender_balance, mint_amount - transfer_amount - l1_cost);
    assert_eq!(receiver_balance, transfer_amount);
}

#[test]
fn test_treasury_charges_l1_cost_to_sender_account_on_user_error() {
    let mut ctx = TestContext::new();

    // Mint tokens in sender account
    let sender = EVM_ADDRESS;
    let mint_amount = 123;
    ctx.deposit_eth(sender, mint_amount);

    // Transfer to receiver account
    let l1_cost = 1;
    let receiver = ALT_EVM_ADDRESS;
    let transfer_amount = mint_amount.saturating_add(1);
    ctx.transfer(receiver, transfer_amount, l1_cost, true);

    let sender_balance = ctx.get_balance(sender);
    let receiver_balance = ctx.get_balance(receiver);
    // let treasury_balance = ctx.get_balance(eth_treasury);
    assert_eq!(sender_balance, mint_amount - l1_cost);
    assert_eq!(receiver_balance, 0);
}
