use {
    alloy::primitives::U256,
    aptos_table_natives::TableResolver,
    move_core_types::{
        account_address::AccountAddress,
        ident_str,
        identifier::IdentStr,
        language_storage::{ModuleId, StructTag},
        value::MoveValue,
    },
    move_vm_runtime::{ModuleStorage, module_traversal::TraversalContext, session::Session},
    move_vm_types::{
        gas::{GasMeter, UnmeteredGasMeter},
        resolver::MoveResolver,
    },
    umi_evm_ext::{EVM_NATIVE_ADDRESS, events::EthTransferLog},
    umi_genesis::FRAMEWORK_ADDRESS,
    umi_shared::{error::EthToken, primitives::ToMoveU256},
};

const TOKEN_ADMIN: AccountAddress = FRAMEWORK_ADDRESS;
const TOKEN_MODULE_NAME: &IdentStr = ident_str!("eth_token");
const MINT_FUNCTION_NAME: &IdentStr = ident_str!("mint");
const TRANSFER_FUNCTION_NAME: &IdentStr = ident_str!("transfer");
const FUNGIBLE_ASSET_MODULE: &IdentStr = ident_str!("fungible_asset_u256");
const FUNGIBLE_ASSET_STORE: &IdentStr = ident_str!("FungibleStore");

/// Address for the Eth token metadata object resource.
/// Derived from `sha3_256([@0x1 | ETH | 0xFE])`.
/// I.e. based on the `create_object_address` function with seed equal to `b"ETH"`.
/// See aptos framework for details:
/// https://github.com/aptos-labs/aptos-core/blob/aptos-node-v1.27.2/aptos-move/framework/aptos-framework/sources/object.move#L216
const ETH_METADATA_ADDRESS: AccountAddress = move_core_types::account_address::AccountAddress::new(
    alloy::hex!("deed7d21428b9ca921615cc0e83e33dbe549568a82caf5ad38b2ddce182a75b4"),
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransferArgs<'a> {
    pub from: &'a AccountAddress,
    pub to: &'a AccountAddress,
    pub amount: U256,
}

pub trait BaseTokenAccounts {
    fn charge_gas_cost<G: GasMeter, MS: ModuleStorage>(
        &self,
        from: &AccountAddress,
        amount: U256,
        session: &mut Session,
        traversal_context: &mut TraversalContext,
        gas_meter: &mut G,
        module_storage: &MS,
    ) -> Result<(), umi_shared::error::Error>;

    fn refund_gas_cost(
        &self,
        to: &AccountAddress,
        amount: U256,
        session: &mut Session,
        traversal_context: &mut TraversalContext,
        module_storage: &impl ModuleStorage,
    ) -> Result<(), umi_shared::error::Error>;

    fn transfer<G: GasMeter, MS: ModuleStorage>(
        &self,
        args: TransferArgs<'_>,
        session: &mut Session,
        traversal_context: &mut TraversalContext,
        gas_meter: &mut G,
        module_storage: &MS,
    ) -> Result<(), umi_shared::error::Error>;
}

#[derive(Debug, Clone)]
pub struct UmiBaseTokenAccounts {
    eth_treasury: AccountAddress,
}

impl UmiBaseTokenAccounts {
    pub fn new(eth_treasury: AccountAddress) -> Self {
        Self { eth_treasury }
    }
}

impl BaseTokenAccounts for UmiBaseTokenAccounts {
    fn charge_gas_cost<G: GasMeter, MS: ModuleStorage>(
        &self,
        from: &AccountAddress,
        amount: U256,
        session: &mut Session,
        traversal_context: &mut TraversalContext,
        gas_meter: &mut G,
        module_storage: &MS,
    ) -> Result<(), umi_shared::error::Error> {
        transfer_eth(
            TransferArgs {
                from,
                to: &self.eth_treasury,
                amount,
            },
            session,
            traversal_context,
            gas_meter,
            module_storage,
        )
    }

    fn refund_gas_cost(
        &self,
        to: &AccountAddress,
        amount: U256,
        session: &mut Session,
        traversal_context: &mut TraversalContext,
        module_storage: &impl ModuleStorage,
    ) -> Result<(), umi_shared::error::Error> {
        let mut gas_meter = UnmeteredGasMeter;
        transfer_eth(
            TransferArgs {
                from: &self.eth_treasury,
                to,
                amount,
            },
            session,
            traversal_context,
            &mut gas_meter,
            module_storage,
        )
    }

    fn transfer<G: GasMeter, MS: ModuleStorage>(
        &self,
        args: TransferArgs<'_>,
        session: &mut Session,
        traversal_context: &mut TraversalContext,
        gas_meter: &mut G,
        module_storage: &MS,
    ) -> Result<(), umi_shared::error::Error> {
        transfer_eth(args, session, traversal_context, gas_meter, module_storage)
    }
}

pub fn mint_eth<G: GasMeter>(
    to: &AccountAddress,
    amount: U256,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
    module_storage: &impl ModuleStorage,
) -> Result<(), umi_shared::error::Error> {
    if amount.is_zero() {
        return Ok(());
    }
    let token_module_id = ModuleId::new(FRAMEWORK_ADDRESS, TOKEN_MODULE_NAME.into());
    let admin_arg = bcs::to_bytes(&MoveValue::Signer(TOKEN_ADMIN)).expect("signer can serialize");
    let to_arg = bcs::to_bytes(to).expect("address can serialize");
    let amount_arg =
        bcs::to_bytes(&MoveValue::U256(amount.to_move_u256())).expect("amount can serialize");

    let function =
        session.load_function(module_storage, &token_module_id, MINT_FUNCTION_NAME, &[])?;

    session
        .execute_entry_function(
            function,
            vec![
                admin_arg.as_slice(),
                to_arg.as_slice(),
                amount_arg.as_slice(),
            ],
            gas_meter,
            traversal_context,
            module_storage,
        )
        .map_err(|e| {
            tracing::error!("mint_eth error: {e:?}");
            umi_shared::error::Error::eth_token_invariant_violation(EthToken::MintAlwaysSucceeds)
        })?;

    Ok(())
}

