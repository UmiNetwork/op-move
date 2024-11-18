pub use self::{
    native_evm_context::NativeEVMContext,
    native_impl::append_evm_natives,
    state_changes::{extract_evm_changes, genesis_state_changes},
};

use {
    move_core_types::{
        account_address::AccountAddress,
        ident_str,
        identifier::IdentStr,
        value::{MoveStructLayout, MoveTypeLayout},
    },
    std::sync::LazyLock,
};

mod native_evm_context;
mod native_impl;
mod solidity_abi;
mod state_changes;
mod type_utils;

#[cfg(test)]
mod tests;

/// Address where the EVM native is stored
const EVM_NATIVE_ADDRESS: AccountAddress = AccountAddress::ONE;

/// Module name to access the EVM native
const EVM_NATIVE_MODULE: &IdentStr = ident_str!("evm");

/// Layout for elements in EVM account storage (they are simply U256 since EVM models the storage
/// as a map (Address, U256) -> U256).
const ACCOUNT_STORAGE_LAYOUT: MoveTypeLayout = MoveTypeLayout::U256;

/// Layout for EVM byte code. It is simply a byte vector because we store the raw bytes directly.
static CODE_LAYOUT: LazyLock<MoveTypeLayout> =
    LazyLock::new(|| MoveTypeLayout::Vector(Box::new(MoveTypeLayout::U8)));

/// Layout for EVM account info. It is a struct containing three fields: balance, nonce and
/// code_hash. We only store the code_hash, not the entire code since this is the same model
/// that is used in `revm`. It saves space if the same bytecode is deployed more than once
/// because we still only store the whole bytecode one time and simply duplicate the same hash
/// across all the contracts using that bytecode.
static ACCOUNT_INFO_LAYOUT: LazyLock<MoveTypeLayout> = LazyLock::new(|| {
    MoveTypeLayout::Struct(MoveStructLayout::Runtime(vec![
        MoveTypeLayout::U256,                                 // balance
        MoveTypeLayout::U64,                                  // nonce
        MoveTypeLayout::Vector(Box::new(MoveTypeLayout::U8)), // code_hash
    ]))
});
