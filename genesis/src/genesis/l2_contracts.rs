use {
    alloy::genesis::Genesis, move_binary_format::errors::PartialVMError,
    move_core_types::effects::ChangeSet, moved_state::State,
};

pub fn init_state(genesis: Genesis, state: &impl State<Err = PartialVMError>) -> ChangeSet {
    moved_evm_ext::evm_native::genesis_state_changes(genesis, state.resolver())
}
