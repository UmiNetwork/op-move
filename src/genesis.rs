use std::collections::BTreeMap;

use aptos_table_natives::{TableChange, TableChangeSet};
use move_core_types::effects::{ChangeSet, Op};
use move_vm_runtime::native_extensions::NativeContextExtensions;
use move_vm_test_utils::InMemoryStorage;
use move_vm_types::gas::UnmeteredGasMeter;

use crate::{move_execution::create_move_vm, state_actor::head_release_bundle};

/// Initializes the in-memory storage and integrate the Aptos framework.
pub fn init_storage() -> InMemoryStorage {
    let mut storage = InMemoryStorage::new();

    // Integrate Aptos framework
    let (change_set, table_change_set) =
        deploy_framework(&mut storage).expect("all bundle modules should be valid");

    // Convert aptos table change to move extension table change
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

    for package in &framework.packages {
        let modules = package.sorted_code_and_modules();
        let sender = *modules
            .first()
            .expect("the package has at least one module")
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
    use move_vm_test_utils::InMemoryStorage;

    #[test]
    fn test_deploy_framework() {
        let framework = head_release_bundle();

        let mut storage = InMemoryStorage::new();

        let (change_set, _) = deploy_framework(&mut storage).unwrap();

        assert_eq!(framework.code_and_compiled_modules().len(), 113);
        assert_eq!(
            framework.code_and_compiled_modules().len(),
            change_set.modules().count(),
        );
    }
}
