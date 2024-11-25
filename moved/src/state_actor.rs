pub use payload::{NewPayloadId, NewPayloadIdInput, StatePayloadId};

use {
    crate::{
        block::{
            Block, BlockHash, BlockQueries, BlockRepository, ExtendedBlock, GasFee, Header,
            HeaderForExecution,
        },
        genesis::config::GenesisConfig,
        merkle_tree::MerkleRootExt,
        move_execution::{
            execute_transaction, quick_get_eth_balance, quick_get_nonce, BaseTokenAccounts,
            CreateL1GasFee, L1GasFee, L1GasFeeInput, LogsBloom,
        },
        primitives::{ToMoveAddress, ToSaturatedU64, B256, U256, U64},
        storage::State,
        types::{
            state::{
                Command, ExecutionOutcome, Payload, PayloadId, PayloadResponse, Query,
                StateMessage, ToPayloadIdInput, WithExecutionOutcome, WithPayloadAttributes,
            },
            transactions::{ExtendedTxEnvelope, NormalizedExtendedTxEnvelope},
        },
        Error::{InvalidTransaction, InvariantViolation, User},
    },
    alloy::{
        consensus::{Receipt, ReceiptWithBloom},
        eips::BlockNumberOrTag,
        primitives::{keccak256, Bloom},
        rlp::{Decodable, Encodable},
        rpc::types::FeeHistory,
    },
    move_binary_format::errors::PartialVMError,
    std::collections::HashMap,
    tokio::{sync::mpsc::Receiver, task::JoinHandle},
};

mod payload;

#[derive(Debug)]
pub struct StateActor<
    S: State,
    P: NewPayloadId,
    H: BlockHash,
    R: BlockRepository<Storage = M>,
    G: GasFee,
    L1G: CreateL1GasFee,
    B: BaseTokenAccounts,
    Q: BlockQueries<Storage = M>,
    M,
> {
    genesis_config: GenesisConfig,
    rx: Receiver<StateMessage>,
    head: B256,
    height: u64,
    payload_id: P,
    block_hash: H,
    gas_fee: G,
    execution_payloads: HashMap<B256, PayloadResponse>,
    pending_payload: Option<(PayloadId, PayloadResponse)>,
    mem_pool: HashMap<B256, (ExtendedTxEnvelope, L1GasFeeInput)>,
    state: S,
    block_repository: R,
    block_queries: Q,
    l1_fee: L1G,
    base_token: B,
    block_memory: M,
}

impl<
        S: State<Err = PartialVMError> + Send + Sync + 'static,
        P: NewPayloadId + Send + Sync + 'static,
        H: BlockHash + Send + Sync + 'static,
        R: BlockRepository<Storage = M> + Send + Sync + 'static,
        G: GasFee + Send + Sync + 'static,
        L1G: CreateL1GasFee + Send + Sync + 'static,
        B: BaseTokenAccounts + Send + Sync + 'static,
        Q: BlockQueries<Storage = M> + Send + Sync + 'static,
        M: Send + Sync + 'static,
    > StateActor<S, P, H, R, G, L1G, B, Q, M>
{
    pub fn spawn(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(msg) = self.rx.recv().await {
                match msg {
                    StateMessage::Command(msg) => self.handle_command(msg),
                    StateMessage::Query(msg) => self.handle_query(msg),
                };
            }
        })
    }
}

