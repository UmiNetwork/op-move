script {
    use 0x4200000000000000000000000000000000000019::base_fee_vault;
    use Evm::evm::{abi_decode_params, is_result_success};

    fun main(owner: &signer) {
        // Test different cases by generating more RLP bytes on https://abi.hashex.org
        let test_bool = abi_decode_params<bool>(x"0000000000000000000000000000000000000000000000000000000000000001");
        assert!(test_bool == true, 0);

        let test_u8 = abi_decode_params<u8>(x"000000000000000000000000000000000000000000000000000000000000002a");
        assert!(test_u8 == 42, 1);

        let test_u16 = abi_decode_params<u16>(x"000000000000000000000000000000000000000000000000000000000000002a");
        assert!(test_u16 == 42, 1);

        let test_u32 = abi_decode_params<u32>(x"000000000000000000000000000000000000000000000000000000000000002a");
        assert!(test_u32 == 42, 1);

        let test_u64 = abi_decode_params<u64>(x"000000000000000000000000000000000000000000000000000000000000002a");
        assert!(test_u64 == 42, 1);

        let test_u128 = abi_decode_params<u128>(x"000000000000000000000000000000000000000000000000000000000000002a");
        assert!(test_u128 == 42, 1);

        let test_u256 = abi_decode_params<u256>(x"000000000000000000000000000000000000000000000000000000000000002a");
        assert!(test_u256 == 42, 1);

        let result = base_fee_vault::version(owner);
        assert!(is_result_success(&result), 0);
    }
}
