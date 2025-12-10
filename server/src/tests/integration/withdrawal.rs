//! Code related to OP-stack withdrawal flow.
//! (Separated into its own module because there is a lot of it.)

use super::*;

mod op_dgf {
    alloy::sol!(
        #[sol(rpc)]
        DisputeGameFactory,
        "src/tests/res/DisputeGameFactory.json"
    );
}

mod op_pdg {
    alloy::sol!(
        #[sol(rpc)]
        PermissionedDisputeGame,
        "src/tests/res/PermissionedDisputeGame.json"
    );
}

mod op_portal {
    alloy::sol!(
        #[sol(rpc)]
        #[derive(Debug)]
        OptimismPortal,
        "src/tests/res/OptimismPortal.json"
    );
}

const MAX_WITHDRAWAL_TIMEOUT: u64 = 16 * 60;
const WITHDRAW_ADDRESS: Address =
    alloy::primitives::address!("4200000000000000000000000000000000000016");

pub async fn withdraw_eth_to_l1(
    chlg: &challenger::ChallengerTask,
    l1_proxies: &L1Addresses,
) -> Result<()> {
    let amount = "1";
    let prefunded_wallet = get_prefunded_wallet().await?;
    let prefunded_address = prefunded_wallet.address();

    // `check_balance=false` balance because tokens are burned.
    let withdraw_tx_hash =
        l2_send_ethers(&prefunded_wallet, WITHDRAW_ADDRESS, amount, false).await?;

    let l1_provider = ProviderBuilder::new()
        .wallet(EthereumWallet::from(prefunded_wallet.clone()))
        .connect_http(Url::parse(&var("L1_RPC_URL")?)?);

    let pre_finalize_balance = l1_provider.get_balance(prefunded_address).await?;

    withdraw_to_l1(withdraw_tx_hash, prefunded_wallet, chlg, l1_proxies).await?;

    let post_finalize_balance = l1_provider.get_balance(prefunded_address).await?;
    assert!(
        pre_finalize_balance < post_finalize_balance,
        "Withdraw should increase funds"
    );

    Ok(())
}

