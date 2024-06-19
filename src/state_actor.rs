use {
    crate::types::{
        engine_api::{ExecutionPayloadV3, GetPayloadResponseV3, PayloadAttributesV3, PayloadId},
        state::{ExecutionOutcome, StateMessage},
    },
    alloy_consensus::transaction::TxEnvelope,
    alloy_rlp::Encodable,
    ethers_core::types::{Bytes, H256, U256, U64},
    std::collections::HashMap,
    tokio::{sync::mpsc::Receiver, task::JoinHandle},
};

#[derive(Debug)]
pub struct StateActor {
    rx: Receiver<StateMessage>,
    head: H256,
    payload_id: PayloadId,
    block_heights: HashMap<H256, U64>,
    execution_payloads: HashMap<H256, GetPayloadResponseV3>,
    pending_payload: Option<(PayloadId, GetPayloadResponseV3)>,
    mem_pool: HashMap<H256, TxEnvelope>,
}

impl StateActor {
    pub fn new(rx: Receiver<StateMessage>) -> Self {
        Self {
            rx,
            head: Default::default(),
            payload_id: PayloadId(Default::default()),
            block_heights: HashMap::new(),
            execution_payloads: HashMap::new(),
            pending_payload: None,
            mem_pool: HashMap::new(),
        }
    }

    pub fn spawn(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(msg) = self.rx.recv().await {
                match msg {
                    StateMessage::UpdateHead { block_hash } => {
                        self.head = block_hash;
                    }
                    StateMessage::SetPayloadId { id } => {
                        self.payload_id = id;
                    }
                    StateMessage::StartBlockBuild {
                        payload_attributes,
                        response_channel,
                    } => {
                        let id = self.payload_id.clone();
                        response_channel.send(id.clone()).ok();
                        let payload = self.create_execution_payload(payload_attributes);
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
                        self.mem_pool.insert(tx_hash, tx);
                    }
                    StateMessage::NewBlock {
                        block_hash,
                        block_height,
                    } => {
                        self.block_heights.insert(block_hash, block_height);
                        if let Some((_, payload)) = self.pending_payload.as_mut() {
                            payload.execution_payload.block_hash = block_hash;
                            payload.execution_payload.block_number = block_height;
                        }
                    }
                }
            }
        })
    }

    fn create_execution_payload(
        &mut self,
        payload_attributes: PayloadAttributesV3,
    ) -> GetPayloadResponseV3 {
        // Include transactions from both `payload_attributes` and internal mem-pool
        let mut transactions = payload_attributes.transactions;
        for (_, tx) in self.mem_pool.drain() {
            let capacity = tx.length();
            let mut bytes = Vec::with_capacity(capacity);
            tx.encode(&mut bytes);
            transactions.push(bytes.into())
        }
        let execution_outcome = self.execute_transactions(&transactions);
        let head_height = self
            .block_heights
            .get(&self.head)
            .copied()
            .unwrap_or(U64::zero());
        GetPayloadResponseV3 {
            execution_payload: ExecutionPayloadV3 {
                parent_hash: self.head,
                fee_recipient: payload_attributes.suggested_fee_recipient,
                state_root: execution_outcome.state_root,
                receipts_root: execution_outcome.receipts_root,
                logs_bloom: execution_outcome.logs_bloom,
                prev_randao: payload_attributes.prev_randao,
                block_number: head_height + 1,
                gas_limit: payload_attributes.gas_limit,
                gas_used: execution_outcome.gas_used,
                timestamp: payload_attributes.timestamp,
                extra_data: Bytes::default(),
                base_fee_per_gas: U256::zero(), // TODO: gas pricing?
                block_hash: H256::default(),    // TODO: proper block hash calculation
                transactions,
                withdrawals: payload_attributes.withdrawals,
                blob_gas_used: U64::zero(),
                excess_blob_gas: U64::zero(),
            },
            block_value: U256::zero(), // TODO: value?
            blobs_bundle: Default::default(),
            should_override_builder: false,
            parent_beacon_block_root: payload_attributes.parent_beacon_block_root,
        }
    }

    fn execute_transactions(&self, _transactions: &[Bytes]) -> ExecutionOutcome {
        // TODO: execution
        ExecutionOutcome {
            state_root: H256::default(),
            receipts_root: H256::default(),
            logs_bloom: Bytes::from(vec![0; 256]),
            gas_used: U64::zero(),
        }
    }
}
