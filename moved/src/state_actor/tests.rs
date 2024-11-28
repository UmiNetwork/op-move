use {
    super::*,
    crate::{
        block::{
            Block, BlockMemory, BlockRepository, Eip1559GasFee, InMemoryBlockQueries,
            InMemoryBlockRepository, MovedBlockHash,
        },
        genesis::{config::GenesisConfig, init_state},
        move_execution::MovedBaseTokenAccounts,
        primitives::{B256, U256},
        storage::InMemoryState,
        types::state::StateMessage,
    },
    alloy::{eips::BlockNumberOrTag, primitives::hex},
    move_core_types::account_address::AccountAddress,
    test_case::test_case,
    tokio::sync::{
        mpsc::{self, Sender},
        oneshot,
    },
};

pub fn create_state_actor() -> (InMemStateActor, Sender<StateMessage>) {
    let genesis_config = GenesisConfig::default();
    let (state_channel, rx) = mpsc::channel(10);

    let head_hash = B256::new(hex!(
        "e56ec7ba741931e8c55b7f654a6e56ed61cf8b8279bf5e3ef6ac86a11eb33a9d"
    ));
    let genesis_block = Block::default().with_hash(head_hash).with_value(U256::ZERO);

    let mut block_memory = BlockMemory::new();
    let mut repository = InMemoryBlockRepository::new();
    repository.add(&mut block_memory, genesis_block);

    let mut state = InMemoryState::new();
    init_state(&genesis_config, &mut state);

    let state = StateActor::new(
        rx,
        state,
        head_hash,
        genesis_config,
        0x03421ee50df45cacu64,
        MovedBlockHash,
        repository,
        Eip1559GasFee::default(),
        U256::ZERO,
        MovedBaseTokenAccounts::new(AccountAddress::ONE),
        InMemoryBlockQueries,
        block_memory,
    );
    (state, state_channel)
}

#[test]
fn test_latest_block_height_is_updated_with_newly_built_block() {
    let (mut state_actor, _) = create_state_actor();
    let (tx, rx) = oneshot::channel();

    state_actor.handle_query(Query::BlockByHeight {
        height: Latest,
        include_transactions: false,
        response_channel: tx,
    });

    let result = rx.blocking_recv().unwrap().unwrap();
    let actual_height = result.0.header.number;
    let expected_height = 0;

    assert_eq!(actual_height, expected_height);

    let (tx, _) = oneshot::channel();

    state_actor.handle_command(Command::StartBlockBuild {
        payload_attributes: Default::default(),
        response_channel: tx,
    });

    let (tx, rx) = oneshot::channel();

    state_actor.handle_query(Query::BlockByHeight {
        height: Latest,
        include_transactions: false,
        response_channel: tx,
    });

    let result = rx.blocking_recv().unwrap().unwrap();
    let actual_height = result.0.header.number;
    let expected_height = 1;

    assert_eq!(actual_height, expected_height);
}

#[test_case(Safe; "safe")]
#[test_case(Pending; "pending")]
#[test_case(Finalized; "finalized")]
fn test_latest_block_height_is_same_as_tag(tag: BlockNumberOrTag) {
    let (mut state_actor, _) = create_state_actor();
    let (tx, _) = oneshot::channel();

    state_actor.handle_command(Command::StartBlockBuild {
        payload_attributes: Default::default(),
        response_channel: tx,
    });

    let (tx, rx) = oneshot::channel();

    state_actor.handle_query(Query::BlockByHeight {
        height: Latest,
        include_transactions: false,
        response_channel: tx,
    });

    let result = rx.blocking_recv().unwrap().unwrap();
    let expected_height = result.0.header.number;

    let (tx, rx) = oneshot::channel();

    state_actor.handle_query(Query::BlockByHeight {
        height: tag,
        include_transactions: false,
        response_channel: tx,
    });

    let result = rx.blocking_recv().unwrap().unwrap();
    let actual_height = result.0.header.number;

    assert_eq!(actual_height, expected_height);
}
