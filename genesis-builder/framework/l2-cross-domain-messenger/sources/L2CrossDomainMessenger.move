module L2CrossDomainMessenger::l2_cross_domain_messenger {
    use aptos_framework::fungible_asset_u256::FungibleAsset;
    use Evm::evm::{abi_encode_params, emit_evm_logs, evm_call, is_result_success, EvmResult};
    use std::error;

    /// The EVM execution failed for some reason.
    const ENOT_SUCCESS: u64 = 1;

    struct SendMessageArgs {
        target: address,
        message: vector<u8>,
        min_gas_limit: u32,
    }

    // See documentation on optimism:
    // https://github.com/ethereum-optimism/optimism/blob/62c7f3b05a70027b30054d4c8974f44000606fb7/packages/contracts-bedrock/contracts/universal/CrossDomainMessenger.sol#L249-L289
    public fun send_message(
        caller: &signer,
        target: address,
        message: vector<u8>,
        min_gas_limit: u32,
        value: FungibleAsset,
    ): EvmResult {
        let arg_struct = SendMessageArgs {
            target, message, min_gas_limit,
        };
        let data = abi_encode_params(
            // Selector for `sendMessage(address,bytes,uint32)` is `0x3dbb202b`.
            vector[0x3d, 0xbb, 0x20, 0x2b],
            arg_struct,
        );
        let result = evm_call(
            caller,
            @L2CrossDomainMessenger,
            value,
            data,
        );

        // The EVM execution must succeed.
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));

        // Include the EVM logs in MoveVM output
        emit_evm_logs(&result);

        result
    }
}
