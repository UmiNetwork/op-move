pub use payload::{NewPayloadId, NewPayloadIdInput, StatePayloadId};

use {
    crate::{
        block::{Block, BlockHash, BlockRepository, Header},
        genesis::{config::GenesisConfig, init_storage},
        merkle_tree::MerkleRootExt,
        move_execution::execute_transaction,
        primitives::{B256, U64},
        storage::{InMemoryState, State},
        types::{
            engine_api::{
                GetPayloadResponseV3, PayloadAttributesV3, PayloadId, ToPayloadIdInput,
                WithPayloadAttributes,
            },
            state::{ExecutionOutcome, StateMessage, WithExecutionOutcome},
            transactions::ExtendedTxEnvelope,
        },
        Error::{InvalidTransaction, InvariantViolation, User},
    },
    alloy_consensus::{Receipt, ReceiptWithBloom},
    alloy_primitives::{keccak256, Bloom, Log},
    alloy_rlp::Decodable,
    move_binary_format::errors::PartialVMError,
    std::collections::HashMap,
    tokio::{sync::mpsc::Receiver, task::JoinHandle},
};

mod payload;

#[derive(Debug)]
pub struct StateActor<S: State, P: NewPayloadId, H: BlockHash, R: BlockRepository> {
    genesis_config: GenesisConfig,
    rx: Receiver<StateMessage>,
    head: B256,
    height: u64,
    payload_id: P,
    block_hash: H,
    execution_payloads: HashMap<B256, GetPayloadResponseV3>,
    pending_payload: Option<(PayloadId, GetPayloadResponseV3)>,
    mem_pool: HashMap<B256, ExtendedTxEnvelope>,
    state: S,
    block_repository: R,
}

impl<P: NewPayloadId, H: BlockHash, R: BlockRepository> StateActor<InMemoryState, P, H, R> {
    pub fn new_in_memory(
        rx: Receiver<StateMessage>,
        genesis_config: GenesisConfig,
        payload_id: P,
        block_hash: H,
        block_repository: R,
    ) -> Self {
        Self::new(
            rx,
            InMemoryState::new(),
            genesis_config,
            payload_id,
            block_hash,
            block_repository,
        )
    }
}

impl<
        S: State<Err = PartialVMError> + Send + Sync + 'static,
        P: NewPayloadId + Send + Sync + 'static,
        H: BlockHash + Send + Sync + 'static,
        R: BlockRepository + Send + Sync + 'static,
    > StateActor<S, P, H, R>
{
    pub fn spawn(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(msg) = self.rx.recv().await {
                self.handle_msg(msg)
            }
        })
    }
}

