module Erc20::erc20 {
    use aptos_framework::fungible_asset_u256::zero;
    use EthToken::eth_token::get_metadata;
    use Evm::evm::{abi_encode_params, emit_evm_logs, evm_call, evm_view, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;

    const ZERO_ADDRESS: address = @0x0;

    const BALANCE_OF_SELECTOR: vector<u8> = vector[0x70, 0xa0, 0x82, 0x31]; // balanceOf(address)
    const TRANSFER_SELECTOR: vector<u8> = vector[0xa9, 0x05, 0x9c, 0xbb]; // transfer(address,uint256)
    const TRANSFER_FROM_SELECTOR: vector<u8> = vector[0x23, 0xb8, 0x72, 0xdd]; // transferFrom(address,address,uint256)
    const APPROVE_SELECTOR: vector<u8> = vector[0x09, 0x5e, 0xa7, 0xb3]; // approve(address,uint256)
    const ALLOWANCE_SELECTOR: vector<u8> = vector[0xdd, 0x62, 0xed, 0x3e]; // allowance(address,address)
    const TOTAL_SUPPLY_SELECTOR: vector<u8> = vector[0x18, 0x16, 0x0d, 0xdd]; // totalSupply()
    const NAME_SELECTOR: vector<u8> = vector[0x06, 0xfd, 0xde, 0x03]; // name()
    const SYMBOL_SELECTOR: vector<u8> = vector[0x95, 0xd8, 0x9b, 0x41]; // symbol()
    const DECIMALS_SELECTOR: vector<u8> = vector[0x31, 0x3c, 0xe5, 0x67]; // decimals()

    struct BalanceOfArgs {
        account: address,
    }

    public fun balance_of(
        token: address,
        account: address,
    ): EvmResult {
        let args = BalanceOfArgs {
            account,
        };

        let data = abi_encode_params(
            BALANCE_OF_SELECTOR,
            args,
        );

        let value = 0;
        let result = evm_view(ZERO_ADDRESS, token, value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        result
    }

    struct AllowanceArgs {
        owner: address,
        spender: address,
    }

    public fun allowance(
        token: address,
        owner: address,
        spender: address,
    ): EvmResult {
        let args = AllowanceArgs {
            owner,
            spender,
        };

        let data = abi_encode_params(
            ALLOWANCE_SELECTOR,
            args,
        );

        let value = 0;
        let result = evm_view(ZERO_ADDRESS, token, value, data);
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
            APPROVE_SELECTOR,
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

    struct TransferArgs {
        recipient: address,
        value: u256,
    }

    public fun transfer(
        caller: &signer,
        token: address,
        recipient: address,
        value: u256,
    ): EvmResult {
        let args = TransferArgs {
            recipient,
            value,
        };

        let data = abi_encode_params(
            TRANSFER_SELECTOR,
            args,
        );

        let value = zero(get_metadata());
        let result = evm_call(caller, token, value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }    

    /// Same as `transfer`, but allowed to be called as an entry function.
    public entry fun transfer_entry(
        caller: &signer,
        token: address,
        recipient: address,
        value: u256,
    ) {
        transfer(caller, token, recipient, value);
    }

    struct TransferFromArgs {
        sender: address,
        recipient: address,
        value: u256,
    }

    public fun transfer_from(
        caller: &signer,
        token: address,
        sender: address,
        recipient: address,
        value: u256,
    ): EvmResult {
        let args = TransferFromArgs {
            sender,
            recipient,
            value,
        };

        let data = abi_encode_params(
            TRANSFER_FROM_SELECTOR,
            args,
        );

        let value = zero(get_metadata());
        let result = evm_call(caller, token, value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }    
    
    public fun total_supply(
        token: address,
    ): EvmResult {
        let data = TOTAL_SUPPLY_SELECTOR;

        let value = 0;
        let result = evm_view(ZERO_ADDRESS, token, value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        result
    }

    public fun name(
        token: address,
    ): EvmResult {
        let data = NAME_SELECTOR;

        let value = 0;
        let result = evm_view(ZERO_ADDRESS, token, value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        result
    }

    public fun symbol(
        token: address,
    ): EvmResult {
        let data = SYMBOL_SELECTOR;

        let value = 0;
        let result = evm_view(ZERO_ADDRESS, token, value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        result
    }

    public fun decimals(
        token: address,
    ): EvmResult {
        let data = DECIMALS_SELECTOR;

        let value = 0;
        let result = evm_view(ZERO_ADDRESS, token, value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        result
    }
}