impl<
        S: State<Err = PartialVMError>,
        P: NewPayloadId,
        H: BlockHash,
        R: BlockRepository<Storage = M>,
        G: GasFee,
        L1G: CreateL1GasFee,
        B: BaseTokenAccounts,
        Q: BlockQueries<Storage = M>,
        M,
    > StateActor<S, P, H, R, G, L1G, B, Q, M>
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rx: Receiver<StateMessage>,
        state: S,
        head: B256,
        genesis_config: GenesisConfig,
        payload_id: P,
        block_hash: H,
        block_repository: R,
        base_fee_per_gas: G,
        l1_fee: L1G,
        base_token: B,
        block_queries: Q,
        block_memory: M,
    ) -> Self {
        Self {
            genesis_config,
            rx,
            head,
            height: 0,
            payload_id,
            execution_payloads: HashMap::new(),
            pending_payload: None,
            mem_pool: HashMap::new(),
            state,
            block_hash,
            block_repository,
            gas_fee: base_fee_per_gas,
            l1_fee,
            base_token,
            block_queries,
            block_memory,
        }
    }

    pub fn handle_query(&self, msg: Query) {
        match msg {
            Query::ChainId { response_channel } => {
                response_channel.send(self.genesis_config.chain_id).ok()
            }
            Query::GetBalance {
                address,
                response_channel,
                ..
            } => {
                // TODO: Support balance from arbitrary blocks
                let account = address.to_move_address();
                let balance = quick_get_eth_balance(&account, self.state.resolver());
                response_channel.send(balance).ok()
            }
            Query::GetNonce {
                address,
                response_channel,
                ..
            } => {
                // TODO: Support nonce from arbitrary blocks
                let address = address.to_move_address();
                let nonce = quick_get_nonce(&address, self.state.resolver());
                response_channel.send(nonce).ok()
            }
            Query::BlockByHash {
                hash,
                response_channel,
                .. // TODO: Support `include_transactions`
            } => response_channel
                .send(self.block_queries.by_hash(&self.block_memory, hash))
                .ok(),
            Query::BlockByHeight {
                height,
                response_channel,
                .. // TODO: Support `include_transactions`
            } => response_channel.send(match height {
                BlockNumberOrTag::Number(height) => self.block_queries.by_height(&self.block_memory, height),
                // TODO: Support block "tag"
                _ => None,
            }).ok(),
            Query::BlockNumber {
                response_channel,
            } => response_channel
                .send(self.height)
                .ok(),
            Query::FeeHistory {
                response_channel,
                ..
                // TODO: Respond with a real fee history
            } => response_channel.send(FeeHistory::default()).ok(),
            Query::EstimateGas {
                response_channel,
                ..
                // TODO: Respond with a real gas estimation
            } => response_channel.send(0).ok(),
            Query::Call {
                response_channel,
                ..
                // TODO: Respond with a real transaction call result
            } => response_channel.send(vec![]).ok(),
        };
    }

    pub fn handle_command(&mut self, msg: Command) {
        match msg {
            Command::UpdateHead { block_hash } => {
                self.head = block_hash;
            }
            Command::StartBlockBuild {
                payload_attributes,
                response_channel,
            } => {
                let input = payload_attributes.to_payload_id_input(&self.head);
                let id = self.payload_id.new_payload_id(input);
                response_channel.send(id).ok();
                let block = self.create_block(payload_attributes);
                self.block_repository
                    .add(&mut self.block_memory, block.clone());
                let payload = PayloadResponse::from(block);
                self.pending_payload = Some((id, payload));
            }
            Command::GetPayload {
                id: request_id,
                response_channel,
            } => match self.pending_payload.take() {
                Some((id, payload)) => {
                    if request_id == id {
                        response_channel.send(Some(payload.clone())).ok();
                        self.execution_payloads
                            .insert(payload.execution_payload.block_hash, payload);
                    } else {
                        println!("WARN: unexpected PayloadId: {request_id}");
                        response_channel.send(None).ok();
                        self.pending_payload.replace((id, payload));
                    }
                }
                None => {
                    response_channel.send(None).ok();
                }
            },
            Command::GetPayloadByBlockHash {
                block_hash,
                response_channel,
            } => {
                let response = self.execution_payloads.get(&block_hash).cloned();
                response_channel.send(response).ok();
            }
            Command::AddTransaction { tx } => {
                let tx_hash = tx.tx_hash().0.into();
                let mut encoded = Vec::new();
                tx.encode(&mut encoded);
                let encoded = encoded.as_slice().into();
                self.mem_pool
                    .insert(tx_hash, (ExtendedTxEnvelope::Canonical(tx), encoded));
            }
        }
    }

    fn create_block(&mut self, payload_attributes: Payload) -> ExtendedBlock {
        // Include transactions from both `payload_attributes` and internal mem-pool
        let transactions = payload_attributes
            .transactions
            .iter()
            .filter_map(|tx_bytes| {
                let mut slice: &[u8] = tx_bytes.as_ref();
                let tx_hash = B256::new(keccak256(slice).0);
                let tx = ExtendedTxEnvelope::decode(&mut slice)
                    .inspect_err(|_| {
                        println!("WARN: Failed to RLP decode transaction in payload_attributes")
                    })
                    .ok()?;

                Some((tx_hash, (tx, L1GasFeeInput::from(slice))))
            })
            .chain(self.mem_pool.drain())
            .collect::<Vec<_>>();
        let parent = self
            .block_repository
            .by_hash(&self.block_memory, self.head)
            .expect("Parent block should exist");
        let base_fee = self.gas_fee.base_fee_per_gas(
            parent.block.header.gas_limit,
            parent.block.header.gas_used,
            parent.block.header.base_fee_per_gas,
        );

        let header_for_execution = HeaderForExecution {
            number: self.height + 1,
            timestamp: payload_attributes.timestamp.as_limbs()[0],
            prev_randao: payload_attributes.prev_randao,
        };
        let execution_outcome = self.execute_transactions(
            transactions
                .iter()
                .cloned()
                .filter_map(|(tx_hash, (tx, bytes))| {
                    tx.try_into().ok().map(|tx| (tx_hash, tx, bytes))
                }),
            base_fee,
            &header_for_execution,
        );

        let transactions: Vec<_> = transactions.into_iter().map(|(_, (tx, _))| tx).collect();
        let transactions_root = transactions
            .iter()
            .map(alloy::rlp::encode)
            .map(keccak256)
            .merkle_root();
        let total_tip = execution_outcome.total_tip;
        // TODO: Compute `withdrawals_root`
        let header = Header::new(self.head, header_for_execution.number)
            .with_payload_attributes(payload_attributes)
            .with_execution_outcome(execution_outcome)
            .with_transactions_root(transactions_root)
            .with_base_fee_per_gas(base_fee);

        let hash = self.block_hash.block_hash(&header);

        Block::new(header, transactions)
            .with_hash(hash)
            .with_value(total_tip)
    }

    fn execute_transactions(
        &mut self,
        transactions: impl Iterator<Item = (B256, NormalizedExtendedTxEnvelope, L1GasFeeInput)>,
        base_fee: U256,
        block_header: &HeaderForExecution,
    ) -> ExecutionOutcome {
        let mut total_tip = U256::ZERO;
        let mut outcomes = Vec::new();
        let mut transactions = transactions.peekable();

        // https://github.com/ethereum-optimism/specs/blob/9dbc6b0/specs/protocol/deposits.md#kinds-of-deposited-transactions
        let l1_fee = transactions
            .peek()
            .and_then(|(_, v, _)| v.as_deposited())
            .map(|v| self.l1_fee.for_deposit(v.data.as_ref()));

        // TODO: parallel transaction processing?
        for (tx_hash, tx, l1_cost_input) in transactions {
            let outcome = match execute_transaction(
                &tx,
                &tx_hash,
                self.state.resolver(),
                &self.genesis_config,
                l1_fee
                    .as_ref()
                    .map(|v| v.l1_fee(l1_cost_input).to_saturated_u64())
                    .unwrap_or(0),
                &self.base_token,
                block_header.clone(),
            ) {
                Ok(outcome) => outcome,
                Err(User(_)) => unreachable!("User errors are handled in execution"),
                Err(InvalidTransaction(_)) => continue,
                Err(InvariantViolation(e)) => panic!("ERROR: execution error {e:?}"),
            };

            self.state
                .apply(outcome.changes)
                .unwrap_or_else(|_| panic!("ERROR: state update failed for transaction {tx:?}"));

            outcomes.push((outcome.vm_outcome.is_ok(), outcome.gas_used, outcome.logs));

            total_tip = total_tip.saturating_add(
                U256::from(outcome.gas_used).saturating_mul(tx.tip_per_gas(base_fee)),
            );
        }

        let mut cumulative_gas_used = 0u64;
        let mut logs_bloom = Bloom::ZERO;
        let receipts = outcomes.into_iter().map(|(status, gas_used, logs)| {
            cumulative_gas_used = cumulative_gas_used.saturating_add(gas_used);

            let bloom = logs.iter().logs_bloom();
            logs_bloom.accrue_bloom(&bloom);

            let receipt = Receipt {
                status: status.into(),
                cumulative_gas_used: cumulative_gas_used as u128,
                logs,
            };
            ReceiptWithBloom::new(receipt, bloom)
        });

        let receipts_root = receipts
            .map(alloy::rlp::encode)
            .map(keccak256)
            .merkle_root();
        let logs_bloom = logs_bloom.into();

        ExecutionOutcome {
            state_root: self.state.state_root(),
            gas_used: U64::from(cumulative_gas_used),
            receipts_root,
            logs_bloom,
            total_tip,
        }
    }
}
