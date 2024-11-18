use {
    super::{
        native_evm_context::NativeEVMContext,
        type_utils::{
            account_info_struct_tag, account_info_to_move_value, account_storage_struct_tag,
            code_hash_struct_tag, get_account_code_hash,
        },
        ACCOUNT_INFO_LAYOUT, ACCOUNT_STORAGE_LAYOUT, CODE_LAYOUT, EVM_NATIVE_ADDRESS,
    },
    crate::primitives::ToMoveU256,
    move_binary_format::errors::PartialVMError,
    move_core_types::{
        effects::{AccountChangeSet, ChangeSet, Op},
        language_storage::StructTag,
        resolver::MoveResolver,
    },
    move_vm_runtime::native_extensions::NativeContextExtensions,
    move_vm_types::values::Value,
    revm::primitives::{
        utilities::KECCAK_EMPTY, Account, AccountInfo, AccountStatus, Address, Bytecode,
        EvmStorageSlot, U256,
    },
};

pub fn genesis_state_changes(
    genesis: alloy::genesis::Genesis,
    resolver: &impl MoveResolver<PartialVMError>,
) -> ChangeSet {
    let mut result = ChangeSet::new();
    let empty_changes = AccountChangeSet::new();
    let mut account_changes = AccountChangeSet::new();
    for (address, genesis_account) in genesis.alloc {
        let (code_hash, code) = match genesis_account.code {
            Some(raw) => {
                let code = Bytecode::new_legacy(raw);
                (code.hash_slow(), Some(code))
            }
            None => (KECCAK_EMPTY, None),
        };
        let storage = genesis_account
            .storage
            .map(|xs| {
                xs.into_iter()
                    .map(|(index, data)| {
                        let index = U256::from_be_bytes(index.0);
                        let data = U256::from_be_bytes(data.0);
                        (index, EvmStorageSlot::new(data))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let account = Account {
            info: AccountInfo {
                balance: genesis_account.balance,
                nonce: genesis_account.nonce.unwrap_or_default(),
                code_hash,
                code,
            },
            storage,
            status: AccountStatus::Touched,
        };
        add_account_changes(
            &address,
            &account,
            resolver,
            &empty_changes,
            &mut account_changes,
        );
    }
    result
        .add_account_changeset(EVM_NATIVE_ADDRESS, account_changes)
        .expect("EVM native changes must be added");
    result
}

pub fn extract_evm_changes(extensions: &NativeContextExtensions) -> ChangeSet {
    let evm_native_ctx = extensions.get::<NativeEVMContext>();
    let mut result = ChangeSet::new();
    let mut account_changes = AccountChangeSet::new();
    for state in &evm_native_ctx.state_changes {
        let mut single_account_changes = AccountChangeSet::new();
        for (address, account) in state {
            // If the account is not touched then there are no changes.
            if !account.is_touched() {
                continue;
            }

            add_account_changes(
                address,
                account,
                evm_native_ctx.resolver,
                &account_changes,
                &mut single_account_changes,
            );
        }
        account_changes
            .squash(single_account_changes)
            .expect("Sequential EVM native changes must merge");
    }
    result
        .add_account_changeset(EVM_NATIVE_ADDRESS, account_changes)
        .expect("EVM native changes must be added");
    result
}

fn add_account_changes(
    address: &Address,
    account: &Account,
    resolver: &dyn MoveResolver<PartialVMError>,
    prior_changes: &AccountChangeSet,
    result: &mut AccountChangeSet,
) {
    debug_assert!(
        account.is_touched(),
        "Untouched accounts are filtered out before calling this function."
    );

    if account.is_selfdestructed() {
        unimplemented!("EVM account self-destruct is not implemented");
    }

    let code_hash = get_account_code_hash(&account.info);

    let resource_exists = |struct_tag: &StructTag| {
        let exists_in_prior_changes = prior_changes.resources().contains_key(struct_tag);
        // Early exit since we don't need to check the resolver if it's in the prior changes.
        if exists_in_prior_changes {
            return exists_in_prior_changes;
        }
        // If not in the prior changes then check the resolver
        resolver
            .get_resource(&EVM_NATIVE_ADDRESS, struct_tag)
            .map(|x| x.is_some())
            .unwrap_or(false)
    };

    // Push AccountInfo resource
    let struct_tag = account_info_struct_tag(address);
    let account_info = account_info_to_move_value(&account.info, code_hash)
        .simple_serialize(&ACCOUNT_INFO_LAYOUT)
        .expect("Account info must serialize");
    let is_created = !resource_exists(&struct_tag);
    let op = if is_created {
        Op::New(account_info.into())
    } else {
        Op::Modify(account_info.into())
    };
    result
        .add_resource_op(struct_tag, op)
        .expect("Resource cannot already exist in result");

    // Push CodeHash resource if needed.
    // We don't need to push anything if the resource already exists.
    let struct_tag = code_hash_struct_tag(&code_hash);
    let code_resource_exists = resource_exists(&struct_tag);
    if !code_resource_exists {
        if let Some(code) = &account.info.code {
            if !code.is_empty() {
                let struct_tag = code_hash_struct_tag(&code_hash);
                let code = Value::vector_u8(code.original_bytes())
                    .simple_serialize(&CODE_LAYOUT)
                    .expect("EVM code must serialize");
                let op = Op::New(code.into());
                // If the same contract is deployed more than once then the same resource
                // could be added twice, but that's ok we can just skip the duplicate.
                result.add_resource_op(struct_tag, op).ok();
            }
        }
    }

    // TODO: If an address self-destructs and then is re-created then its storage
    // must be entirely reset. With the current model we cannot easily delete all the
    // storage for an account (we would need to loop through all the resources for
    // the EVM native). Therefore this may need a redesign if we decide to support
    // EVM self-destruct.
    for (index, value) in account.changed_storage_slots() {
        let struct_tag = account_storage_struct_tag(address, index);
        let op = if value.present_value.is_zero() {
            Op::Delete
        } else {
            let move_value = Value::u256(value.present_value.to_move_u256())
                .simple_serialize(&ACCOUNT_STORAGE_LAYOUT)
                .expect("EVM storage value must serialize");
            if value.original_value.is_zero() {
                Op::New(move_value.into())
            } else {
                Op::Modify(move_value.into())
            }
        };
        result
            .add_resource_op(struct_tag, op)
            .expect("Cannot have duplicate storage index");
    }
}
