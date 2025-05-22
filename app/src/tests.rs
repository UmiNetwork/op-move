use {
    super::*,
    crate::TestDependencies,
    alloy::{
        consensus::{SignableTransaction, TxEip1559, TxEnvelope},
        eips::BlockNumberOrTag::{self, *},
        hex,
        network::TxSignerSync,
        primitives::{TxKind, address},
        rpc::types::FeeHistory,
        signers::local::PrivateKeySigner,
    },
    move_core_types::{account_address::AccountAddress, effects::ChangeSet},
    move_vm_runtime::{
        AsUnsyncCodeStorage,
        module_traversal::{TraversalContext, TraversalStorage},
    },
    move_vm_types::gas::UnmeteredGasMeter,
    moved_blockchain::{
        block::{
            Block, BlockHash, BlockRepository, Eip1559GasFee, Header, InMemoryBlockQueries,
            InMemoryBlockRepository, MovedBlockHash,
        },
        in_memory::SharedMemory,
        payload::InMemoryPayloadQueries,
        receipt::{InMemoryReceiptQueries, InMemoryReceiptRepository, ReceiptMemory},
        state::{BlockHeight, InMemoryStateQueries, MockStateQueries, StateQueries},
        transaction::{InMemoryTransactionQueries, InMemoryTransactionRepository},
    },
    moved_evm_ext::state::{InMemoryStorageTrieRepository, StorageTrieRepository},
    moved_execution::{MovedBaseTokenAccounts, create_vm_session, session_id::SessionId},
    moved_genesis::{
        CreateMoveVm, MovedVm,
        config::{CHAIN_ID, GenesisConfig},
    },
    moved_shared::{
        error::{Error, UserError},
        primitives::{Address, B256, ToMoveAddress, U64, U256},
    },
    moved_state::{InMemoryState, ResolverBasedModuleBytesStorage, State},
    test_case::test_case,
};

/// The address corresponding to this private key is 0x8fd379246834eac74B8419FfdA202CF8051F7A03
pub const PRIVATE_KEY: [u8; 32] = [0xaa; 32];

pub const EVM_ADDRESS: Address = address!("8fd379246834eac74b8419ffda202cf8051f7a03");

#[derive(Debug)]
pub struct Signer {
    pub inner: PrivateKeySigner,
    pub nonce: u64,
}

impl Signer {
    pub fn new(key_bytes: &[u8; 32]) -> Self {
        Self {
            inner: PrivateKeySigner::from_bytes(&key_bytes.into()).unwrap(),
            nonce: 0,
        }
    }
}

fn create_app_with_given_queries<SQ: StateQueries + Send + Sync + 'static>(
    height: u64,
    state_queries: SQ,
) -> Application<TestDependencies<SQ>> {
    let genesis_config = GenesisConfig::default();

    let head_hash = B256::new(hex!(
        "e56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d"
    ));
    let genesis_block = Block::default().with_hash(head_hash).with_value(U256::ZERO);

    let mut memory = SharedMemory::new();
    let mut repository = InMemoryBlockRepository::new();

    for i in 0..=height {
        let mut block = genesis_block.clone();
        block.block.header.number = i;
        repository.add(&mut memory, block).unwrap();
    }

    let mut state = InMemoryState::new();
    let mut evm_storage = InMemoryStorageTrieRepository::new();
    let (changes, tables, evm_storage_changes) = moved_genesis_image::load();
    moved_genesis::apply(
        changes,
        tables,
        evm_storage_changes,
        &genesis_config,
        &mut state,
        &mut evm_storage,
    );

    Application {
        mem_pool: Default::default(),
        genesis_config,
        base_token: MovedBaseTokenAccounts::new(AccountAddress::ONE),
        block_hash: MovedBlockHash,
        block_queries: InMemoryBlockQueries,
        block_repository: repository,
        on_payload: CommandActor::on_payload_noop(),
        on_tx: CommandActor::on_tx_noop(),
        on_tx_batch: CommandActor::on_tx_batch_noop(),
        payload_queries: InMemoryPayloadQueries::new(),
        receipt_queries: InMemoryReceiptQueries::new(),
        receipt_repository: InMemoryReceiptRepository::new(),
        receipt_memory: ReceiptMemory::new(),
        storage: memory,
        state,
        state_queries,
        evm_storage,
        transaction_queries: InMemoryTransactionQueries::new(),
        transaction_repository: InMemoryTransactionRepository::new(),
        gas_fee: Eip1559GasFee::default(),
        l1_fee: U256::ZERO,
        l2_fee: U256::ZERO,
    }
}

