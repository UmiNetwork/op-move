module 0x8fd379246834eac74b8419ffda202cf8051f7a03::solidity_fixed_bytes {
    use 0x1::evm::{abi_encode_params, as_fixed_bytes};

    public entry fun encode_fixed_bytes(
        input: vector<u8>,
    ) {
        let args = as_fixed_bytes(input);
        abi_encode_params(vector[], args);
    }
}
