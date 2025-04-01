macro_rules! impl_shared {
    () => {
        type BlockHash = moved_blockchain::block::MovedBlockHash;
        type BaseTokenAccounts = moved_execution::MovedBaseTokenAccounts;
        type BaseGasFee = moved_blockchain::block::Eip1559GasFee;
        type CreateL1GasFee = moved_execution::CreateEcotoneL1GasFee;
        type CreateL2GasFee = moved_execution::CreateMovedL2GasFee;

        fn block_hash() -> Self::BlockHash {
            moved_blockchain::block::MovedBlockHash
        }

        fn base_gas_fee() -> Self::BaseGasFee {
            moved_blockchain::block::Eip1559GasFee::new(
                crate::EIP1559_ELASTICITY_MULTIPLIER,
                crate::EIP1559_BASE_FEE_MAX_CHANGE_DENOMINATOR,
            )
        }

        fn create_l1_gas_fee() -> Self::CreateL1GasFee {
            moved_execution::CreateEcotoneL1GasFee
        }

        fn create_l2_gas_fee() -> Self::CreateL2GasFee {
            moved_execution::CreateMovedL2GasFee
        }

        fn base_token_accounts(genesis_config: &GenesisConfig) -> Self::BaseTokenAccounts {
            moved_execution::MovedBaseTokenAccounts::new(genesis_config.treasury)
        }
    };
}

pub(crate) use impl_shared;