fn mint_eth(
    state: &impl State,
    evm_storage: &impl StorageTrieRepository,
    addr: AccountAddress,
    amount: U256,
) -> ChangeSet {
    let moved_vm = MovedVm::new(&Default::default());
    let module_bytes_storage = ResolverBasedModuleBytesStorage::new(state.resolver());
    let code_storage = module_bytes_storage.as_unsync_code_storage(&moved_vm);
    let vm = moved_vm.create_move_vm().unwrap();
    let mut session = create_vm_session(
        &vm,
        state.resolver(),
        SessionId::default(),
        evm_storage,
        &(),
        &(),
    );
    let traversal_storage = TraversalStorage::new();
    let mut traversal_context = TraversalContext::new(&traversal_storage);
    let mut gas_meter = UnmeteredGasMeter;

    moved_execution::mint_eth(
        &addr,
        amount,
        &mut session,
        &mut traversal_context,
        &mut gas_meter,
        &code_storage,
    )
    .unwrap();

    session.finish(&code_storage).unwrap()
}

fn create_app_with_fake_queries(
    addr: AccountAddress,
    initial_balance: U256,
) -> Application<TestDependencies> {
    let genesis_config = GenesisConfig::default();

    let head_hash = B256::new(hex!(
        "e56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d"
    ));
    let genesis_block = Block::default().with_hash(head_hash).with_value(U256::ZERO);

    let mut memory = SharedMemory::new();
    let mut repository = InMemoryBlockRepository::new();
    repository.add(&mut memory, genesis_block).unwrap();

    let evm_storage = InMemoryStorageTrieRepository::new();
    let mut state = InMemoryState::new();
    let (genesis_changes, table_changes, evm_storage_changes) = moved_genesis_image::load();
    state
        .apply_with_tables(genesis_changes.clone(), table_changes)
        .unwrap();
    evm_storage.apply(evm_storage_changes).unwrap();
    let changes_addition = mint_eth(&state, &evm_storage, addr, initial_balance);
    state.apply(changes_addition.clone()).unwrap();

    let state_queries = InMemoryStateQueries::from_genesis(state.state_root());

    Application::<TestDependencies> {
        mem_pool: Default::default(),
        genesis_config,
        base_token: MovedBaseTokenAccounts::new(AccountAddress::ONE),
        block_hash: MovedBlockHash,
        block_queries: InMemoryBlockQueries,
        block_repository: repository,
        on_payload: CommandActor::on_payload_in_memory(),
        on_tx: CommandActor::on_tx_in_memory(),
        on_tx_batch: CommandActor::on_tx_batch_in_memory(),
        payload_queries: InMemoryPayloadQueries::new(),
        receipt_queries: InMemoryReceiptQueries::new(),
        receipt_repository: InMemoryReceiptRepository::new(),
        receipt_memory: ReceiptMemory::new(),
        storage: memory,
        state,
        state_queries,
        evm_storage,
        transaction_queries: InMemoryTransactionQueries::new(),
        transaction_repository: InMemoryTransactionRepository::new(),
        gas_fee: Eip1559GasFee::default(),
        l1_fee: U256::ZERO,
        l2_fee: U256::ZERO,
    }
}

#[test]
fn test_build_block_hash() {
    use alloy::{hex, primitives::address};

    let payload_attributes = Payload {
        timestamp: U64::from(0x6759e370_u64),
        prev_randao: B256::new(hex!(
            "ade920edae8d7bb10146e7baae162b5d5d8902c5a2a4e9309d0bf197e7fdf9b6"
        )),
        suggested_fee_recipient: address!("4200000000000000000000000000000000000011"),
        withdrawals: Vec::new(),
        parent_beacon_block_root: Default::default(),
        transactions: Vec::new(),
        gas_limit: U64::from(0x1c9c380),
    };

    let execution_outcome = ExecutionOutcome {
        receipts_root: B256::new(hex!(
            "3c55e3bccc48ee3ee637d8fc6936e4825d1489cbebf6057ce8025d63755ebf54"
        )),
        state_root: B256::new(hex!(
            "5affa0c563587bc4668feaea28e997d29961e864be20b0082d123bcb2fbbaf55"
        )),
        logs_bloom: Default::default(),
        gas_used: U64::from(0x272a2),
        total_tip: Default::default(),
    };

    let header = Header {
        parent_hash: B256::new(hex!(
            "966c80cc0cbf7dbf7a2b2579002b95c8756f388c3fbf4a77c4d94d3719880c6e"
        )),
        number: 1,
        transactions_root: B256::new(hex!(
            "c355179c91ebb544d6662d6ad580c45eb3f155e1626b693b3afa4fdca677c450"
        )),
        base_fee_per_gas: Some(0x3b5dc100),
        blob_gas_used: Some(0),
        excess_blob_gas: Some(0),
        withdrawals_root: Some(B256::new(hex!(
            "56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421"
        ))),
        ..Default::default()
    }
    .with_payload_attributes(payload_attributes)
    .with_execution_outcome(execution_outcome);

    let hash = MovedBlockHash.block_hash(&header);
    assert_eq!(
        hash,
        B256::new(hex!(
            "c9f7a6ef5311bf49b8322a92f3d75bd5c505ee613323fb58c7166c3511a62bcf"
        ))
    );
}