impl<S: State<Err = PartialVMError>, P: NewPayloadId, H: BlockHash, R: BlockRepository>
    StateActor<S, P, H, R>
{
    pub fn new(
        rx: Receiver<StateMessage>,
        mut storage: S,
        genesis_config: GenesisConfig,
        payload_id: P,
        block_hash: H,
        block_repository: R,
    ) -> Self {
        init_storage(&genesis_config, &mut storage);

        Self {
            genesis_config,
            rx,
            head: Default::default(),
            height: 0,
            payload_id,
            execution_payloads: HashMap::new(),
            pending_payload: None,
            mem_pool: HashMap::new(),
            state: storage,
            block_hash,
            block_repository,
        }
    }

    pub fn handle_msg(&mut self, msg: StateMessage) {
        match msg {
            StateMessage::UpdateHead { block_hash } => {
                self.head = block_hash;
            }
            StateMessage::StartBlockBuild {
                payload_attributes,
                response_channel,
            } => {
                let input = payload_attributes.to_payload_id_input(&self.head);
                let id = self.payload_id.new_payload_id(input);
                response_channel.send(id.clone()).ok();
                let block = self.create_block(payload_attributes);
                let hash = self.block_hash.block_hash(&block.header);
                let block = block.with_hash(hash);
                self.block_repository.add(block.clone());
                let payload = GetPayloadResponseV3::from(block);
                self.execution_payloads.insert(hash, payload.clone());
                self.pending_payload = Some((id, payload));
            }
            StateMessage::GetPayload {
                id: request_id,
                response_channel,
            } => match self.pending_payload.take() {
                Some((id, payload)) => {
                    if request_id == id {
                        response_channel.send(Some(payload.clone())).ok();
                        self.execution_payloads
                            .insert(payload.execution_payload.block_hash, payload);
                    } else {
                        let request_str: String = request_id.into();
                        println!("WARN: unexpected PayloadId: {request_str}");
                        response_channel.send(None).ok();
                        self.pending_payload = Some((id, payload));
                    }
                }
                None => {
                    response_channel.send(None).ok();
                }
            },
            StateMessage::GetPayloadByBlockHash {
                block_hash,
                response_channel,
            } => {
                let response = self.execution_payloads.get(&block_hash).cloned();
                response_channel.send(response).ok();
            }
            StateMessage::AddTransaction { tx } => {
                let tx_hash = tx.tx_hash().0.into();
                self.mem_pool
                    .insert(tx_hash, ExtendedTxEnvelope::Canonical(tx));
            }
        }
    }

    fn create_block(&mut self, payload_attributes: PayloadAttributesV3) -> Block {
        // Include transactions from both `payload_attributes` and internal mem-pool
        let transactions = payload_attributes
            .transactions
            .iter()
            .filter_map(|tx_bytes| {
                let mut slice: &[u8] = tx_bytes.as_ref();
                let tx_hash = B256::new(keccak256(slice).0);

                match ExtendedTxEnvelope::decode(&mut slice) {
                    Ok(tx) => Some((tx_hash, tx)),
                    Err(_) => {
                        println!("WARN: Failed to RLP decode transaction in payload_attributes");
                        None
                    }
                }
            })
            .chain(self.mem_pool.drain())
            .collect::<Vec<_>>();

        let execution_outcome = self.execute_transactions(&transactions);

        // TODO: Determine gas pricing for `base_fee_per_gas`
        // TODO: Compute `transaction_root`
        // TODO: Compute `withdrawals_root`
        let header = Header::new(self.head, self.height + 1)
            .with_payload_attributes(payload_attributes)
            .with_execution_outcome(execution_outcome);
        let transactions = transactions.into_iter().map(|(_, tx)| tx).collect();

        Block::new(header, transactions)
    }

    fn execute_transactions(
        &mut self,
        transactions: &[(B256, ExtendedTxEnvelope)],
    ) -> ExecutionOutcome {
        let mut outcomes = Vec::new();
        let mut logs_bloom = Bloom::ZERO;

        // TODO: parallel transaction processing?
        for (tx_hash, tx) in transactions {
            let outcome =
                match execute_transaction(tx, tx_hash, self.state.resolver(), &self.genesis_config)
                {
                    Ok(outcome) => outcome,
                    Err(User(_)) => unreachable!("User errors are handled in execution"),
                    Err(InvalidTransaction(_)) => continue,
                    Err(InvariantViolation(e)) => panic!("ERROR: execution error {e:?}"),
                };

            self.state
                .apply(outcome.changes)
                .unwrap_or_else(|_| panic!("ERROR: state update failed for transaction {tx:?}"));
            outcomes.push((outcome.vm_outcome.is_ok(), outcome.gas_used));
            logs_bloom.accrue_bloom(&outcome.logs_bloom);
        }

        let mut cumulative_gas_used = 0u64;
        let receipts = outcomes.into_iter().map(|(status, gas_used)| {
            cumulative_gas_used = cumulative_gas_used.saturating_add(gas_used);

            let receipt = Receipt {
                status: status.into(),
                cumulative_gas_used: cumulative_gas_used as u128,
                logs: Vec::<Log>::new(),
            };
            ReceiptWithBloom::new(receipt, Bloom::ZERO)
        });

        let receipts_root = receipts
            .map(alloy_rlp::encode)
            .map(keccak256)
            .merkle_root()
            .0
            .into();

        let logs_bloom = logs_bloom.0 .0.into();

        ExecutionOutcome {
            state_root: self.state.state_root(),
            gas_used: U64::from(cumulative_gas_used),
            receipts_root,
            logs_bloom,
        }
    }
}
