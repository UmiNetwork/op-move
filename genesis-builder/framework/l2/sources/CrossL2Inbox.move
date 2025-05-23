module CrossL2Inbox::cross_l2_inbox {
    use aptos_framework::fungible_asset_u256::FungibleAsset;
    
    use Evm::evm::{abi_encode_params, emit_evm_logs, evm_call, evm_view, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;

    struct Identifier {
        origin: address,
        block_number: u256,
        log_index: u256,
        timestamp: u256,
        chain_id: u256,
    }


    public fun block_number(
    ): EvmResult {
        let data = vector[87, 232, 113, 231];

        let result = evm_view(@0x0, @CrossL2Inbox, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun chain_id(
    ): EvmResult {
        let data = vector[154, 138, 5, 146];

        let result = evm_view(@0x0, @CrossL2Inbox, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct ExecuteMessageArgs {
        id: Identifier,
        target: address,
        message: vector<u8>,
    }

    public fun execute_message(
        caller: &signer,
        id: Identifier,
        target: address,
        message: vector<u8>,
        _value: FungibleAsset,
    ): EvmResult {
        let arg_struct = ExecuteMessageArgs {
            id,
            target,
            message,
        };

        let data = abi_encode_params(
            vector[89, 132, 197, 62],
            arg_struct,
        );

        let result = evm_call(caller, @CrossL2Inbox, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun log_index(
    ): EvmResult {
        let data = vector[218, 153, 247, 41];

        let result = evm_view(@0x0, @CrossL2Inbox, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun origin(
    ): EvmResult {
        let data = vector[147, 139, 95, 50];

        let result = evm_view(@0x0, @CrossL2Inbox, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun timestamp(
    ): EvmResult {
        let data = vector[184, 7, 119, 234];

        let result = evm_view(@0x0, @CrossL2Inbox, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun version(
    ): EvmResult {
        let data = vector[84, 253, 77, 80];

        let result = evm_view(@0x0, @CrossL2Inbox, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }
}