pub fn transfer_eth<G: GasMeter>(
    args: TransferArgs<'_>,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
    module_storage: &impl ModuleStorage,
) -> Result<(), umi_shared::error::Error> {
    if args.amount.is_zero() {
        return Ok(());
    }
    let token_module_id = ModuleId::new(FRAMEWORK_ADDRESS, TOKEN_MODULE_NAME.into());
    let admin_arg = bcs::to_bytes(&MoveValue::Signer(TOKEN_ADMIN)).expect("signer can serialize");
    let from_arg = bcs::to_bytes(args.from).expect("from address can serialize");
    let to_arg = bcs::to_bytes(args.to).expect("to address can serialize");
    let amount_arg =
        bcs::to_bytes(&MoveValue::U256(args.amount.to_move_u256())).expect("amount can serialize");

    let function = session.load_function(
        module_storage,
        &token_module_id,
        TRANSFER_FUNCTION_NAME,
        &[],
    )?;

    session.execute_entry_function(
        function,
        vec![
            admin_arg.as_slice(),
            from_arg.as_slice(),
            to_arg.as_slice(),
            amount_arg.as_slice(),
        ],
        gas_meter,
        traversal_context,
        module_storage,
    )?;

    Ok(())
}

pub fn replicate_transfers<G: GasMeter, L: EthTransferLog>(
    eth_transfer_logger: &L,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
    module_storage: &impl ModuleStorage,
) -> Result<(), umi_shared::error::Error> {
    // Transfer the transaction value from EVM native account to `origin`.
    // This step is needed because all EVM transactions start with the caller
    // transferring tokens to the EVM native account as part of `evm_call`.
    // We transfer them back to then follow the sequence of transfers that
    // happened in the EVM.
    // Note: in the case of deposit transactions the new based tokens are
    // minted to the EVM native account. So this logic is still needed in
    // that case. The general invariant is that all base tokens used during
    // EVM execution are held by the EVM native account within the MoveVM.
    for (origin, value) in eth_transfer_logger.take_origins() {
        if !value.is_zero() {
            transfer_eth(
                TransferArgs {
                    from: &EVM_NATIVE_ADDRESS,
                    to: &origin,
                    amount: value,
                },
                session,
                traversal_context,
                gas_meter,
                module_storage,
            )?;
        }
    }

    for transfer in eth_transfer_logger.take_transfers() {
        transfer_eth(
            TransferArgs {
                from: &transfer.from,
                to: &transfer.to,
                amount: transfer.amount,
            },
            session,
            traversal_context,
            gas_meter,
            module_storage,
        )?;
    }

    Ok(())
}

/// Simplified API for getting the base token balance with no side effects.
/// Use it only for view methods as it does not use a VM session in the request pipeline.
pub fn quick_get_eth_balance(
    account: &AccountAddress,
    state: &(impl MoveResolver + TableResolver),
) -> Result<U256, umi_shared::error::Error> {
    let struct_tag = StructTag {
        address: FRAMEWORK_ADDRESS,
        module: FUNGIBLE_ASSET_MODULE.into(),
        name: FUNGIBLE_ASSET_STORE.into(),
        type_args: Vec::new(),
    };
    let (Some(bytes), _) = state.get_resource_bytes_with_metadata_and_layout(
        &store_address(account),
        &struct_tag,
        &[],
        None,
    )?
    else {
        return Ok(U256::ZERO);
    };

    // First 32 bytes are the metadata address
    debug_assert_eq!(&bytes[0..32], ETH_METADATA_ADDRESS.as_slice());

    // Next 32 bytes are the balance (little endian encoded)
    let amount_le = &bytes[32..64];
    Ok(U256::from_le_slice(amount_le))
}

/// Compute the address where the `FungibleStore` resource will be located.
/// Based on `create_user_derived_object_address` function with `derive_from` equal to
/// the Eth token metadata address. See aptos framework for details:
/// https://github.com/aptos-labs/aptos-core/blob/aptos-node-v1.27.2/aptos-move/framework/aptos-framework/sources/object.move#L226
fn store_address(owner: &AccountAddress) -> AccountAddress {
    let input = [owner.as_slice(), ETH_METADATA_ADDRESS.as_slice(), &[0xFC]].concat();
    AccountAddress::new(move_vm_types::sha3_256(&input))
}

#[cfg(any(feature = "test-doubles", test))]
mod tests {
    use {super::*, umi_shared::error::Error};

    impl BaseTokenAccounts for () {
        fn charge_gas_cost<G: GasMeter, MS: ModuleStorage>(
            &self,
            _from: &AccountAddress,
            _amount: U256,
            _session: &mut Session,
            _traversal_context: &mut TraversalContext,
            _gas_meter: &mut G,
            _module_storage: &MS,
        ) -> Result<(), Error> {
            Ok(())
        }

        fn transfer<G: GasMeter, MS: ModuleStorage>(
            &self,
            _args: TransferArgs<'_>,
            _session: &mut Session,
            _traversal_context: &mut TraversalContext,
            _gas_meter: &mut G,
            _module_storage: &MS,
        ) -> Result<(), Error> {
            Ok(())
        }

        fn refund_gas_cost(
            &self,
            _to: &AccountAddress,
            _amount: U256,
            _session: &mut Session,
            _traversal_context: &mut TraversalContext,
            _module_storage: &impl ModuleStorage,
        ) -> Result<(), Error> {
            Ok(())
        }
    }
}