#[test_case(Latest, 4, 4; "Latest")]
#[test_case(Finalized, 4, 4; "Finalized")]
#[test_case(Safe, 4, 4; "Safe")]
#[test_case(Earliest, 4, 0; "Earliest")]
#[test_case(Pending, 4, 4; "Pending")]
#[test_case(Number(2), 4, 2; "Number")]
fn test_nonce_is_fetched_by_height_successfully(
    height: BlockNumberOrTag,
    head_height: BlockHeight,
    expected_height: BlockHeight,
) {
    let address = Address::new(hex!("11223344556677889900ffeeaabbccddee111111"));
    let app = create_app_with_given_queries(
        head_height,
        MockStateQueries(address.to_move_address(), expected_height),
    );

    let actual_nonce = app.nonce_by_height(address, height).unwrap();
    let expected_nonce = 3;

    assert_eq!(actual_nonce, expected_nonce);
}

#[test_case(Latest, 2, 2; "Latest")]
#[test_case(Finalized, 2, 2; "Finalized")]
#[test_case(Safe, 2, 2; "Safe")]
#[test_case(Earliest, 2, 0; "Earliest")]
#[test_case(Pending, 2, 2; "Pending")]
#[test_case(Number(1), 2, 1; "Number")]
fn test_balance_is_fetched_by_height_successfully(
    height: BlockNumberOrTag,
    head_height: BlockHeight,
    expected_height: BlockHeight,
) {
    let address = Address::new(hex!("44223344556677889900ffeeaabbccddee111111"));
    let app = create_app_with_given_queries(
        head_height,
        MockStateQueries(address.to_move_address(), expected_height),
    );

    let actual_balance = app.balance_by_height(address, height).unwrap();
    let expected_balance = U256::from(5);

    assert_eq!(actual_balance, expected_balance);
}

fn create_transaction(nonce: u64) -> TxEnvelope {
    let to = Address::new(hex!("44223344556677889900ffeeaabbccddee111111"));
    let amount = U256::from(4);
    let signer = Signer::new(&PRIVATE_KEY);
    let mut tx = TxEip1559 {
        chain_id: CHAIN_ID,
        nonce: signer.nonce + nonce,
        gas_limit: u64::MAX,
        max_fee_per_gas: 0,
        max_priority_fee_per_gas: 0,
        to: TxKind::Call(to),
        value: amount,
        access_list: Default::default(),
        input: Default::default(),
    };
    let signature = signer.inner.sign_transaction_sync(&mut tx).unwrap();

    TxEnvelope::Eip1559(tx.into_signed(signature))
}

#[test]
fn test_fetched_balances_are_updated_after_transfer_of_funds() {
    let to = Address::new(hex!("44223344556677889900ffeeaabbccddee111111"));
    let initial_balance = U256::from(10);
    let amount = U256::from(4);
    let mut app = create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), initial_balance);

    let tx = create_transaction(0);

    app.add_transaction(tx);
    app.start_block_build(Default::default(), U64::from(0x03421ee50df45cacu64));

    let actual_recipient_balance = app.balance_by_height(to, Latest).unwrap();
    let expected_recipient_balance = amount;

    assert_eq!(actual_recipient_balance, expected_recipient_balance);

    let actual_sender_balance = app.balance_by_height(EVM_ADDRESS, Latest).unwrap();
    let expected_sender_balance = initial_balance - amount;

    assert_eq!(actual_sender_balance, expected_sender_balance);
}

