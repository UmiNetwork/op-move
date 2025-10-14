use {
    crate::{
        Application, Dependencies, ExecutionOutcome, PayloadForExecution,
        actor::UnrecoverableAppFailure,
        input::{WithExecutionOutcome, WithPayloadAttributes},
    },
    alloy::{
        consensus::Receipt,
        primitives::{B256, Bloom},
        rlp::Encodable,
        rpc::types::engine::ForkchoiceState,
    },
    metrics::histogram,
    umi_blockchain::{
        block::{
            BaseGasFee, Block, BlockHash, BlockQueries, BlockRepository, ExtendedBlock, Header,
        },
        payload::{PayloadId, PayloadQueries},
        receipt::{ExtendedReceipt, ReceiptQueries, ReceiptRepository},
        transaction::{ExtendedTransaction, TransactionRepository},
    },
    umi_evm_ext::{
        HeaderForExecution,
        state::{BlockHashWriter, StorageTrieRepository},
    },
    umi_execution::{
        CanonicalExecutionInput, CreateL1GasFee, CreateL2GasFee, DepositExecutionInput, L1GasFee,
        L2GasFeeInput, LogsBloom, execute_transaction,
        transaction::{NormalizedEthTransaction, NormalizedExtendedTxEnvelope, WrapReceipt},
    },
    umi_shared::{
        error::Error::{DatabaseState, InvalidTransaction, InvariantViolation, User},
        primitives::{U64, U256},
    },
    umi_state::State,
};

