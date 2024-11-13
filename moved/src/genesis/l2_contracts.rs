use {
    crate::{move_execution, storage::State},
    alloy::genesis::Genesis,
    move_binary_format::errors::PartialVMError,
};

pub fn init_state(genesis: Genesis, state: &mut impl State<Err = PartialVMError>) {
    let changes = move_execution::genesis_state_changes(genesis, state.resolver());
    state
        .apply(changes)
        .expect("L2 contract changes must apply");
}
