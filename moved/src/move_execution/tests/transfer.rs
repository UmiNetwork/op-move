use super::*;

/// Deposits can be made to the L2.
#[test]
fn test_deposit_tx() {
    let mut ctx = TestContext::new();

    let mint_amount = 123u64;
    ctx.deposit_eth(EVM_ADDRESS, mint_amount);
    let balance = ctx.get_balance(EVM_ADDRESS);
    assert_eq!(balance, mint_amount);
}

#[test]
fn test_withdrawal_tx() {
    let mut ctx = TestContext::new();

    // 1. Deposit ETH to user
    let mint_amount = 123;
    ctx.deposit_eth(EVM_ADDRESS, mint_amount);

    let balance = ctx.get_balance(EVM_ADDRESS);
    assert_eq!(balance, mint_amount);

    // 2. Use script to withdraw
    let logs = ctx.run_script(
        "withdrawal_script",
        &[],
        vec![
            TransactionArgument::Address(EVM_ADDRESS.to_move_address()),
            TransactionArgument::U256(U256::from(mint_amount).to_move_u256()),
        ],
    );
    assert_eq!(ctx.get_balance(EVM_ADDRESS), 0);
    assert!(
        logs.iter()
            .any(|log| log.address.to_move_address() == L2_CROSS_DOMAIN_MESSENGER_ADDRESS),
        "Outcome must have logs from the L2CrossDomainMessenger contract"
    );
}

#[test]
fn test_eoa_base_token_transfer() {
    // Initialize state
    let mut ctx = TestContext::new();

    // Mint tokens in sender account
    let sender = EVM_ADDRESS;
    let mint_amount = 123;
    ctx.deposit_eth(sender, mint_amount);

    // Should fail when transfer is larger than account balance
    let receiver = ALT_EVM_ADDRESS;
    let transfer_amount = mint_amount.saturating_add(1);
    ctx.transfer(receiver, transfer_amount, 0, true);

    // Should work with proper transfer
    let transfer_amount = mint_amount.wrapping_shr(1);
    ctx.transfer(receiver, transfer_amount, 0, false);

    let sender_balance = ctx.get_balance(sender);
    let receiver_balance = ctx.get_balance(receiver);
    assert_eq!(sender_balance, mint_amount - transfer_amount);
    assert_eq!(receiver_balance, transfer_amount);
}
