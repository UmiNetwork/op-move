use {
    alloy::{
        consensus::transaction::TxEnvelope,
        eips::{BlockId, BlockNumberOrTag},
        primitives::Bloom,
        rpc::types::{FeeHistory, TransactionRequest},
    },
    moved_blockchain::{
        block::{BlockResponse, ExtendedBlock, Header},
        payload::{NewPayloadIdInput, PayloadId, PayloadResponse},
        receipt::TransactionReceipt,
        state::ProofResponse,
        transaction::TransactionResponse,
    },
    moved_shared::primitives::{Address, Bytes, ToU64, B2048, B256, U256, U64},
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
        response_channel: oneshot::Sender<moved_shared::error::Result<u64>>,
    },
    Call {
        transaction: TransactionRequest,
        block_number: BlockNumberOrTag,
        response_channel: oneshot::Sender<moved_shared::error::Result<Vec<u8>>>,
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
    GetPayload {
        id: PayloadId,
        response_channel: oneshot::Sender<Option<PayloadResponse>>,
    },
    GetPayloadByBlockHash {
        block_hash: B256,
        response_channel: oneshot::Sender<Option<PayloadResponse>>,
    },
}

impl From<Query> for StateMessage {
    fn from(value: Query) -> Self {
        Self::Query(value)
    }
}

pub type RpcBlock = alloy::rpc::types::Block<RpcTransaction>;
pub type RpcTransaction = op_alloy::rpc_types::Transaction;

#[derive(Debug)]
pub struct ExecutionOutcome {
    pub receipts_root: B256,
    pub state_root: B256,
    pub logs_bloom: B2048,
    pub gas_used: U64,
    pub total_tip: U256,
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
