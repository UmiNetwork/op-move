module Erc20::erc20 {
    use aptos_framework::fungible_asset_u256::zero;
    use EthToken::eth_token::get_metadata;
    use Evm::evm::{abi_encode_params, emit_evm_logs, evm_call, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;

    struct BalanceOfArgs {
        account: address,
    }

    public fun balance_of(
        caller: &signer,
        token: address,
        account: address,
    ): EvmResult {
        let args = BalanceOfArgs {
            account,
        };

        let data = abi_encode_params(
            vector[0x70, 0xa0, 0x82, 0x31],
            args,
        );

        let value = zero(get_metadata());
        let result = evm_call(caller, token, value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        result
    }

    struct AllowanceArgs {
        owner: address,
        spender: address,
    }

    public fun allowance(
        caller: &signer,
        token: address,
        owner: address,
        spender: address,
    ): EvmResult {
        let args = AllowanceArgs {
            owner,
            spender,
        };

        let data = abi_encode_params(
            vector[0xdd, 0x62, 0xed, 0x3e],
            args,
        );

        let value = zero(get_metadata());
        let result = evm_call(caller, token, value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        result
    }    

    struct ApproveArgs {
        spender: address,
        value: u256,
    }

    public fun approve(
        caller: &signer,
        token: address,
        spender: address,
        value: u256,
    ): EvmResult {
        let args = ApproveArgs {
            spender,
            value,
        };

        let data = abi_encode_params(
            vector[0x09, 0x5e, 0xa7, 0xb3],
            args,
        );

        let value = zero(get_metadata());
        let result = evm_call(caller, token, value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    /// Same as `approve`, but allowed to be called as an entry function.
    public entry fun approve_entry(
        caller: &signer,
        token: address,
        spender: address,
        value: u256,
    ) {
        approve(caller, token, spender, value);
    }

    // TODO: transfer, transferFrom, etc.
}
