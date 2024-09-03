use {
    super::transactions::DepositedTx,
    crate::{
        genesis::config::{GenesisConfig, CHAIN_ID},
        primitives::ToMoveAddress,
        types::transactions::NormalizedEthTransaction,
    },
    alloy_primitives::U256,
    aptos_vm::move_vm_ext::UserTransactionContext,
    ethers_core::types::H256,
};

pub struct SessionId {
    pub txn_hash: [u8; 32],
    pub script_hash: Option<[u8; 32]>,
    pub chain_id: u8,
    pub user_txn_context: Option<UserTransactionContext>,
}

impl SessionId {
    pub fn new_from_canonical(
        tx: &NormalizedEthTransaction,
        tx_hash: &H256,
        genesis_config: &GenesisConfig,
    ) -> Self {
        let chain_id = u8_chain_id(genesis_config);
        let sender = tx.signer.to_move_address();
        let user_context = UserTransactionContext::new(
            sender,
            Vec::new(),
            sender,
            tx.gas_limit(),
            u64_gas_price(&tx.max_fee_per_gas),
            chain_id,
            None, // TODO
            None,
        );
        Self {
            txn_hash: tx_hash.0,
            // TODO: support script transactions
            script_hash: None,
            chain_id,
            user_txn_context: Some(user_context),
        }
    }

    pub fn new_from_deposited(
        tx: &DepositedTx,
        tx_hash: &H256,
        genesis_config: &GenesisConfig,
    ) -> Self {
        let chain_id = u8_chain_id(genesis_config);
        let sender = tx.from.to_move_address();
        let user_context = UserTransactionContext::new(
            sender,
            Vec::new(),
            sender,
            tx.gas.into_limbs()[0],
            0,
            chain_id,
            None,
            None,
        );
        Self {
            txn_hash: tx_hash.0,
            script_hash: None,
            chain_id,
            user_txn_context: Some(user_context),
        }
    }
}

/// A default session id to use in tests where we don't care about the transaction details.
#[cfg(test)]
impl Default for SessionId {
    fn default() -> Self {
        Self {
            txn_hash: [0; 32],
            script_hash: None,
            chain_id: 1,
            user_txn_context: None,
        }
    }
}

// TODO: Should we make it an invariant that the gas price is always less than u64::MAX?
fn u64_gas_price(u256_gas_price: &U256) -> u64 {
    match u256_gas_price.as_limbs() {
        [value, 0, 0, 0] => *value,
        _ => u64::MAX,
    }
}

/// Ethereum uses U256 (and most projects on Ethereum use u64) for chain id,
/// but Aptos requires u8.
fn u8_chain_id(genesis_config: &GenesisConfig) -> u8 {
    if genesis_config.chain_id == CHAIN_ID {
        1
    } else {
        // TODO: Should have some generic fallback algorithm here
        // (e.g. just take the least significant byte) and a feature
        // flag to enable it. This would allow people to set their own
        // custom chain ids. This is not launch-critical since for now
        // we are only picking chain ids internally.
        panic!("Unknown chain id: {}", genesis_config.chain_id);
    }
}