#[test]
fn test_fetched_nonces_are_updated_after_executing_transaction() {
    let to = Address::new(hex!("44223344556677889900ffeeaabbccddee111111"));
    let initial_balance = U256::from(5);
    let mut app = create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), initial_balance);

    let tx = create_transaction(0);

    app.add_transaction(tx);
    app.start_block_build(Default::default(), U64::from(0x03421ee50df45cacu64));

    let actual_recipient_balance = app.nonce_by_height(to, Latest).unwrap();
    let expected_recipient_balance = 0;

    assert_eq!(actual_recipient_balance, expected_recipient_balance);

    let actual_sender_balance = app.nonce_by_height(EVM_ADDRESS, Latest).unwrap();
    let expected_sender_balance = 1;

    assert_eq!(actual_sender_balance, expected_sender_balance);
}

#[test]
fn test_one_payload_can_be_fetched_repeatedly() {
    let initial_balance = U256::from(5);
    let mut app = create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), initial_balance);

    let tx = create_transaction(0);

    app.add_transaction(tx);

    let payload_id = U64::from(0x03421ee50df45cacu64);

    app.start_block_build(Default::default(), payload_id);

    let expected_payload = app.payload(payload_id);
    let actual_payload = app.payload(payload_id);

    assert_eq!(expected_payload, actual_payload);
}

#[test]
fn test_older_payload_can_be_fetched_again_successfully() {
    let initial_balance = U256::from(15);
    let mut app = create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), initial_balance);

    let tx = create_transaction(0);

    app.add_transaction(tx);

    let payload_id = U64::from(0x03421ee50df45cacu64);

    app.start_block_build(
        Payload {
            gas_limit: U64::MAX,
            ..Default::default()
        },
        payload_id,
    );

    let expected_payload = app.payload(payload_id);

    let tx = create_transaction(1);

    app.add_transaction(tx);

    let payload_2_id = U64::from(0x03421ee50df45dadu64);

    app.start_block_build(
        Payload {
            timestamp: U64::from(1u64),
            gas_limit: U64::MAX,
            ..Default::default()
        },
        payload_2_id,
    );

    app.payload(payload_2_id);

    let actual_payload = app.payload(payload_id);

    assert_eq!(expected_payload, actual_payload);
}

#[test]
fn test_txs_from_one_account_have_proper_nonce_ordering() {
    let initial_balance = U256::from(1000);
    let mut app = create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), initial_balance);

    let mut tx_hashes: Vec<B256> = Vec::with_capacity(10);

    for i in 0..10 {
        let tx = create_transaction(i);
        tx_hashes.push(tx.tx_hash().0.into());
        app.add_transaction(tx);
    }

    let payload_id = U64::from(0x03421ee50df45cacu64);

    app.start_block_build(Default::default(), payload_id);

    for (i, tx_hash) in tx_hashes.iter().enumerate() {
        // Get receipt for this transaction
        let receipt = app.transaction_receipt(*tx_hash);

        assert!(
            receipt.is_some(),
            "Transaction with nonce {} and hash {:?} has no receipt",
            i,
            tx_hash
        );

        let receipt = receipt.unwrap();

        assert!(
            receipt.inner.inner.status(),
            "Transaction with nonce {} and hash {:?} failed",
            i,
            tx_hash
        );

        assert!(
            receipt
                .inner
                .transaction_index
                .is_some_and(|idx| idx == i as u64),
            "Transaction with nonce {} has incorrect index {:?}",
            i,
            receipt.inner.transaction_index
        );

        assert_eq!(
            receipt.inner.from, EVM_ADDRESS,
            "Transaction with nonce {} has unexpected sender",
            i
        );
    }

    let payload = app.payload(payload_id);
    assert!(
        payload
            .as_ref()
            .is_some_and(|p| p.execution_payload.transactions.len() == 10),
        "Expected {} transactions in block, but found {:?}",
        10,
        payload.map(|p| p.execution_payload.transactions.len())
    );
}

#[test_case(0, None => matches Err(Error::User(UserError::InvalidBlockCount)); "zero block count")]
#[test_case(5, None => matches Ok(_); "block count too long")]
#[test_case(1, Some(vec![0.0; 101]) => matches Err(Error::User(UserError::RewardPercentilesTooLong)); "too many percentiles")]
#[test_case(1, Some(vec![50.0, 101.0]) => matches Err(Error::User(UserError::InvalidRewardPercentiles)); "percentile out of range")]
#[test_case(1, Some(vec![-5.0]) => matches Err(Error::User(UserError::InvalidRewardPercentiles)); "negative percentile")]
#[test_case(1, Some(vec![75.0, 25.0, 50.0]) => matches Err(Error::User(UserError::InvalidRewardPercentiles)); "unsorted percentiles")]
#[test_case(1, Some(vec![25.0, 50.0, 75.0]) => matches Ok(_); "valid percentiles")]
#[test_case(1, None => matches Ok(_); "no percentiles")]
fn test_fee_history_validation(
    block_count: u64,
    percentiles: Option<Vec<f64>>,
) -> Result<FeeHistory, Error> {
    let address = Address::new(hex!("11223344556677889900ffeeaabbccddee111111"));
    let app = create_app_with_given_queries(1, MockStateQueries(address.to_move_address(), 0));
    app.fee_history(block_count, Latest, percentiles)
}

