module 0x8fd379246834eac74b8419ffda202cf8051f7a03::solidity_fixed_bytes {
    use 0x1::evm::{abi_encode_params, as_fixed_bytes, as_fixed_bytes_1,  as_fixed_bytes_16, as_fixed_bytes_32};

    public entry fun encode_fixed_bytes1(
        input: vector<u8>,
    ) {
        let args = as_fixed_bytes_1(input);
        abi_encode_params(vector[], args);
    }

    public entry fun encode_fixed_bytes16(
        input: vector<u8>,
    ) {
        let args = as_fixed_bytes_16(input);
        abi_encode_params(vector[], args);
    }

    public entry fun encode_fixed_bytes32(
        input: vector<u8>,
    ) {
        let args = as_fixed_bytes_32(input);
        abi_encode_params(vector[], args);
    }

    public entry fun encode_fixed_bytes_bad_args(
        input: vector<u8>,
    ) {
        let args = as_fixed_bytes<u8,u8,u8,u8,u8>(input);
        abi_encode_params(vector[], args);
    }
}
