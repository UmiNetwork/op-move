module 0x8fd379246834eac74b8419ffda202cf8051f7a03::erc20_interface {
    use 0x1::evm::{abi_encode_params, entry_evm_call};

    struct Erc20TransferArgs {
        to: address,
        amount: u256,
    }

    public entry fun erc20_transfer(
        token_address: address,
        from: &signer,
        to: address,
        amount: u256,
    ) {
        let args = abi_encode_params(
            // ERC-20 transfer selector: 0xa9059cbb
            vector[0xa9, 0x05, 0x9c, 0xbb],
            Erc20TransferArgs {
                to,
                amount,
            }
        );
        entry_evm_call(
            from,
            token_address,
            args,
        );
    }
}