#[test_case(1, Latest, 5; "single block latest")]
#[test_case(2, Latest, 4; "two blocks latest")]
#[test_case(100, Latest, 0; "block count exceeds available")]
#[test_case(2, Earliest, 0; "earliest block")]
#[test_case(2, Number(3), 2; "specific block number")]
#[test_case(1, Number(0), 0; "genesis block")]
fn test_fee_history_block_ranges(
    block_count: u64,
    block_tag: BlockNumberOrTag,
    expected_oldest: u64,
) {
    let address = Address::new(hex!("11223344556677889900ffeeaabbccddee111111"));
    let app = create_app_with_given_queries(5, MockStateQueries(address.to_move_address(), 0));

    let result = app.fee_history(block_count, block_tag, None);
    assert!(result.is_ok());

    let fee_history = result.unwrap();
    assert_eq!(fee_history.oldest_block, expected_oldest);
}

#[test_case(None, 1; "no percentiles")]
#[test_case(Some(vec![50.0]), 1; "single percentile")]
#[test_case(Some(vec![25.0, 50.0, 75.0]), 3; "triple percentiles")]
#[test_case(Some(vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0]), 9; "many percentiles")]
fn test_fee_history_reward_lengths(percentiles: Option<Vec<f64>>, expected_reward_length: usize) {
    let address = Address::new(hex!("11223344556677889900ffeeaabbccddee111111"));
    let app = create_app_with_given_queries(1, MockStateQueries(address.to_move_address(), 0));

    let result = app.fee_history(1, Latest, percentiles);
    assert!(result.is_ok());

    let fee_history = result.unwrap();

    match &fee_history.reward {
        Some(rewards) => {
            assert_eq!(rewards.len(), 1);
            assert_eq!(rewards[0].len(), expected_reward_length);
        }
        None => assert_eq!(expected_reward_length, 1),
    }
}

#[test]
fn test_fee_history_eip1559_fields() {
    let address = Address::new(hex!("11223344556677889900ffeeaabbccddee111111"));
    let mut app = create_app_with_given_queries(0, MockStateQueries(address.to_move_address(), 1));

    app.start_block_build(Default::default(), U64::from(1));

    let result = app.fee_history(1, Latest, None);
    assert!(result.is_ok());

    let fee_history = result.unwrap();

    // Verify EIP-1559 fields
    assert_eq!(fee_history.base_fee_per_gas.len(), 2); // Current + next block
    assert_eq!(fee_history.gas_used_ratio.len(), 1);

    // Verify EIP-4844 fields are zero (not supported)
    assert_eq!(fee_history.base_fee_per_blob_gas.len(), 2);
    assert!(fee_history.base_fee_per_blob_gas.iter().all(|&x| x == 0));
    assert_eq!(fee_history.blob_gas_used_ratio.len(), 1);
    assert!(fee_history.blob_gas_used_ratio.iter().all(|&x| x == 0.0));
}

#[test_case(0, true; "empty blocks have zero gas ratio")]
#[test_case(5, false; "blocks with transactions have non-zero gas ratio")]
fn test_fee_history_empty_vs_full_blocks(num_txs: usize, expect_zero_ratio: bool) {
    let mut app = create_app_with_fake_queries(EVM_ADDRESS.to_move_address(), U256::from(1000));

    for i in 0..num_txs {
        let tx = create_transaction(i as u64);
        app.add_transaction(tx);
    }

    let payload = Payload {
        gas_limit: U64::from(1_000_000),
        ..Default::default()
    };
    app.start_block_build(payload, U64::from(1));

    let result = app.fee_history(1, Latest, Some(vec![50.0]));
    assert!(result.is_ok());

    let fee_history = result.unwrap();

    if expect_zero_ratio {
        assert_eq!(fee_history.gas_used_ratio[0], 0.0);
    } else {
        assert!(fee_history.gas_used_ratio[0] > 0.0);
    }
}
