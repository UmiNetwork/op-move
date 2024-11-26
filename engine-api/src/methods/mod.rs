pub mod block_number;
pub mod call;
pub mod chain_id;
pub mod estimate_gas;
pub mod fee_history;
pub mod forkchoice_updated;
pub mod get_balance;
pub mod get_block_by_hash;
pub mod get_block_by_number;
pub mod get_nonce;
pub mod get_payload;
pub mod get_transaction_receipt;
pub mod new_payload;
pub mod send_raw_transaction;

#[cfg(test)]
pub mod tests {
    use {
        alloy::primitives::hex,
        move_core_types::account_address::AccountAddress,
        moved::{
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
        tokio::sync::mpsc::{self, Sender},
    };

    pub fn create_state_actor() -> (moved::state_actor::InMemStateActor, Sender<StateMessage>) {
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

        let state = moved::state_actor::StateActor::new(
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
}
