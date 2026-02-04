module CrossL2Inbox::cross_l2_inbox {
    use aptos_framework::fungible_asset_u256::zero;
    use EthToken::eth_token::get_metadata;
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

    struct CalculateChecksumArgs {
        id: Identifier,
        msg_hash: Evm::evm::SolidityFixedBytes<Evm::evm::U5<Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1>>,
    }

    public fun calculate_checksum(
        id: Identifier,
        msg_hash: Evm::evm::SolidityFixedBytes<Evm::evm::U5<Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1>>,
    ): EvmResult {
        let arg_struct = CalculateChecksumArgs {
            id,
            msg_hash,
        };

        let data = abi_encode_params(
            vector[51, 27, 99, 127],
            arg_struct,
        );

        let result = evm_view(@0x0, @CrossL2Inbox, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct ValidateMessageArgs {
        id: Identifier,
        msg_hash: Evm::evm::SolidityFixedBytes<Evm::evm::U5<Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1>>,
    }

    public fun validate_message(
        caller: &signer,
        id: Identifier,
        msg_hash: Evm::evm::SolidityFixedBytes<Evm::evm::U5<Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1>>,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = ValidateMessageArgs {
            id,
            msg_hash,
        };

        let data = abi_encode_params(
            vector[171, 77, 111, 117],
            arg_struct,
        );

        let result = evm_call(caller, @CrossL2Inbox, _value, data);
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
