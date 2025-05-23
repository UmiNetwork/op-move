module Evm::evm {
    use aptos_framework::event;
    use aptos_framework::fungible_asset_u256::{Self, FungibleAsset};
    use aptos_framework::primary_fungible_store_u256::ensure_primary_store_exists;
    use EthToken::eth_token::get_metadata;
    use std::error;
    use std::signer;

    /// Call to `system_evm_create` failed because the EVM execution was
    /// not successful.
    const EDEPLOYMENT_FAILED: u64 = 1;

    /// Call to `system_evm_call` failed because the EVM execution was
    /// not successful.
    const ECALL_FAILED: u64 = 2;

    /// Solidity FixedBytes must have length between 1 and 32 (inclusive).
    const EINVALID_FIXED_BYTES_SIZE: u64 = 3;

    const OWNER: address = @evm_admin;

    struct EvmLog has copy, store, drop {
        addr: address,
        topics: vector<u256>,
        data: vector<u8>
    }

    #[event]
    struct EvmLogsEvent has store, drop {
        logs: vector<EvmLog>
    }

    /// TODO: what capabilities should this have?
    struct EvmResult has drop {
        is_success: bool,
        output: vector<u8>,
        logs: vector<EvmLog>
    }

    /// Marker struct for 0 in binary.
    struct B0 {}

    /// Marker struct for 1 in binary.
    struct B1 {}

    /// Marker struct to encode all byte sizes between 1 and 32 for 
    /// use in the fixed bytes type representation at the type level. The encoding is 
    /// binary and uses `B0` and `B1` as its type parameters. The actual
    /// size is also 1 bigger than the U5 instance, i.e. 11111 in binary stands 
    /// for 32 in decimal.
    struct U5<phantom A, phantom B, phantom C, phantom D, phantom E> {}

    /// Mark a byte array as being of fixed length for the purpose
    /// of encoding it into the Solidity ABI. The only legal phantom type
    /// argument is supposed to be `U5` with `B0` or `B1` inside of it.
    /// Any other type passed into it (which is only available through
    /// the native `abi_decode_params()`) will silently assume a size marker of 32.
    /// 
    /// See the mentioned structs' docstrings for more info.
    struct SolidityFixedBytes<phantom S> has drop {
        data: vector<u8>
    }

    /// Mark a collection of values as being of fixed length for the purpose
    /// of encoding it into the Solidity ABI.
    struct SolidityFixedArray<T> has drop {
        elements: vector<T>
    }

    /// The constructor function for `SolidityFixedBytes`.
    ///
    /// Refer to the U5 docstring for the five generic parameters explanation. While these type
    /// parameters are meant to be exclusively B0 or B1 marker structs defined above, due
    /// to limitations of the Move language this cannot be enforced.
    ///
    /// However, the native implementations of `abi_decode_params()` and `abi_encode_params()`
    /// will treat any other type in type argument position as B1 and only B0 itself will map to B0.
    ///
    /// For user convenience, non-generic wrappers for the most common fixed bytes type
    /// sizes are also provided in this module.
    public fun as_fixed_bytes<A, B, C, D, E>(data: vector<u8>):
        SolidityFixedBytes<U5<A, B, C, D, E>> {
        let actual_size = std::vector::length(&data);

        // Solidity ABI always pads fixed bytes to 32 bytes
        assert!(actual_size == 32, error::invalid_argument(EINVALID_FIXED_BYTES_SIZE));

        SolidityFixedBytes<U5<A, B, C, D, E>> { data }
    }

    /// Specialized convenience function to mark ABI-encoded bytes as bytes1 in Solidity.
    public fun as_fixed_bytes_1(data: vector<u8>): SolidityFixedBytes<U5<B0, B0, B0, B0, B0>> {
        as_fixed_bytes<B0, B0, B0, B0, B0>(data)
    }

    /// Specialized convenience function to mark ABI-encoded bytes as bytes2 in Solidity.
    public fun as_fixed_bytes_2(data: vector<u8>): SolidityFixedBytes<U5<B0, B0, B0, B0, B1>> {
        as_fixed_bytes<B0, B0, B0, B0, B1>(data)
    }

    /// Specialized convenience function to mark ABI-encoded bytes as bytes4 in Solidity.
    public fun as_fixed_bytes_4(data: vector<u8>): SolidityFixedBytes<U5<B0, B0, B0, B1, B1>> {
        as_fixed_bytes<B0, B0, B0, B1, B1>(data)
    }

    /// Specialized convenience function to mark ABI-encoded bytes as bytes8 in Solidity.
    public fun as_fixed_bytes_8(data: vector<u8>): SolidityFixedBytes<U5<B0, B0, B1, B1, B1>> {
        as_fixed_bytes<B0, B0, B1, B1, B1>(data)
    }

    /// Specialized convenience function to mark ABI-encoded bytes as bytes16 in Solidity.
    public fun as_fixed_bytes_16(data: vector<u8>): SolidityFixedBytes<U5<B0, B1, B1, B1, B1>> {
        as_fixed_bytes<B0, B1, B1, B1, B1>(data)
    }

    /// Specialized convenience function to mark ABI-encoded bytes as bytes20 in Solidity.
    public fun as_fixed_bytes_20(data: vector<u8>): SolidityFixedBytes<U5<B1, B0, B0, B1, B1>> {
        as_fixed_bytes<B1, B0, B0, B1, B1>(data)
    }

    /// Specialized convenience function to mark ABI-encoded bytes as bytes32 in Solidity.
    public fun as_fixed_bytes_32(data: vector<u8>): SolidityFixedBytes<U5<B1, B1, B1, B1, B1>> {
        as_fixed_bytes<B1, B1, B1, B1, B1>(data)
    }

    public fun as_fixed_array<T>(elements: vector<T>): SolidityFixedArray<T> {
        SolidityFixedArray { elements }
    }

    /// Emit the EVM logs to MoveVM logging system
    public fun emit_evm_logs(result: &EvmResult) {
        event::emit(EvmLogsEvent { logs: result.logs });
    }

    /// Same as `evm_call`, but with the type signature modified to follow the rules of
    /// entry functions (namely: `value` must be zero because `FungibleAsset` cannot
    /// be exernally constructed, and there cannot be a return value).
    public entry fun entry_evm_call(
        caller: &signer, to: address, data: vector<u8>
    ) {
        let eth_metadata = get_metadata();
        let value = fungible_asset_u256::zero(eth_metadata);
        let result = evm_call(caller, to, value, data);
        emit_evm_logs(&result);
    }

    public fun evm_call(
        caller: &signer,
        to: address,
        value: FungibleAsset,
        data: vector<u8>
    ): EvmResult {
        native_evm_call(
            signer::address_of(caller),
            to,
            get_asset_value(value),
            data
        )
    }

    public fun evm_create(
        caller: &signer, value: FungibleAsset, data: vector<u8>
    ): EvmResult {
        let caller_addr = signer::address_of(caller);

        native_evm_create(caller_addr, get_asset_value(value), data)
    }

    /// Simulate an EVM transaction (as would be done by `evm_call`) without
    /// persisting any changes. This function is good for view methods of
    /// solidity contracts for example. Since this function does not persist
    /// any changes `caller` is not required to be a `signer` (there is no
    /// security risk of "forging" a signature). Similarly, `value` is allowed
    /// to be specified as a raw number instead of being a `FungibleAsset`.
    /// 
    /// Note: even though you are free to specify any address for `caller`,
    /// the EVM will still validate if it is an externally owned address (EOA),
    /// this means if you pass the address of a contract your transaction will
    /// fail with an error `RejectCallerWithCode`. If the `caller` does not
    /// matter to the view call then it is recommended you simply use the zero
    /// address `@0x0` to avoid this error.
    public native fun evm_view(
        caller: address,
        to: address,
        value: u256,
        data: vector<u8>
    ): EvmResult;

    /// Encode the move value into bytes using the Solidity ABI
    /// such that it would be suitable for passing to a Solidity contract's function.
    /// The prefix can be used to prepend the output with a Solidity 4-byte function
    /// selector if needed.
    public native fun abi_encode_params<T>(prefix: vector<u8>, value: T): vector<u8>;

    /// Decode the Solidity ABI bytes into move value
    /// such that it would be suitable for using Solidity contract's return value.
    public native fun abi_decode_params<T>(value: vector<u8>): T;

    /// View function for checking if EVM execution was successful.
    public fun is_result_success(result: &EvmResult): bool {
        result.is_success
    }

    /// View function to retrieve EVM execution output.
    public fun evm_output(result: &EvmResult): vector<u8> {
        result.output
    }

    fun get_asset_value(f: FungibleAsset): u256 {
        let amount = fungible_asset_u256::amount(&f);
        if (amount == 0) {
            fungible_asset_u256::destroy_zero(f);
            return 0
        };
        let eth_metadata = get_metadata();
        let store = ensure_primary_store_exists(OWNER, eth_metadata);
        fungible_asset_u256::deposit(store, f);
        amount
    }

    // A private function used by the system to deploy an EVM contract as
    // requested by a user. Note this function does spend the user's tokens
    // if `value` is non-zero.
    fun system_evm_create(
        caller: &signer,
        value: u256,
        data: vector<u8>
    ): EvmResult {
        let payment = withdraw_payment(caller, value);
        let result = evm_create(caller, payment, data);

        assert!(result.is_success, error::aborted(EDEPLOYMENT_FAILED));
        emit_evm_logs(&result);

        result
    }

    // A private function used by the system to call an EVM contract as
    // requested by a user directly in a transaction.
    // Note this function does spend the user's tokens if `value` is non-zero.
    fun system_evm_call(
        caller: &signer,
        to: address,
        value: u256,
        data: vector<u8>
    ): EvmResult {
        let payment = withdraw_payment(caller, value);
        let result = evm_call(caller, to, payment, data);

        assert!(result.is_success, error::aborted(ECALL_FAILED));
        emit_evm_logs(&result);

        result
    }

    fun withdraw_payment(caller: &signer, value: u256): FungibleAsset {
        let eth_metadata = get_metadata();
        if (value == 0) {
            fungible_asset_u256::zero(eth_metadata)
        } else {
            let caller_addr = signer::address_of(caller);
            let store = ensure_primary_store_exists(caller_addr, eth_metadata);
            fungible_asset_u256::withdraw(caller, store, value)
        }
    }

    // A private function used by the system to deposit a deposited-type transaction.
    // This function only delegates to `native_evm_call` because we cannot call
    // native functions directly for some reason.
    fun system_deposit_evm_call(
        caller: address,
        to: address,
        value: u256,
        data: vector<u8>
    ): EvmResult {
        native_evm_call(caller, to, value, data)
    }

    native fun native_evm_call(
        caller: address,
        to: address,
        value: u256,
        data: vector<u8>
    ): EvmResult;
    native fun native_evm_create(
        caller: address, value: u256, data: vector<u8>
    ): EvmResult;
}
