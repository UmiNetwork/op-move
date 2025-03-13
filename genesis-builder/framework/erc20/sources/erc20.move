module Erc20::erc20 {
    use aptos_framework::fungible_asset_u256::zero;
    use EthToken::eth_token::get_metadata;
    use Evm::evm::{abi_encode_params, evm_call, is_result_success, EvmResult};
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

    // TODO: transfer, approve, transferFrom, etc.
}
