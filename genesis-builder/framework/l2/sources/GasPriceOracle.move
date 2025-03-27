module GasPriceOracle::gas_price_oracle {
    use aptos_framework::fungible_asset_u256::zero;
    use EthToken::eth_token::get_metadata;
    use Evm::evm::{abi_encode_params, emit_evm_logs, evm_call, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;


    public fun decimals(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[46, 15, 38, 37];

        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun base_fee(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[110, 242, 92, 58];

        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun base_fee_scalar(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[197, 152, 89, 24];

        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun blob_base_fee(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[248, 32, 97, 64];

        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun blob_base_fee_scalar(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[104, 213, 220, 166];

        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun gas_price(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[254, 23, 59, 151];

        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct GetL1FeeArgs {
        data: vector<u8>,
    }

    public fun get_l1_fee(
        caller: &signer,
        data: vector<u8>,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = GetL1FeeArgs {
            data,
        };

        let data = abi_encode_params(
            vector[73, 148, 142, 14],
            arg_struct,
        );

        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct GetL1FeeUpperBoundArgs {
        unsigned_tx_size: u256,
    }

    public fun get_l1_fee_upper_bound(
        caller: &signer,
        unsigned_tx_size: u256,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = GetL1FeeUpperBoundArgs {
            unsigned_tx_size,
        };

        let data = abi_encode_params(
            vector[241, 199, 165, 139],
            arg_struct,
        );

        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct GetL1GasUsedArgs {
        data: vector<u8>,
    }

    public fun get_l1_gas_used(
        caller: &signer,
        data: vector<u8>,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = GetL1GasUsedArgs {
            data,
        };

        let data = abi_encode_params(
            vector[222, 38, 196, 161],
            arg_struct,
        );

        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun is_ecotone(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[78, 246, 226, 36];

        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun is_fjord(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[150, 14, 58, 35];

        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun l1_base_fee(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[81, 155, 75, 211];

        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun overhead(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[12, 24, 193, 98];

        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun scalar(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[244, 94, 101, 216];

        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun set_ecotone(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[34, 185, 10, 179];

        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun set_fjord(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[142, 152, 177, 6];

        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun version(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[84, 253, 77, 80];

        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }
}
