use {
    move_binary_format::errors::PartialVMError,
    move_core_types::effects::ChangeSet,
    move_table_extension::TableChangeSet,
    moved_genesis::{
        build, config::GenesisConfig, CreateMoveVm, MovedVm, SerdeAllChanges, SerdeChanges,
        SerdeTableChangeSet,
    },
    moved_state::{InMemoryState, State},
    std::io::Write,
};

fn main() {
    let state = InMemoryState::default();
    let genesis_config = GenesisConfig::default();
    let vm = MovedVm;

    save(&vm, &genesis_config, &state);
}

pub fn save(
    vm: &impl CreateMoveVm,
    config: &GenesisConfig,
    state: &impl State<Err = PartialVMError>,
) -> (ChangeSet, TableChangeSet) {
    let path = std::env::var("OUT_DIR").unwrap() + "/genesis.bin";
    let (changes, tables) = build(vm, config, state);
    let changes = SerdeChanges::from(changes);
    let tables = SerdeTableChangeSet::from(tables);
    let all_changes = SerdeAllChanges::new(changes, tables);
    let contents = bcs::to_bytes(&all_changes).unwrap();
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(contents.as_slice()).unwrap();
    file.flush().unwrap();

    (all_changes.changes.into(), all_changes.tables.into())
}
