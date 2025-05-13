module OptimismMintableERC721Factory::optimism_mintable_erc721_factory {
    use aptos_framework::fungible_asset_u256::zero;
    use EthToken::eth_token::get_metadata;
    use Evm::evm::{abi_encode_params, emit_evm_logs, evm_call, evm_view, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;


    public fun bridge(
    ): EvmResult {
        let data = vector[238, 154, 49, 162];

        let result = evm_view(@0x0, @OptimismMintableERC721Factory, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun remote_chain_id(
    ): EvmResult {
        let data = vector[125, 29, 12, 91];

        let result = evm_view(@0x0, @OptimismMintableERC721Factory, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct CreateOptimismMintableErc721Args {
        remote_token: address,
        name: vector<u8>,
        symbol: vector<u8>,
    }

    public fun create_optimism_mintable_erc721(
        caller: &signer,
        remote_token: address,
        name: vector<u8>,
        symbol: vector<u8>,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = CreateOptimismMintableErc721Args {
            remote_token,
            name,
            symbol,
        };

        let data = abi_encode_params(
            vector[217, 125, 246, 82],
            arg_struct,
        );

        let result = evm_call(caller, @OptimismMintableERC721Factory, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct IsOptimismMintableErc721Args {
        key: address,
    }

    public fun is_optimism_mintable_erc721(
        key: address,
    ): EvmResult {
        let arg_struct = IsOptimismMintableErc721Args {
            key,
        };

        let data = abi_encode_params(
            vector[85, 114, 172, 174],
            arg_struct,
        );

        let result = evm_view(@0x0, @OptimismMintableERC721Factory, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun version(
    ): EvmResult {
        let data = vector[84, 253, 77, 80];

        let result = evm_view(@0x0, @OptimismMintableERC721Factory, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }
}
