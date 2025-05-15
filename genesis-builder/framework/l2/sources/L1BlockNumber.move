module L1BlockNumber::l1_block_number {
    
    use Evm::evm::{emit_evm_logs, evm_view, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;


    public fun get_l1_block_number(
    ): EvmResult {
        let data = vector[185, 179, 239, 233];

        let result = evm_view(@0x0, @L1BlockNumber, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun version(
    ): EvmResult {
        let data = vector[84, 253, 77, 80];

        let result = evm_view(@0x0, @L1BlockNumber, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }
}