impl<'app, D: Dependencies<'app>> Application<'app, D> {
    /// Build a block from the given payload.
    /// Returns the hash of the constructed block (if any).
    /// Returns `None` if the payload was previously executed or is already in progress.
    #[tracing::instrument(level = "debug", skip(self, attributes))]
    pub(crate) fn start_block_build(
        &mut self,
        attributes: PayloadForExecution,
        id: PayloadId,
    ) -> Result<Option<B256>, UnrecoverableAppFailure> {
        let t0 = std::time::Instant::now();

        let payload_exists = self
            .payload_queries
            .by_id(&self.storage_reader, id)
            .map_err(|e| {
                tracing::error!(
                    "Failure during `start_block_build`. Payload queries failed: {e:?}"
                );
                UnrecoverableAppFailure
            })?
            .is_some();
        if payload_exists {
            return Ok(None);
        }
        let in_progress_payloads = self.payload_queries.get_in_progress();
        // If there is a job with this id already present, nothing to do
        if in_progress_payloads.start_id(id).is_err() {
            return Ok(None);
        }

        let attributes_txs_len = attributes.transactions.len();
        let no_tx_pool = attributes.no_tx_pool.unwrap_or(false);

        let transactions = if no_tx_pool {
            // Though it is a relatively rare event without follower nodes, `op-node` can demand
            // to rebuild a block deterministically from payload attributes only. This also happens
            // when an L2 reorg is triggered. We log it for diagnostic purposes.
            tracing::warn!(
                "Building from payload attributes only, with no mempool txs: {attributes:?}"
            );
            attributes.transactions.to_vec()
        } else {
            attributes
                .transactions
                .iter()
                .cloned()
                .chain(self.mem_pool.iter().cloned().map(Into::into))
                .collect::<Vec<_>>()
        };

        let parent = self
            .block_repository
            .get_forkchoice_state(&self.storage)
            .and_then(|fc| {
                self.block_repository.by_hash(&self.storage, fc.head_block_hash)
            })
            .map_err(|e| {
                tracing::error!("Failure during `start_block_build`. Failed to get latest block from block repository: {e:?}");
                UnrecoverableAppFailure
            })?
            .expect("Block repository is non-empty (must always at least contain genesis)");

        let new_transactions = transactions
            .into_iter()
            .filter_map(|tx|
                // Do not include transactions we have already processed before
                match self.receipt_repository.contains(&self.receipt_memory, tx.tx_hash()) {
                    Ok(false) => Some(Ok(tx)),
                    Ok(true) => {
                        // Double check the transaction exists in the current fork
                        let receipt = self.receipt_queries.by_transaction_hash(&self.receipt_memory_reader, tx.tx_hash()).ok().flatten();
                        let Some(tx_block_hash) = receipt.and_then(|rx| rx.inner.block_hash) else {
                            return Some(Ok(tx));
                        };

                        let is_canonical = self.block_queries.ancestor_check(&self.storage_reader, tx_block_hash, parent.hash).ok().unwrap_or(false);
                        if is_canonical {
                            None
                        } else {
                            Some(Ok(tx))
                        }
                    }
                    Err(e) => Some(Err(e)),
                }
            )
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                tracing::error!("Failure during `start_block_build`. Receipt queries failed: {e:?}");
                UnrecoverableAppFailure
            })?;

        #[cfg(feature = "op-upgrade")]
        if let Some(params) = &attributes.eip1559_params {
            self.gas_fee.set_parameters_from_attrs(params);
        }
        let base_fee = self.gas_fee.base_fee_per_gas(
            parent.block.header.gas_limit,
            parent.block.header.gas_used,
            parent.block.header.base_fee_per_gas.unwrap_or_default(),
        );

        let header_for_execution = HeaderForExecution {
            number: parent.block.header.number + 1,
            timestamp: attributes.timestamp.as_limbs()[0],
            prev_randao: attributes.prev_randao,
            chain_id: self.genesis_config.chain_id,
        };
        let (execution_outcome, receipts, executed_transactions) = self.execute_transactions(
            new_transactions.clone().into_iter(),
            base_fee,
            &header_for_execution,
        )?;

        let transactions_root =
            alloy_trie::root::ordered_trie_root_with_encoder(&executed_transactions, |tx, buf| {
                tx.trie_encode(buf)
            });

        // The withdrawals in the EIP-4895 sense are not used by Optimism at all
        // (<https://specs.optimism.io/protocol/isthmus/exec-engine.html?highlight=withdrawals#p2p>),
        // so instead of calculating the root we can set it to an empty list hash at all times.
        // This warning is just a precaution for the unlikely case of it being present.
        if !attributes.withdrawals.is_empty() {
            tracing::warn!(
                "Payload attributes {:?} had unexpected non-empty withdrawals. Will still set withdrawals root to empty list hash",
                &attributes.withdrawals
            );
        }
        let total_tip = execution_outcome.total_tip;

        #[cfg(feature = "op-upgrade")]
        let withdrawals_root = {
            let evm_db = umi_evm_ext::ResolverBackedDB::new(
                &self.evm_storage,
                self.state.resolver(),
                &(),
                header_for_execution.number,
            );
            let message_passer_account = evm_db
            .get_account(&crate::L2_TO_L1_MESSAGE_PASSER_ADDRESS)
            .map_err(|e| {
                tracing::error!(
                    "Failure during `start_block_build`. Failed to get L2ToL1MessagePasser account from storage: {e:?}"
                );
                UnrecoverableAppFailure
            })?.expect("L2ToL1MessagePasser is deployed at genesis");
            Some(message_passer_account.inner.storage_root)
        };
        #[cfg(not(feature = "op-upgrade"))]
        let withdrawals_root = Some(alloy::consensus::constants::EMPTY_WITHDRAWALS);

        let header = Header {
            parent_hash: parent.hash,
            number: header_for_execution.number,
            transactions_root,
            withdrawals_root,
            base_fee_per_gas: Some(base_fee),
            blob_gas_used: Some(0),
            excess_blob_gas: Some(0),
            #[cfg(feature = "op-upgrade")]
            extra_data: self.gas_fee.encode_parameters_for_header(),
            ..Default::default()
        }
        .with_payload_attributes(attributes)
        .with_execution_outcome(execution_outcome);

        let block_hash = self.block_hash.block_hash(&header);

        let block = Block::new(
            header,
            executed_transactions
                .iter()
                .map(|v| v.trie_hash())
                .collect(),
        )
        .into_extended_with_hash(block_hash)
        .with_value(total_tip)
        .with_payload_id(id);

        let block_number = block.block.header.number;

        let extended_transactions = executed_transactions
            .iter()
            .cloned()
            .enumerate()
            .map(|(transaction_index, inner)| {
                ExtendedTransaction::new(
                    inner.effective_gas_price(base_fee),
                    inner,
                    block_number,
                    block_hash,
                    transaction_index as u64,
                )
            })
            .collect::<Vec<_>>();

        let size = block.byte_length(extended_transactions.clone());
        let block = block.with_size(size);

        self.receipt_repository
            .extend(
                &mut self.receipt_memory,
                receipts
                    .into_iter()
                    .map(|receipt| receipt.with_block_hash(block_hash)),
            )
            .map_err(|e| {
                tracing::error!(
                    "Failure during `start_block_build`. Failed to write receipts: {e:?}"
                );
                UnrecoverableAppFailure
            })?;

        self.transaction_repository
            .extend(&mut self.storage, extended_transactions)
            .map_err(|e| {
                tracing::error!(
                    "Failure during `start_block_build`. Failed to write transactions: {e:?}"
                );
                UnrecoverableAppFailure
            })?;

        self.block_hash_writer.push(block_number, block_hash);
        self.block_repository
            .add(&mut self.storage, block.clone())
            .map_err(|e| {
                tracing::error!(
                    "Failure during `start_block_build`. Failed to write produced block: {e:?}"
                );
                UnrecoverableAppFailure
            })?;

        (self.on_payload)(self, id, block_hash).map_err(|e| {
            tracing::error!(
                "Failure during `start_block_build`. `on_payload` callback failed: {e:?}"
            );
            UnrecoverableAppFailure
        })?;

        // Mempool should only be cleaned after all other building stages succeeded
        if !no_tx_pool {
            // Payload transactions always come first in the block
            new_transactions
                .iter()
                .skip(attributes_txs_len)
                .try_for_each(|tx| {
                    let tx = tx.as_canonical().ok_or_else(|| {
                        tracing::error!("Failure during `start_block_build`. Deposit transaction encountered outside of payload attributes in mempool removal.");
                        UnrecoverableAppFailure
                    })?;
                    self.mem_pool.remove_by_nonce(tx.nonce, tx.signer);
                    Ok(())
                })?;

            tracing::debug!(
                "Removed {} mempool transactions after successful block building",
                new_transactions.len().saturating_sub(attributes_txs_len)
            );
        }

        in_progress_payloads.finish_id(block, executed_transactions.into_iter().map(Into::into));

        histogram!("block_build_duration_seconds").record(t0.elapsed().as_secs_f64());
        metrics::counter!("blocks_produced_count").increment(1);

        Ok(Some(block_hash))
    }

    pub fn add_transaction(&mut self, tx: NormalizedEthTransaction) {
        self.mem_pool.insert(tx);
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub(crate) fn forkchoice_update(
        &mut self,
        state: ForkchoiceState,
        payload_id: Option<PayloadId>,
    ) -> Result<(), UnrecoverableAppFailure> {
        // We include the payload id here so that it can be marked as in-progress right away
        // even though the block build job hasn't started yet. This prevents getting an
        // Unknown payload id error if a `get_payload` request is sent before this
        // function finishes.
        if let Some(id) = payload_id {
            self.payload_queries
                .get_in_progress()
                .forkchoice_insert_id(id);
        }
        let head_block = self
            .block_repository
            .by_hash(&self.storage, state.head_block_hash)
            .map_err(|e| {
                tracing::error!(
                    "Failed to look up new canonical block hash during forkchoice update: {e:?}"
                );
                UnrecoverableAppFailure
            })?
            .ok_or_else(|| {
                tracing::error!(
                    "New canonical head block {:?} not found.",
                    state.head_block_hash
                );
                UnrecoverableAppFailure
            })?;
        self.block_repository
            .forkchoice_update(&mut self.storage, state)
            .map_err(|e| {
                tracing::error!("Failed to update forkchoice state: {e:?}");
                UnrecoverableAppFailure
            })?;
        let new_state_root = if head_block.block.header.number == 0 {
            // For the genesis block we take the state root from the genesis config instead
            // of the block header because the header root only includes the EVM initial state
            // to make the rest of OP Stack happy, but in reality we do want to use the state which
            // also includes all the initial Move contracts.
            self.genesis_config.initial_state_root
        } else {
            head_block.block.header.state_root
        };
        self.state.switch_state_root(new_state_root).map_err(|e| {
            tracing::error!("Failed to update state root during forkchoice update: {e:?}");
            UnrecoverableAppFailure
        })?;
        Ok(())
    }

    pub fn genesis_update(
        &mut self,
        block: ExtendedBlock,
    ) -> Result<(), <D::BlockRepository as BlockRepository>::Err> {
        let genesis_hash = block.hash;
        self.block_hash_writer.push(0, genesis_hash);
        self.block_repository.add(&mut self.storage, block)?;
        (self.on_payload)(self, PayloadId::ZERO, genesis_hash).ok();

        let mut fc = self.block_repository.get_forkchoice_state(&self.storage)?;
        if fc.head_block_hash == B256::ZERO {
            fc.finalized_block_hash = genesis_hash;
            fc.safe_block_hash = genesis_hash;
            fc.head_block_hash = genesis_hash;
            self.block_repository
                .forkchoice_update(&mut self.storage, fc)?;
        }

        Ok(())
    }

    fn execute_transactions(
        &mut self,
        transactions: impl Iterator<Item = NormalizedExtendedTxEnvelope>,
        base_fee: u64,
        block_header: &HeaderForExecution,
    ) -> Result<
        (
            ExecutionOutcome,
            Vec<ExtendedReceipt>,
            Vec<NormalizedExtendedTxEnvelope>,
        ),
        UnrecoverableAppFailure,
    > {
        let mut total_tip = U256::ZERO;
        let mut receipts = Vec::new();
        let mut executed_transactions = Vec::new();
        let mut transactions = transactions.peekable();
        let mut cumulative_gas_used = 0u128;
        let mut logs_bloom = Bloom::ZERO;
        let mut tx_index = 0;
        let mut log_offset = 0;

        // https://github.com/ethereum-optimism/specs/blob/9dbc6b0/specs/protocol/deposits.md#kinds-of-deposited-transactions
        let l1_fee = transactions
            .peek()
            .and_then(|tx| tx.as_deposit())
            .map(|tx| self.l1_fee.for_deposit(tx.input.as_ref()));
        let l2_fee = self.l2_fee.with_default_gas_fee_multiplier();

        // TODO: parallel transaction processing?
        for normalized_tx in transactions {
            let l2_gas_input = L2GasFeeInput::new(
                normalized_tx.gas_limit(),
                normalized_tx.effective_gas_price(base_fee),
            );
            let tx_hash = normalized_tx.tx_hash();
            let input = match &normalized_tx {
                NormalizedExtendedTxEnvelope::Canonical(tx) => CanonicalExecutionInput {
                    tx,
                    tx_hash: &tx.tx_hash,
                    state: self.state.resolver(),
                    storage_trie: &self.evm_storage,
                    genesis_config: &self.genesis_config,
                    l1_cost: l1_fee
                        .as_ref()
                        .map(|v| v.l1_fee(normalized_tx.l1_gas_fee_input()))
                        .unwrap_or(U256::ZERO),
                    l2_fee: l2_fee.clone(),
                    l2_input: l2_gas_input,
                    base_token: &self.base_token,
                    block_header: block_header.clone(),
                    block_hash_lookup: &self.block_hash_lookup,
                }
                .into(),
                NormalizedExtendedTxEnvelope::DepositedTx(tx) => DepositExecutionInput {
                    tx,
                    tx_hash: &tx_hash,
                    state: self.state.resolver(),
                    storage_trie: &self.evm_storage,
                    genesis_config: &self.genesis_config,
                    block_header: block_header.clone(),
                    block_hash_lookup: &self.block_hash_lookup,
                }
                .into(),
            };
            let outcome = match execute_transaction(input, &mut self.resolver_cache) {
                Ok(outcome) => outcome,
                e @ (Err(InvalidTransaction(_)) | Err(DatabaseState)) => {
                    tracing::warn!("Filtered invalid transaction. hash={tx_hash:?} reason={e:?}");
                    continue;
                }
                Err(User(e)) => unreachable!("User errors are handled in execution {e:?}"),
                Err(InvariantViolation(e)) => panic!("ERROR: execution error {e:?}"),
            };

            let l1_block_info = l1_fee
                .as_ref()
                .and_then(|x| x.l1_block_info(normalized_tx.l1_gas_fee_input()));

            self.on_tx(outcome.changes.move_vm.clone().accounts)
                .map_err(|e| {
                    tracing::error!(
                        "Failure during `start_block_build`. `on_tx` callback failed: {e:?}"
                    );
                    UnrecoverableAppFailure
                })?;

            self.state.apply(outcome.changes.move_vm).map_err(|e| {
                tracing::error!("State update failed for transaction {normalized_tx:?}\n{e:?}");
                UnrecoverableAppFailure
            })?;
            self.evm_storage.apply(outcome.changes.evm).map_err(|e| {
                tracing::error!(
                    "EVM storage update failed for transaction {normalized_tx:?}\n{e:?}"
                );
                UnrecoverableAppFailure
            })?;

            cumulative_gas_used = cumulative_gas_used.saturating_add(outcome.gas_used as u128);

            let bloom = outcome.logs.iter().logs_bloom();
            logs_bloom.accrue_bloom(&bloom);

            let tx_log_offset = log_offset;
            log_offset += outcome.logs.len() as u64;
            let receipt = Receipt {
                status: outcome.vm_outcome.is_ok().into(),
                cumulative_gas_used: if cumulative_gas_used < u64::MAX as u128 {
                    cumulative_gas_used as u64
                } else {
                    u64::MAX
                },
                logs: outcome.logs,
            };

            let receipt = normalized_tx.wrap_receipt(receipt, bloom);

            total_tip = total_tip.saturating_add(
                U256::from(outcome.gas_used)
                    .saturating_mul(U256::from(normalized_tx.tip_per_gas(base_fee))),
            );

            let (to, from) = match &normalized_tx {
                NormalizedExtendedTxEnvelope::Canonical(tx) => (tx.to.to(), tx.signer),
                NormalizedExtendedTxEnvelope::DepositedTx(tx) => (tx.to.to(), tx.from),
            };

            receipts.push(ExtendedReceipt {
                transaction_hash: tx_hash,
                to: to.copied(),
                from,
                receipt,
                l1_block_info,
                gas_used: outcome.gas_used,
                l2_gas_price: outcome.l2_price,
                transaction_index: tx_index,
                contract_address: outcome.deployment,
                logs_offset: tx_log_offset,
                block_hash: Default::default(),
                block_number: block_header.number,
                block_timestamp: block_header.timestamp,
            });

            executed_transactions.push(normalized_tx);

            tx_index += 1;
        }

        (self.on_tx_batch)(self).map_err(|e| {
            tracing::error!(
                "Failure during `start_block_build`. `on_tx_batch` callback failed: {e:?}"
            );
            UnrecoverableAppFailure
        })?;

        // Compute the receipts root by RLP-encoding each receipt to be a leaf of
        // a merkle trie.
        let receipts_root =
            alloy_trie::root::ordered_trie_root_with_encoder(&receipts, |rx, buf| {
                rx.receipt.encode(buf)
            });
        let logs_bloom = logs_bloom.into();

        let outcome = ExecutionOutcome {
            state_root: self.state.state_root(),
            gas_used: U64::from(cumulative_gas_used),
            receipts_root,
            logs_bloom,
            total_tip,
        };
        Ok((outcome, receipts, executed_transactions))
    }
}
