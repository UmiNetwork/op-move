use {
    alloy::{
        consensus::Sealed,
        primitives::{Address, Bytes, TxKind},
    },
    op_alloy::consensus::TxDeposit,
    std::sync::{LazyLock, Mutex},
    umi_execution::transaction::NormalizedExtendedTxEnvelope,
    umi_shared::primitives::{B256, U256},
};

static FAUCT_TXS: LazyLock<Mutex<Vec<(Address, u128)>>> = LazyLock::new(|| Mutex::new(Vec::new()));

pub fn get_requests() -> Vec<(Address, u128)> {
    let mut result = Vec::new();

    let Ok(mut queue) = FAUCT_TXS.lock() else {
        return result;
    };

    std::mem::swap(&mut result, &mut queue);

    result
}

pub fn new_tx(parent_hash: B256, address: Address, amount: u128) -> NormalizedExtendedTxEnvelope {
    let mut source_pre_image = vec![0; 32 + 20 + 32];
    source_pre_image[0..32].copy_from_slice(parent_hash.as_ref());
    source_pre_image[32..52].copy_from_slice(address.as_ref());
    source_pre_image[52..84].copy_from_slice(&amount.to_be_bytes());

    let source_hash = alloy::primitives::keccak256(&source_pre_image);
    let deposit_tx = TxDeposit {
        source_hash,
        from: address,
        to: TxKind::Call(address),
        mint: amount,
        value: U256::from(amount),
        gas_limit: 100_000,
        is_system_transaction: false,
        input: Bytes::new(),
    };
    let sealed = Sealed::new(deposit_tx);
    NormalizedExtendedTxEnvelope::DepositedTx(sealed)
}

pub fn push_request(address: Address, amount: u128) {
    let Ok(mut queue) = FAUCT_TXS.lock() else {
        return;
    };

    queue.push((address, amount));
}
