module L2ToL2CrossDomainMessenger::l2_to_l2_cross_domain_messenger {
    use aptos_framework::fungible_asset_u256::FungibleAsset;
    
    use Evm::evm::{abi_encode_params, emit_evm_logs, evm_call, evm_view, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;


    public fun cross_domain_message_sender(
    ): EvmResult {
        let data = vector[56, 255, 222, 24];

        let result = evm_view(@0x0, @L2ToL2CrossDomainMessenger, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun cross_domain_message_source(
    ): EvmResult {
        let data = vector[36, 121, 68, 98];

        let result = evm_view(@0x0, @L2ToL2CrossDomainMessenger, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun message_nonce(
    ): EvmResult {
        let data = vector[236, 199, 4, 40];

        let result = evm_view(@0x0, @L2ToL2CrossDomainMessenger, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun message_version(
    ): EvmResult {
        let data = vector[82, 97, 127, 60];

        let result = evm_view(@0x0, @L2ToL2CrossDomainMessenger, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct RelayMessageArgs {
        destination: u256,
        source: u256,
        nonce: u256,
        sender: address,
        target: address,
        message: vector<u8>,
    }

    public fun relay_message(
        caller: &signer,
        destination: u256,
        source: u256,
        nonce: u256,
        sender: address,
        target: address,
        message: vector<u8>,
        _value: FungibleAsset,
    ): EvmResult {
        let arg_struct = RelayMessageArgs {
            destination,
            source,
            nonce,
            sender,
            target,
            message,
        };

        let data = abi_encode_params(
            vector[30, 205, 38, 242],
            arg_struct,
        );

        let result = evm_call(caller, @L2ToL2CrossDomainMessenger, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct SendMessageArgs {
        destination: u256,
        target: address,
        message: vector<u8>,
    }

    public fun send_message(
        caller: &signer,
        destination: u256,
        target: address,
        message: vector<u8>,
        _value: FungibleAsset,
    ): EvmResult {
        let arg_struct = SendMessageArgs {
            destination,
            target,
            message,
        };

        let data = abi_encode_params(
            vector[112, 86, 244, 31],
            arg_struct,
        );

        let result = evm_call(caller, @L2ToL2CrossDomainMessenger, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct SuccessfulMessagesArgs {
        key: Evm::evm::SolidityFixedBytes<Evm::evm::U5<Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1>>,
    }

    public fun successful_messages(
        key: Evm::evm::SolidityFixedBytes<Evm::evm::U5<Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1>>,
    ): EvmResult {
        let arg_struct = SuccessfulMessagesArgs {
            key,
        };

        let data = abi_encode_params(
            vector[177, 177, 178, 9],
            arg_struct,
        );

        let result = evm_view(@0x0, @L2ToL2CrossDomainMessenger, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun version(
    ): EvmResult {
        let data = vector[84, 253, 77, 80];

        let result = evm_view(@0x0, @L2ToL2CrossDomainMessenger, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }
}
