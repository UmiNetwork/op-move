use std::collections::BTreeMap;

use aptos_table_natives::{TableChange, TableChangeSet};
use move_core_types::effects::{ChangeSet, Op};
use move_vm_runtime::native_extensions::NativeContextExtensions;
use move_vm_test_utils::InMemoryStorage;
use move_vm_types::gas::UnmeteredGasMeter;

use crate::{move_execution::create_move_vm, state_actor::head_release_bundle};

/// Initializes the in-memory storage and integrates the Aptos framework.
pub fn init_storage() -> InMemoryStorage {
    let mut storage = InMemoryStorage::new();

    let (change_set, table_change_set) =
        deploy_framework(&mut storage).expect("All bundle modules should be valid");

    // This function converts a Aptos TableChange to a move table extension struct.
    // InMemoryStorage relies on this conversion to apply storage changes correctly.
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
        .apply_extended(change_set, table_change_set)
        .unwrap();

    storage
}

fn deploy_framework(storage: &mut InMemoryStorage) -> anyhow::Result<(ChangeSet, TableChangeSet)> {
    let framework = head_release_bundle();
    let vm = create_move_vm().unwrap();

    let mut extensions = NativeContextExtensions::default();
    extensions.add(aptos_table_natives::NativeTableContext::new(
        [0; 32], storage,
    ));
    let mut session = vm.new_session_with_extensions(storage, extensions);

    // Iterate over the bundled packages in the Aptos framework
    for package in &framework.packages {
        // Get the sorted list of code and modules from the package
        let modules = package.sorted_code_and_modules();
        // Retrieve the address of the account from the first module
        // Assume the package has at least one module, otherwise, it will panic
        let sender = *modules
            .first()
            .expect("The package has at least one module")
            .1
            .self_id()
            .address();

        let code = modules
            .into_iter()
            .map(|(code, _)| code.to_vec())
            .collect::<Vec<_>>();

        session.publish_module_bundle(code, sender, &mut UnmeteredGasMeter)?;
    }

    let (change_set, mut extensions) = session.finish_with_extensions()?;
    let table_change_set = extensions
        .remove::<aptos_table_natives::NativeTableContext>()
        .into_change_set()?;

    Ok((change_set, table_change_set))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_actor::head_release_bundle;
    use aptos_framework::ReleaseBundle;
    use move_vm_test_utils::InMemoryStorage;
    use once_cell::sync::Lazy;
    use std::process::Command;

    #[test]
    fn test_deploy_framework() {
        // Aptos framework has 113 modules in the head release bundle
        const HEAD_RELEASE_BUNDLE_MODULES_LEN: usize = 113;

        let framework = head_release_bundle();

        let mut storage = InMemoryStorage::new();

        let (change_set, _) = deploy_framework(&mut storage).unwrap();

        assert_eq!(
            framework.code_and_compiled_modules().len(),
            HEAD_RELEASE_BUNDLE_MODULES_LEN
        );
        assert_eq!(
            framework.code_and_compiled_modules().len(),
            change_set.modules().count(),
        );
    }

    #[cfg(unix)]
    // const CUSTOM_RELEASE_BUNDLE_BYTES: &[u8] = include_bytes!("../custom/custom.mrb");
    const CUSTOM_RELEASE_BUNDLE_BYTES: &[u8] = include_bytes!("../custom/bin/head.mrb");

    #[cfg(windows)]
    const CUSTOM_RELEASE_BUNDLE_BYTES: &[u8] = include_bytes!("../custom/custom.mrb");

    static CUSTOM_RELEASE_BUNDLE: Lazy<ReleaseBundle> = Lazy::new(|| {
        bcs::from_bytes::<ReleaseBundle>(CUSTOM_RELEASE_BUNDLE_BYTES).expect("bcs succeeds")
    });

    fn custom_release_bundle() -> &'static ReleaseBundle {
        &CUSTOM_RELEASE_BUNDLE
    }

    #[test]
    fn test_deploy_custom_framework() {
        const CUSTOM_RELEASE_BUNDLE_MODULES_LEN: usize = 77;

        let framework = custom_release_bundle();

        assert_eq!(
            framework.code_and_compiled_modules().len(),
            CUSTOM_RELEASE_BUNDLE_MODULES_LEN
        );
    }

    #[test]
    fn test_cargo_command() {
        // Define the cargo command and its arguments
        let output = Command::new("aptos-framework")
            .current_dir("custom/bin")
            .args(["release"])
            .output()
            .expect("Failed to execute cargo command");

        // Check if the command was successful
        if output.status.success() {
            // Print the stdout
            println!("Output: {}", String::from_utf8_lossy(&output.stdout));
        } else {
            // Print the stderr
            eprintln!("Error: {}", String::from_utf8_lossy(&output.stderr));
        }
    }
}
