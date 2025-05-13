module SequencerFeeVault::sequencer_fee_vault {
    use aptos_framework::fungible_asset_u256::zero;
    use EthToken::eth_token::get_metadata;
    use Evm::evm::{emit_evm_logs, evm_call, evm_view, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;


    public fun min_withdrawal_amount(
    ): EvmResult {
        let data = vector[211, 229, 121, 43];

        let result = evm_view(@0x0, @SequencerFeeVault, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun recipient(
    ): EvmResult {
        let data = vector[13, 144, 25, 225];

        let result = evm_view(@0x0, @SequencerFeeVault, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun withdrawal_network(
    ): EvmResult {
        let data = vector[208, 225, 47, 144];

        let result = evm_view(@0x0, @SequencerFeeVault, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun l1_fee_wallet(
    ): EvmResult {
        let data = vector[212, 255, 146, 24];

        let result = evm_view(@0x0, @SequencerFeeVault, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun total_processed(
    ): EvmResult {
        let data = vector[132, 65, 29, 101];

        let result = evm_view(@0x0, @SequencerFeeVault, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun version(
    ): EvmResult {
        let data = vector[84, 253, 77, 80];

        let result = evm_view(@0x0, @SequencerFeeVault, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun withdraw(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[60, 207, 214, 11];

        let result = evm_call(caller, @SequencerFeeVault, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }
}
