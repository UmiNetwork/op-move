//! Module defining types related to the state of op-move.
//! E.g. known block hashes, current head of the chain, etc.
//! Also defines the messages the State Actor (which manages the state)
//! accepts.

use {
    super::queries::ProofResponse,
    crate::{
        block::{ExtendedBlock, Header},
        state_actor::NewPayloadIdInput,
        transaction::ExtendedTransaction,
        types::transactions::NormalizedExtendedTxEnvelope,
    },
    alloy::{
        consensus::transaction::TxEnvelope,
        eips::{eip2718::Encodable2718, BlockId, BlockNumberOrTag},
        primitives::Bloom,
        rpc::types::{BlockTransactions, FeeHistory, TransactionRequest, Withdrawals},
    },
    moved_shared::primitives::{Address, Bytes, ToU64, B2048, B256, U256, U64},
    op_alloy::{
        consensus::{OpReceiptEnvelope, OpTxEnvelope, TxDeposit},
        rpc_types::L1BlockInfo,
    },
    tokio::sync::oneshot,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Payload {
    pub timestamp: U64,
    pub prev_randao: B256,
    pub suggested_fee_recipient: Address,
    pub withdrawals: Vec<Withdrawal>,
    pub parent_beacon_block_root: B256,
    pub transactions: Vec<Bytes>,
    pub gas_limit: U64,
}

pub type Withdrawal = alloy::rpc::types::Withdrawal;

pub type PayloadId = U64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadResponse {
    pub execution_payload: ExecutionPayload,
    pub block_value: U256,
    pub blobs_bundle: BlobsBundle,
    pub should_override_builder: bool,
    pub parent_beacon_block_root: B256,
}

impl PayloadResponse {
    pub fn from_block(value: ExtendedBlock) -> Self {
        Self {
            parent_beacon_block_root: value
                .block
                .header
                .parent_beacon_block_root
                .unwrap_or_default(),
            block_value: value.value,
            execution_payload: ExecutionPayload::from_block(value),
            blobs_bundle: Default::default(),
            should_override_builder: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecutionPayload {
    pub parent_hash: B256,
    pub fee_recipient: Address,
    pub state_root: B256,
    pub receipts_root: B256,
    pub logs_bloom: B2048,
    pub prev_randao: B256,
    pub block_number: U64,
    pub gas_limit: U64,
    pub gas_used: U64,
    pub timestamp: U64,
    pub extra_data: Bytes,
    pub base_fee_per_gas: U256,
    pub block_hash: B256,
    pub transactions: Vec<Bytes>,
    pub withdrawals: Vec<Withdrawal>,
    pub blob_gas_used: U64,
    pub excess_blob_gas: U64,
}

impl ExecutionPayload {
    pub fn from_block(value: ExtendedBlock) -> Self {
        let transactions = value
            .block
            .transactions
            .into_iter()
            .map(|tx| {
                let capacity = tx.eip2718_encoded_length();
                let mut bytes = Vec::with_capacity(capacity);
                tx.encode_2718(&mut bytes);
                bytes.into()
            })
            .collect();

        Self {
            block_hash: value.hash,
            parent_hash: value.block.header.parent_hash,
            fee_recipient: value.block.header.beneficiary,
            state_root: value.block.header.state_root,
            receipts_root: value.block.header.receipts_root,
            logs_bloom: value.block.header.logs_bloom.0,
            prev_randao: value.block.header.mix_hash,
            block_number: U64::from(value.block.header.number),
            gas_limit: U64::from(value.block.header.gas_limit),
            gas_used: U64::from(value.block.header.gas_used),
            timestamp: U64::from(value.block.header.timestamp),
            extra_data: value.block.header.extra_data,
            base_fee_per_gas: U256::from(value.block.header.base_fee_per_gas.unwrap_or_default()),
            transactions,
            withdrawals: Vec::new(), // TODO: withdrawals
            blob_gas_used: U64::from(value.block.header.blob_gas_used.unwrap_or_default()),
            excess_blob_gas: U64::from(value.block.header.excess_blob_gas.unwrap_or_default()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BlobsBundle {
    pub commitments: Vec<Bytes>,
    pub proofs: Vec<Bytes>,
    pub blobs: Vec<Bytes>,
}

#[derive(Debug)]
pub enum StateMessage {
    Command(Command),
    Query(Query),
}

#[derive(Debug)]
pub enum Command {
    UpdateHead {
        block_hash: B256,
    },
    StartBlockBuild {
        payload_attributes: Payload,
        response_channel: oneshot::Sender<PayloadId>,
    },
    GetPayload {
        id: PayloadId,
        response_channel: oneshot::Sender<Option<PayloadResponse>>,
    },
    GetPayloadByBlockHash {
        block_hash: B256,
        response_channel: oneshot::Sender<Option<PayloadResponse>>,
    },
    AddTransaction {
        tx: TxEnvelope,
    },
    GenesisUpdate {
        block: ExtendedBlock,
    },
}

impl From<Command> for StateMessage {
    fn from(value: Command) -> Self {
        Self::Command(value)
    }
}

#[derive(Debug)]
pub enum Query {
    ChainId {
        response_channel: oneshot::Sender<u64>,
    },
    BalanceByHeight {
        address: Address,
        height: BlockNumberOrTag,
        response_channel: oneshot::Sender<Option<U256>>,
    },
    NonceByHeight {
        address: Address,
        height: BlockNumberOrTag,
        response_channel: oneshot::Sender<Option<u64>>,
    },
    BlockByHash {
        hash: B256,
        include_transactions: bool,
        response_channel: oneshot::Sender<Option<BlockResponse>>,
    },
    BlockByHeight {
        height: BlockNumberOrTag,
        include_transactions: bool,
        response_channel: oneshot::Sender<Option<BlockResponse>>,
    },
    BlockNumber {
        response_channel: oneshot::Sender<u64>,
    },
    FeeHistory {
        block_count: u64,
        block_number: BlockNumberOrTag,
        reward_percentiles: Option<Vec<f64>>,
        response_channel: oneshot::Sender<FeeHistory>,
    },
    EstimateGas {
        transaction: TransactionRequest,
        block_number: BlockNumberOrTag,
        response_channel: oneshot::Sender<crate::Result<u64>>,
    },
    Call {
        transaction: TransactionRequest,
        block_number: BlockNumberOrTag,
        response_channel: oneshot::Sender<crate::Result<Vec<u8>>>,
    },
    TransactionReceipt {
        tx_hash: B256,
        response_channel: oneshot::Sender<Option<TransactionReceipt>>,
    },
    TransactionByHash {
        tx_hash: B256,
        response_channel: oneshot::Sender<Option<TransactionResponse>>,
    },
    GetProof {
        address: Address,
        storage_slots: Vec<U256>,
        height: BlockId,
        response_channel: oneshot::Sender<Option<ProofResponse>>,
    },
}

impl From<Query> for StateMessage {
    fn from(value: Query) -> Self {
        Self::Query(value)
    }
}

pub type RpcBlock = alloy::rpc::types::Block<op_alloy::rpc_types::Transaction>;
pub type RpcTransaction = op_alloy::rpc_types::Transaction;

#[derive(Debug)]
pub struct BlockResponse(pub RpcBlock);

impl BlockResponse {
    fn new(value: RpcBlock) -> Self {
        Self(value)
    }

    pub fn from_block_with_transaction_hashes(value: ExtendedBlock) -> Self {
        Self::new(RpcBlock {
            transactions: BlockTransactions::Hashes(
                value
                    .block
                    .transactions
                    .iter()
                    .map(|tx| match tx {
                        OpTxEnvelope::Legacy(tx) => *tx.hash(),
                        OpTxEnvelope::Eip1559(tx) => *tx.hash(),
                        OpTxEnvelope::Eip2930(tx) => *tx.hash(),
                        OpTxEnvelope::Eip7702(tx) => *tx.hash(),
                        OpTxEnvelope::Deposit(tx) => tx.hash(),
                        _ => unreachable!("Tx type not supported"),
                    })
                    .collect(),
            ),
            header: alloy::rpc::types::Header {
                hash: value.hash,
                inner: value.block.header,
                // TODO: review fields below
                total_difficulty: None,
                size: None,
            },
            uncles: Vec::new(),
            withdrawals: Some(Withdrawals(Vec::new())),
        })
    }

    pub fn from_block_with_transactions(value: ExtendedBlock) -> Self {
        Self::new(RpcBlock {
            transactions: BlockTransactions::Full(
                value
                    .block
                    .transactions
                    .into_iter()
                    .enumerate()
                    .map(|(i, inner)| {
                        let tx = alloy::rpc::types::Transaction {
                            block_hash: Some(value.hash),
                            block_number: Some(value.block.header.number),
                            transaction_index: Some(i as u64),
                            // TODO: Gassing it up requires either modifying supported variants of
                            // `OpTxEnvelope` or storing a different type in block transactions
                            // altogether
                            effective_gas_price: None,
                            from: compute_from(&inner)
                                .expect("Block transactions should contain valid signature"),
                            inner,
                        };
                        let version_nonce = get_deposit_nonce(&tx.inner);
                        op_alloy::rpc_types::Transaction {
                            inner: tx,
                            deposit_nonce: version_nonce.as_ref().map(|v| v.nonce),
                            deposit_receipt_version: version_nonce.map(|v| v.version),
                        }
                    })
                    .collect(),
            ),
            header: alloy::rpc::types::Header {
                hash: value.hash,
                inner: value.block.header,
                // TODO: review fields below
                total_difficulty: None,
                size: None,
            },
            uncles: Vec::new(),
            withdrawals: Some(Withdrawals(Vec::new())),
        })
    }
}

// Nonce and version for messages of CrossDomainMessenger L2 contract.
struct VersionedNonce {
    version: u64,
    nonce: u64,
}

fn get_deposit_nonce(tx: &OpTxEnvelope) -> Option<VersionedNonce> {
    if let OpTxEnvelope::Deposit(tx) = tx {
        inner_get_deposit_nonce(tx)
    } else {
        None
    }
}

fn inner_get_deposit_nonce(tx: &TxDeposit) -> Option<VersionedNonce> {
    use alloy::sol_types::SolType;

    // Function selector for `relayMessage`.
    // See optimism/packages/contracts-bedrock/src/universal/CrossDomainMessenger.sol
    const RELAY_MESSAGE_SELECTOR: [u8; 4] = [0xd7, 0x64, 0xad, 0x0b];

    // The upper 16 bits are for the version, the rest are for the nonce.
    const NONCE_MASK: U256 = U256::from_be_bytes(alloy::hex!(
        "0000ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    ));

    alloy::sol! {
        struct RelayMessageArgs {
            uint256 nonce;
            address sender;
            address target;
            uint256 value;
            uint256 min_gas_limit;
            bytes message;
        }
    }

    if !tx.input.starts_with(&RELAY_MESSAGE_SELECTOR) {
        return None;
    }

    let args = RelayMessageArgs::abi_decode_params(&tx.input[4..], true).ok()?;

    // See optimism/packages/contracts-bedrock/src/libraries/Encoding.sol
    let encoded_versioned_nonce = args.nonce;
    let version = encoded_versioned_nonce.checked_shr(240)?.saturating_to();
    let nonce = (encoded_versioned_nonce & NONCE_MASK).saturating_to();
    Some(VersionedNonce { version, nonce })
}

fn compute_from(tx: &OpTxEnvelope) -> Result<Address, alloy::primitives::SignatureError> {
    match tx {
        OpTxEnvelope::Legacy(tx) => tx.recover_signer(),
        OpTxEnvelope::Eip1559(tx) => tx.recover_signer(),
        OpTxEnvelope::Eip2930(tx) => tx.recover_signer(),
        OpTxEnvelope::Eip7702(tx) => tx.recover_signer(),
        OpTxEnvelope::Deposit(tx) => Ok(tx.from),
        _ => unreachable!("Tx type not supported"),
    }
}

pub type TransactionResponse = op_alloy::rpc_types::Transaction;

impl From<ExtendedTransaction> for TransactionResponse {
    fn from(value: ExtendedTransaction) -> Self {
        let (deposit_nonce, deposit_receipt_version) = get_deposit_nonce(value.inner())
            .map(|nonce| (Some(nonce.nonce), Some(nonce.version)))
            .unwrap_or((None, None));

        Self {
            inner: alloy::rpc::types::eth::Transaction {
                from: compute_from(value.inner())
                    .expect("Block transactions should contain valid signature"),
                inner: value.inner,
                block_hash: Some(value.block_hash),
                block_number: Some(value.block_number),
                transaction_index: Some(value.transaction_index),
                effective_gas_price: Some(value.effective_gas_price),
            },
            deposit_nonce,
            deposit_receipt_version,
        }
    }
}

pub type TransactionReceipt = op_alloy::rpc_types::OpTransactionReceipt;

#[derive(Debug)]
pub struct ExecutionOutcome {
    pub receipts_root: B256,
    pub state_root: B256,
    pub logs_bloom: B2048,
    pub gas_used: U64,
    pub total_tip: U256,
}

#[derive(Debug, Clone)]
pub struct TransactionWithReceipt {
    pub tx_hash: B256,
    pub tx: OpTxEnvelope,
    pub tx_index: u64,
    pub normalized_tx: NormalizedExtendedTxEnvelope,
    pub receipt: OpReceiptEnvelope,
    pub l1_block_info: Option<L1BlockInfo>,
    pub gas_used: u64,
    pub l2_gas_price: U256,
    /// If the transaction deployed a new contract, gives the address.
    ///
    /// In Move contracts are identified by AccountAddress + ModuleID,
    /// so this field cannot capture all the detail of a new deployment,
    /// however we cannot extend the field because it is here for Ethereum
    /// compatibility. As a compromise, we will put the AccountAddress here
    /// and the user would need to look up the ModuleID by inspecting the
    /// transaction object itself.
    pub contract_address: Option<Address>,
    /// Counts the number of logs that exist in transactions appearing earlier
    /// in the same block.
    ///
    /// This allows computing the log index for each log in this transaction.
    pub logs_offset: u64,
}

pub(crate) trait WithExecutionOutcome {
    fn with_execution_outcome(self, outcome: ExecutionOutcome) -> Self;
}

impl WithExecutionOutcome for Header {
    fn with_execution_outcome(self, outcome: ExecutionOutcome) -> Self {
        Self {
            state_root: outcome.state_root,
            receipts_root: outcome.receipts_root,
            logs_bloom: Bloom::new(outcome.logs_bloom.0),
            gas_used: outcome.gas_used.to_u64(),
            ..self
        }
    }
}

pub(crate) trait ToPayloadIdInput<'a> {
    fn to_payload_id_input(&'a self, head: &'a B256) -> NewPayloadIdInput<'a>;
}

impl<'a> ToPayloadIdInput<'a> for Payload {
    fn to_payload_id_input(&'a self, head: &'a B256) -> NewPayloadIdInput<'a> {
        NewPayloadIdInput::new_v3(
            head,
            self.timestamp.into_limbs()[0],
            &self.prev_randao,
            &self.suggested_fee_recipient,
        )
        .with_beacon_root(&self.parent_beacon_block_root)
        .with_withdrawals(
            self.withdrawals
                .iter()
                .map(ToWithdrawal::to_withdrawal)
                .collect::<Vec<_>>(),
        )
    }
}

trait ToWithdrawal {
    fn to_withdrawal(&self) -> alloy::eips::eip4895::Withdrawal;
}

impl ToWithdrawal for Withdrawal {
    fn to_withdrawal(&self) -> alloy::eips::eip4895::Withdrawal {
        alloy::eips::eip4895::Withdrawal {
            index: self.index,
            validator_index: self.validator_index,
            address: self.address,
            amount: self.amount,
        }
    }
}

pub(crate) trait WithPayloadAttributes {
    fn with_payload_attributes(self, payload: Payload) -> Self;
}

impl WithPayloadAttributes for Header {
    fn with_payload_attributes(self, payload: Payload) -> Self {
        Self {
            beneficiary: payload.suggested_fee_recipient,
            gas_limit: payload.gas_limit.to_u64(),
            timestamp: payload.timestamp.to_u64(),
            parent_beacon_block_root: Some(payload.parent_beacon_block_root),
            mix_hash: payload.prev_randao,
            ..self
        }
    }
}

#[test]
fn test_get_deposit_nonce() {
    const INPUT: [u8; 420] = alloy::hex!("d764ad0b0001000000000000000000000000000000000000000000000000000000000002000000000000000000000000c8088d0362bb4ac757ca77e211c30503d39cef4800000000000000000000000042000000000000000000000000000000000000100000000000000000000000000000000000000000000000056bc75e2d631000000000000000000000000000000000000000000000000000000000000000030d4000000000000000000000000000000000000000000000000000000000000000c000000000000000000000000000000000000000000000000000000000000000a41635f5fd000000000000000000000000c152ff76a513e15be1be43d102a881f076e707b3000000000000000000000000c152ff76a513e15be1be43d102a881f076e707b30000000000000000000000000000000000000000000000056bc75e2d631000000000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");

    let tx = TxDeposit {
        input: INPUT.into(),
        ..Default::default()
    };
    let VersionedNonce { version, nonce } = inner_get_deposit_nonce(&tx).unwrap();
    assert_eq!(nonce, 2);
    assert_eq!(version, 1);
}
