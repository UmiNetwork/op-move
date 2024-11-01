module Evm::evm {
    use aptos_framework::fungible_asset::{Self, FungibleAsset};
    use aptos_framework::primary_fungible_store::ensure_primary_store_exists;
    use EthToken::eth_token::get_metadata;
    use std::error;
    use std::signer;

    /// For now deploying EVM contracts is restricted to an admin account.
    /// This restriction may be lifted in the future.
    const ENOT_OWNER: u64 = 1;

    const OWNER: address = @0x1;

    /// TODO: what capabilities should this have?
    struct EvmLog has drop {
        addr: address,
        topics: vector<u256>,
        data: vector<u8>,
    }

    /// TODO: what capabilities should this have?
    struct EvmResult has drop {
        is_success: bool,
        output: vector<u8>,
        logs: vector<EvmLog>,
    }

    /// Same as `evm_call`, but with the type signature modified to follow the rules of
    /// entry functions (namely: `value` must be zero because `FungibleAsset` cannot
    /// be exernally constructed, and there cannot be a return value).
    public entry fun entry_evm_call(
        caller: &signer,
        to: address,
        data: vector<u8>
    ) {
        let eth_metadata = get_metadata();
        let value = fungible_asset::zero(eth_metadata);
        evm_call(caller, to, value, data);
    }

    public fun evm_call(
        caller: &signer,
        to: address,
        value: FungibleAsset,
        data: vector<u8>
    ): EvmResult {
        native_evm_call(signer::address_of(caller), to, get_asset_value(value), data)
    }

    public fun evm_create(
        caller: &signer,
        value: FungibleAsset,
        data: vector<u8>
    ): EvmResult {
        let caller_addr = signer::address_of(caller);
        assert!(caller_addr == OWNER, error::permission_denied(ENOT_OWNER));

        native_evm_create(caller_addr, get_asset_value(value), data)
    }

    fun get_asset_value(f: FungibleAsset): u256 {
        let amount = fungible_asset::amount(&f);
        if (amount == 0) {
            fungible_asset::destroy_zero(f);
            return 0
        };
        let eth_metadata = get_metadata();
        let store = ensure_primary_store_exists(OWNER, eth_metadata);
        fungible_asset::deposit(store, f);
        (amount as u256)
    }

    native fun native_evm_call(caller: address, to: address, value: u256, data: vector<u8>): EvmResult;
    native fun native_evm_create(caller: address, value: u256, data: vector<u8>): EvmResult;
}
