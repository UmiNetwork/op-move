script {
    use 0x4200000000000000000000000000000000000019::base_fee_vault;
    use Evm::evm::{abi_decode_params, evm_output, is_result_success};
    use std::string::{bytes, String};

    fun main(owner: &signer) {
        // Test different decoding cases by generating more RLP bytes at https://abi.hashex.org
        let test_bool = abi_decode_params<bool>(x"0000000000000000000000000000000000000000000000000000000000000001");
        assert!(test_bool == true, 0);

        let test_u8 = abi_decode_params<u8>(x"000000000000000000000000000000000000000000000000000000000000002a");
        assert!(test_u8 == 42, 1);

        let test_u16 = abi_decode_params<u16>(x"000000000000000000000000000000000000000000000000000000000000002a");
        assert!(test_u16 == 42, 2);

        let test_u32 = abi_decode_params<u32>(x"000000000000000000000000000000000000000000000000000000000000002a");
        assert!(test_u32 == 42, 3);

        let test_u64 = abi_decode_params<u64>(x"000000000000000000000000000000000000000000000000000000000000002a");
        assert!(test_u64 == 42, 4);

        let test_u128 = abi_decode_params<u128>(x"000000000000000000000000000000000000000000000000000000000000002a");
        assert!(test_u128 == 42, 5);

        let test_u256 = abi_decode_params<u256>(x"000000000000000000000000000000000000000000000000000000000000002a");
        assert!(test_u256 == 42, 6);

        let test_vec_u8 = abi_decode_params<vector<u8>>(x"000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000040102030400000000000000000000000000000000000000000000000000000000");
        assert!(test_vec_u8 == x"01020304", 7);

        let test_str = abi_decode_params<String>(x"0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000568656c6c6f000000000000000000000000000000000000000000000000000000");
        assert!(*bytes(&test_str) == b"hello", 8);

        /* ************************ */
        /* L2 Contract Call Testing */
        /* ************************ */

        // Call to the `version()` has string output "1.4.1"
        let result = base_fee_vault::version(owner);
        assert!(is_result_success(&result), 8);
        let version_string = abi_decode_params<String>(evm_output(&result));
        assert!(*bytes(&version_string) == b"1.4.1", 9);
    }
}