pub async fn withdraw_to_l1(
    withdraw_tx_hash: B256,
    l1_wallet: PrivateKeySigner,
    chlg: &challenger::ChallengerTask,
    l1_addr: &L1Addresses,
) -> Result<()> {
    let l2_provider = ProviderBuilder::new().connect_http(Url::parse(L2_RPC_URL)?);
    let rx = l2_provider
        .get_transaction_receipt(withdraw_tx_hash)
        .await?
        .unwrap();

    // Extract the withdrawal event from the transaction log
    let withdrawal_log = rx
        .inner
        .logs()
        .iter()
        .find(|l| l.address() == WITHDRAW_ADDRESS)
        .unwrap();
    let event = withdraw_event();
    let decoded = event.decode_log(withdrawal_log.data()).unwrap();
    let (withdrawal_hash, _) = decoded.body.last().unwrap().as_fixed_bytes().unwrap();

    // `storage_slot` is calculated based on the Solidity convention for how maps work.
    let slot_preimage = [withdrawal_hash, &[0u8; 32]].concat();
    let storage_slot = alloy::primitives::keccak256(slot_preimage);

    let l1_provider = ProviderBuilder::new()
        .wallet(EthereumWallet::from(l1_wallet))
        .connect_http(Url::parse(&var("L1_RPC_URL")?)?);

    // Contract used on the L1 for withdrawals
    let portal_contract =
        op_portal::OptimismPortal::new(l1_addr.optimism_portal_proxy, &l1_provider);

    // Contract used on the L1 to keep track of the L2 state
    let game_factory =
        op_dgf::DisputeGameFactory::new(l1_addr.dispute_game_factory_proxy, &l1_provider);

    // Wait for proposer to push new blocks top L1
    let withdraw_block_number = withdrawal_log.block_number.unwrap();
    let now = Instant::now();
    let (game_idx, game_block_num) = loop {
        let game_count_resolved = chlg.curr_idx();
        let games = game_factory
            .findLatestGames(1, U256::from(game_count_resolved), U256::from(3))
            .call()
            .await?;

        let mut found_game_idx = U256::ZERO;
        let mut found_game_block_num = 0;
        for game in games.iter().rev() {
            let game_idx = game.index;
            let game = game_factory.gameAtIndex(game_idx).call().await?;
            let permissioned_game = op_pdg::PermissionedDisputeGame::new(game.proxy_, &l1_provider);
            // If the latest L2 block number that the L1 knows about
            // is larger than the block where the withdraw happened then we
            // can move on to the next step. Otherwise we wait before
            // checking again.
            let game_block_number = permissioned_game.l2BlockNumber().call().await?;

            // Timeout to prevent this from being an infinite loop if something breaks
            if now.elapsed().as_secs() > MAX_WITHDRAWAL_TIMEOUT {
                anyhow::bail!(
                "WITHDRAW ERROR: L1 contract `DisputeGameFactory` not updated to block containing withdraw within 10 minutes. CurrentBlock={game_block_number} TargetBlock={withdraw_block_number}"
            );
            }

            let game_status = permissioned_game.status().call().await?;
            // DEFENDER_WINS status
            if game_status == 2 && game_block_number > withdraw_block_number {
                found_game_idx = game_idx;
                found_game_block_num = game_block_number.saturating_to::<u64>();
                break;
            }
        }

        if found_game_idx != U256::ZERO {
            break (found_game_idx, found_game_block_num);
        }

        tokio::time::sleep(Duration::from_secs(30)).await;
    };

    // Look up the corresponding L2 block
    let block = l2_provider
        .get_block_by_number(game_block_num.into())
        .await?
        .unwrap();

    // Get the merkle proof for the withdrawal L2 contract at that height
    let proof = l2_provider
        .get_proof(WITHDRAW_ADDRESS, vec![storage_slot])
        .number(game_block_num)
        .await?;

    // Prepare args for the OptimismPortal contract
    let withdraw_tx = op_portal::Types::WithdrawalTransaction {
        nonce: decoded.indexed[0].as_uint().unwrap().0,
        sender: decoded.indexed[1].as_address().unwrap(),
        target: decoded.indexed[2].as_address().unwrap(),
        value: decoded.body[0].as_uint().unwrap().0,
        gasLimit: decoded.body[1].as_uint().unwrap().0,
        data: decoded.body[2].as_bytes().unwrap().to_vec().into(),
    };
    let output_proof = op_portal::Types::OutputRootProof {
        version: Default::default(),
        stateRoot: block.header.state_root,
        messagePasserStorageRoot: block.header.withdrawals_root.unwrap(),
        latestBlockhash: block.header.hash,
    };

    // Submit proof of withdrawal to L1
    let prove_tx = portal_contract.proveWithdrawalTransaction(
        withdraw_tx.clone(),
        game_idx,
        output_proof,
        proof.storage_proof[0].proof.clone(),
    );

    let pending = prove_tx
        .send()
        .await
        .inspect_err(|e| println!("Prove Err in pending: {e:?}"))?;
    let prove_tx_hash = pending
        .watch()
        .await
        .inspect_err(|e| println!("Prove Err in waiting conf: {e:?}"))?;

    let prove_rx = l1_provider
        .get_transaction_receipt(prove_tx_hash)
        .await?
        .unwrap();
    assert!(prove_rx.status(), "Prove Tx failed");

    // Wait for finalization readiness, it should happen within `max(proofTimestamp + PROOF_MATURITY_DELAY_SECONDS,
    // resolutionTimestamp + DISPUTE_GAME_FINALITY_DELAY_SECONDS)`
    tokio::time::sleep(Duration::from_secs(30)).await;

    // Finalize withdrawal
    let pending = portal_contract
        .finalizeWithdrawalTransaction(withdraw_tx)
        .send()
        .await
        .inspect_err(|e| println!("Finalize Err {e:?}"))?;
    let finalize_tx_hash = pending
        .watch()
        .await
        .inspect_err(|e| println!("Finalize Err {e:?}"))?;
    let finalize_rx = l1_provider
        .get_transaction_receipt(finalize_tx_hash)
        .await?
        .unwrap();
    assert!(finalize_rx.status(), "Finalize Tx failed");

    Ok(())
}

fn withdraw_event() -> alloy::json_abi::Event {
    let message_passed = r#"event MessagePassed(
            uint256 indexed nonce,
            address indexed sender,
            address indexed target,
            uint256 value,
            uint256 gasLimit,
            bytes data,
            bytes32 withdrawalHash
        )"#
    .replace('\n', "");
    alloy::json_abi::Event::parse(&message_passed).unwrap()
}

#[test]
fn test_withdrawal_event() {
    let event = withdraw_event();
    assert_eq!(
        event.selector().0,
        alloy::hex!("02a52367d10742d8032712c1bb8e0144ff1ec5ffda1ed7d70bb05a2744955054")
    );
}
