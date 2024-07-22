use {
    crate::storage::Storage,
    crate::{move_execution::create_move_vm, state_actor::head_release_bundle},
    aptos_table_natives::{NativeTableContext, TableChange, TableChangeSet},
    lazy_static::lazy_static,
    move_binary_format::errors::PartialVMError,
    move_core_types::{
        account_address::AccountAddress,
        effects::{ChangeSet, Op},
    },
    move_vm_runtime::{native_extensions::NativeContextExtensions, session::Session},
    move_vm_types::gas::UnmeteredGasMeter,
    std::collections::BTreeMap,
    std::fs,
    std::path::PathBuf,
    std::str::FromStr,
    sui_framework::SystemPackage,
    sui_types::base_types::ObjectID,
};

const CRATE_ROOT: &str = env!("CARGO_MANIFEST_DIR");
const SUI_SNAPSHOT_NAME: &str = "sui.mrb";
pub const FRAMEWORK_ADDRESS: AccountAddress = AccountAddress::ONE;
pub const TOKEN_ADDRESS: AccountAddress = AccountAddress::THREE;
pub const TOKEN_OBJECT_ADDRESS: AccountAddress = AccountAddress::FOUR;

lazy_static! {
    static ref SUI_STDLIB_ADDRESS: AccountAddress = AccountAddress::from_str("0x21").unwrap();
    static ref SUI_FRAMEWORK_ADDRESS: AccountAddress = AccountAddress::from_str("0x22").unwrap();
    static ref SUI_STDLIB_PACKAGE_ID: ObjectID = ObjectID::from_str("0x21").unwrap();
    static ref SUI_FRAMEWORK_PACKAGE_ID: ObjectID = ObjectID::from_str("0x22").unwrap();
}

/// Initializes the in-memory storage with integrates the Aptos and Sui frameworks.
pub fn init_storage(storage: &mut impl Storage<Err = PartialVMError>) {
    let (change_set, table_change_set) =
        deploy_framework(storage).expect("All bundle modules should be valid");

    // This function converts a `TableChange` to a move table extension struct.
    // InMemoryStorage relies on this conversion to apply the storage changes correctly.
    let convert_to_move_extension_table_change = |aptos_table_change: TableChange| {
        let entries = aptos_table_change
            .entries
            .into_iter()
            .map(|(key, op)| {
                let new_op = match op {
                    Op::New((bytes, _)) => Op::New(bytes),
                    Op::Modify((bytes, _)) => Op::Modify(bytes),
                    Op::Delete => Op::Delete,
                };
                (key, new_op)
            })
            .collect::<BTreeMap<_, _>>();

        move_table_extension::TableChange { entries }
    };
    let table_change_set = move_table_extension::TableChangeSet {
        new_tables: table_change_set.new_tables,
        removed_tables: table_change_set.removed_tables,
        changes: table_change_set
            .changes
            .into_iter()
            .map(|(k, v)| (k, convert_to_move_extension_table_change(v)))
            .collect(),
    };

    storage
        .apply_with_tables(change_set, table_change_set)
        .unwrap();
}

fn deploy_framework(
    storage: &mut impl Storage<Err = PartialVMError>,
) -> anyhow::Result<(ChangeSet, TableChangeSet)> {
    let vm = create_move_vm()?;
    let mut extensions = NativeContextExtensions::default();
    extensions.add(NativeTableContext::new([0u8; 32], storage));
    let mut session = vm.new_session_with_extensions(storage, extensions);

    deploy_aptos_framework(&mut session)?;
    deploy_sui_framework(&mut session)?;

    let (change_set, mut extensions) = session.finish_with_extensions()?;
    let table_change_set = extensions
        .remove::<NativeTableContext>()
        .into_change_set()?;

    Ok((change_set, table_change_set))
}

fn deploy_aptos_framework(session: &mut Session) -> anyhow::Result<()> {
    let framework = head_release_bundle();
    // Iterate over the bundled packages in the Aptos framework
    for package in &framework.packages {
        let modules = package.sorted_code_and_modules();
        // Address from the first module is sufficient as they're the same within the package
        let sender = modules.first().expect("Package has at least one module");
        let sender = *sender.1.self_id().address();

        assert!(
            sender == FRAMEWORK_ADDRESS
                || sender == TOKEN_ADDRESS
                || sender == TOKEN_OBJECT_ADDRESS,
            "The framework should be deployed to a statically known address. {sender} not known."
        );

        let code = modules
            .into_iter()
            .map(|(code, _)| code.to_vec())
            .collect::<Vec<_>>();

        session.publish_module_bundle(code, sender, &mut UnmeteredGasMeter)?;
    }
    Ok(())
}

fn deploy_sui_framework(session: &mut Session) -> anyhow::Result<()> {
    // Load the framework packages from the framework snapshot
    let snapshots = load_bytecode_snapshot()?;
    let stdlib = snapshots
        .get(&SUI_STDLIB_PACKAGE_ID)
        .expect("Sui Move Stdlib package should exist in snapshot")
        .to_owned();
    let framework = snapshots
        .get(&SUI_FRAMEWORK_PACKAGE_ID)
        .expect("Sui Framework package should exist in snapshot")
        .to_owned();

    let mut gas = UnmeteredGasMeter;
    session.publish_module_bundle(stdlib.bytes, *SUI_STDLIB_ADDRESS, &mut gas)?;
    session.publish_module_bundle(framework.bytes, *SUI_FRAMEWORK_ADDRESS, &mut gas)?;
    Ok(())
}

fn load_bytecode_snapshot() -> anyhow::Result<BTreeMap<ObjectID, SystemPackage>> {
    let snapshot_path = PathBuf::from(CRATE_ROOT).join(SUI_SNAPSHOT_NAME);
    let binary = fs::read(snapshot_path)?;
    let snapshots: Vec<SystemPackage> = bcs::from_bytes(&binary)?;
    let packages = snapshots.into_iter().map(|pkg| (pkg.id, pkg)).collect();
    Ok(packages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_actor::head_release_bundle;
    use move_vm_test_utils::InMemoryStorage;

    // Aptos framework has 113 modules and Sui has 70. They are kept mutually exclusive.
    const APTOS_MODULES_LEN: usize = 113;
    const SUI_MODULES_LEN: usize = 70;
    const TOTAL_MODULES_LEN: usize = 183;

    #[test]
    fn test_deploy_framework() {
        let aptos_framework_len = head_release_bundle().code_and_compiled_modules().len();
        assert_eq!(aptos_framework_len, APTOS_MODULES_LEN);
        let sui_framework_len = load_bytecode_snapshot()
            .unwrap()
            .iter()
            .map(|(_id, pkg)| pkg.modules())
            .flatten()
            .count();
        assert_eq!(sui_framework_len, SUI_MODULES_LEN);

        let mut storage = InMemoryStorage::new();
        let (change_set, _) = deploy_framework(&mut storage).unwrap();
        assert_eq!(change_set.modules().count(), TOTAL_MODULES_LEN);
    }
}
