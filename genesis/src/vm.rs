use {
    crate::framework::CreateMoveVm,
    aptos_gas_schedule::{LATEST_GAS_FEATURE_VERSION, MiscGasParameters, NativeGasParameters},
    aptos_native_interface::SafeNativeBuilder,
    aptos_types::on_chain_config::{Features, TimedFeaturesBuilder},
    aptos_vm::natives::aptos_natives_with_builder,
    move_binary_format::errors::VMError,
    move_vm_runtime::move_vm::MoveVM,
};

pub struct MovedVm;

impl CreateMoveVm for MovedVm {
    fn create_move_vm(&self) -> Result<MoveVM, VMError> {
        let mut builder = SafeNativeBuilder::new(
            LATEST_GAS_FEATURE_VERSION,
            NativeGasParameters::zeros(),
            MiscGasParameters::zeros(),
            TimedFeaturesBuilder::enable_all().build(),
            Features::default(),
        );
        let mut natives = aptos_natives_with_builder(&mut builder);
        moved_evm_ext::append_evm_natives(&mut natives, &builder);
        let vm = MoveVM::new(natives)?;
        Ok(vm)
    }
}
