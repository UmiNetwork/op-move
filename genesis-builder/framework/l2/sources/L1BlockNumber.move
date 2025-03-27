module L1BlockNumber::l1_block_number {
    use aptos_framework::fungible_asset_u256::zero;
    use EthToken::eth_token::get_metadata;
    use Evm::evm::{emit_evm_logs, evm_call, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;


    public fun get_l1_block_number(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[185, 179, 239, 233];

        let result = evm_call(caller, @L1BlockNumber, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun version(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[84, 253, 77, 80];

        let result = evm_call(caller, @L1BlockNumber, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }
}
