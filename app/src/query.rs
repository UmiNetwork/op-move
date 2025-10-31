use {
    crate::{ApplicationReader, Dependencies},
    alloy::{
        consensus::{Header, Transaction},
        eips::{
            BlockId,
            BlockNumberOrTag::{self, Earliest, Finalized, Latest, Number, Pending, Safe},
        },
        rpc::types::{FeeHistory, TransactionRequest},
    },
    aptos_api_types::TableItemRequest,
    move_core_types::{
        account_address::AccountAddress, identifier::Identifier, language_storage::StructTag,
    },
    move_table_extension::TableHandle,
    umi_blockchain::{
        block::{
            BaseGasFee, BlockQueries, BlockResponse,
            DEFAULT_EIP1559_BASE_FEE_MAX_CHANGE_DENOMINATOR, DEFAULT_EIP1559_ELASTICITY_MULTIPLIER,
            Eip1559GasFee, ForkchoiceState,
        },
        payload::{MaybePayloadResponse, PayloadId, PayloadQueries, PayloadResponse},
        receipt::{ReceiptQueries, TransactionReceipt},
        state::{
            MoveModuleResponse, MoveResourceResponse, MoveValueResponse, ProofResponse,
            StateQueries,
        },
        transaction::{TransactionQueries, TransactionResponse},
    },
    umi_evm_ext::HeaderForExecution,
    umi_execution::simulate::{call_transaction, simulate_transaction},
    umi_shared::{
        error::{Error, InvariantViolation, Result, UserError},
        primitives::{Address, B256, Bytes, ToMoveAddress, U256},
    },
};

const MAX_PERCENTILE_COUNT: usize = 100;
const PRIORITY_FEE_SUGGESTED_INCREASE_PERCENT: u128 = 10;
pub(crate) const MIN_SUGGESTED_PRIORITY_FEE: u128 = 1_000_000;
pub(crate) const MAX_SUGGESTED_PRIORITY_FEE: u128 = 500_000_000_000;

#[derive(Debug)]
enum FeeHistoryBlockId {
    RangeEnd(B256),
    LoopBody(B256),
}

impl<'app, D: Dependencies<'app>> ApplicationReader<'app, D> {
    pub fn chain_id(&self) -> u64 {
        self.genesis_config.chain_id
    }

    pub fn client_version(&self) -> String {
        format!(
            "op-move/v{}-{}/{}/{}",
            env!("CARGO_PKG_VERSION"),
            env!("GIT_HEAD"),
            env!("TARGET_TRIPLET"),
            env!("RUSTC_VERSION")
        )
    }

    pub fn balance_by_height(&self, address: Address, height: BlockNumberOrTag) -> Result<U256> {
        self.state_queries.balance_at(
            address.to_move_address(),
            self.resolve_height_to_header(height)?.hash,
        )
    }

    pub fn evm_bytecode_by_height(
        &self,
        address: Address,
        height: BlockNumberOrTag,
    ) -> Result<Bytes> {
        Ok(self
            .state_queries
            .evm_bytecode_at(
                address.to_move_address(),
                self.resolve_height_to_header(height)?.hash,
            )?
            .unwrap_or_default())
    }

    pub fn nonce_by_height(&self, address: Address, height: BlockNumberOrTag) -> Result<u64> {
        Ok(self.state_queries.nonce_at(
            &self.evm_storage,
            address.to_move_address(),
            self.resolve_height_to_header(height)?.hash,
        )?)
    }

    pub fn move_module_by_height(
        &self,
        address: AccountAddress,
        module_name: &str,
        height: BlockNumberOrTag,
    ) -> Result<MoveModuleResponse> {
        self.state_queries
            .move_module_at(
                address,
                module_name,
                self.resolve_height_to_header(height)?.hash,
            )?
            .ok_or_else(|| Error::User(UserError::MissingModule(module_name.to_string())))
    }

    pub fn move_resource_by_height(
        &self,
        address: AccountAddress,
        resource_name: &str,
        height: BlockNumberOrTag,
    ) -> Result<MoveResourceResponse> {
        self.state_queries
            .move_resource_at(
                address,
                resource_name,
                self.resolve_height_to_header(height)?.hash,
            )?
            .ok_or_else(|| Error::User(UserError::MissingResource(resource_name.to_string())))
    }

    pub fn move_table_item_by_height(
        &self,
        handle: &TableHandle,
        request: TableItemRequest,
        height: BlockNumberOrTag,
    ) -> Result<MoveValueResponse> {
        self.state_queries
            .table_item_at(handle, request, self.resolve_height_to_header(height)?.hash)?
            .ok_or_else(|| UserError::MissingTableItem.into())
    }

    pub fn storage(&self, address: Address, index: U256, height: BlockNumberOrTag) -> Result<U256> {
        let hash = self.resolve_height_to_header(height)?.hash;
        self.state_queries
            .evm_storage_at(&self.evm_storage, address, index, hash)
            .map_err(|_| Error::DatabaseState)
    }

    pub fn block_by_hash(&self, hash: B256, include_transactions: bool) -> Result<BlockResponse> {
        self.block_queries
            .by_hash(&self.storage, hash, include_transactions)
            .map_err(|_| Error::DatabaseState)?
            .ok_or(Error::User(UserError::InvalidBlockHash(hash)))
    }

    pub fn block_by_height(
        &self,
        height: BlockNumberOrTag,
        include_transactions: bool,
    ) -> Result<BlockResponse> {
        let header = self.resolve_height_to_header(height)?;
        self.block_queries
            .by_hash(&self.storage, header.hash, include_transactions)
            .map_err(|_| Error::DatabaseState)?
            .ok_or(Error::User(UserError::InvalidBlockHeight(header.number)))
    }

    pub fn get_forkchoice_state(&self) -> Result<ForkchoiceState> {
        self.block_queries
            .get_forkchoice_state(&self.storage)
            .map_err(|_| Error::DatabaseState)
    }

    pub fn block_number(&self) -> Result<u64> {
        let fc = self
            .block_queries
            .get_forkchoice_state(&self.storage)
            .map_err(|_| Error::DatabaseState)?;
        let block = self
            .block_queries
            .by_hash(&self.storage, fc.head_block_hash, false)
            .map_err(|_| Error::DatabaseState)?;
        let height = block
            .map(|b| b.0.header.number)
            .ok_or(Error::InvariantViolation(InvariantViolation::GenesisBlock))?;
        Ok(height)
    }

    pub fn ancestor_check(&self, maybe_ancestor: B256, head: B256) -> Result<bool> {
        self.block_queries
            .ancestor_check(&self.storage, maybe_ancestor, head)
            .map_err(|_| Error::DatabaseState)
    }

    pub fn fee_history(
        &self,
        block_count: u64,
        block_number: BlockNumberOrTag,
        reward_percentiles: Option<Vec<f64>>,
    ) -> Result<FeeHistory> {
        if block_count < 1 {
            return Err(Error::User(UserError::InvalidBlockCount(block_count)));
        }
        // reward percentiles should be within (0..100) range and non-decreasing, up to a maximum
        // of 100 elements
        if let Some(reward) = &reward_percentiles {
            if reward.len() > MAX_PERCENTILE_COUNT {
                return Err(Error::User(UserError::RewardPercentilesTooLong {
                    max: MAX_PERCENTILE_COUNT,
                    given: reward.len(),
                }));
            }
            if reward.windows(2).any(|w| w[0] > w[1]) {
                return Err(Error::User(UserError::InvalidRewardPercentiles(
                    reward.clone(),
                )));
            }
            if reward.first() < Some(&0.0) || reward.last() > Some(&100.0) {
                return Err(Error::User(UserError::InvalidRewardPercentiles(
                    reward.clone(),
                )));
            }
        }

        let last_block = self.resolve_height_to_header(block_number)?;

        let latest_block_num = self.block_number()?;
        // Genesis block is counted as 0
        let block_count = std::cmp::min(block_count, latest_block_num + 1);
        // As block count was clipped above,
        // saturating sub is technically not needed, but it's still better
        // to err on the safe side
        let oldest_block = (last_block.number + 1).saturating_sub(block_count);

        // base fees (and blob base fees) array should include the fee of the next block past the
        // end of the range as well
        let mut base_fees = Vec::with_capacity(block_count as usize + 1);
        let mut gas_used_ratio = Vec::with_capacity(block_count as usize);

        let mut total_reward: Option<Vec<Vec<u128>>>;

        match reward_percentiles {
            None => {
                total_reward = None;
                let mut current_block_num = last_block.number;
                let mut current_block_id = FeeHistoryBlockId::RangeEnd(last_block.hash);
                while current_block_num >= oldest_block {
                    let parent_hash = self.collect_fee_history_for_block(
                        &mut base_fees,
                        &mut gas_used_ratio,
                        &mut Vec::new(),
                        current_block_id,
                        |_, _, _| (),
                    )?;
                    if parent_hash.is_zero() || current_block_num == 0 {
                        break;
                    }
                    current_block_id = FeeHistoryBlockId::LoopBody(parent_hash);
                    current_block_num = current_block_num.saturating_sub(1);
                }
            }
            Some(percentiles) => {
                let mut inner_total_reward = Vec::with_capacity(block_count as usize);
                let mut current_block_num = last_block.number;
                let mut current_block_id = FeeHistoryBlockId::RangeEnd(last_block.hash);
                while current_block_num >= oldest_block {
                    let parent_hash = self.collect_fee_history_for_block(
                        &mut base_fees,
                        &mut gas_used_ratio,
                        &mut inner_total_reward,
                        current_block_id,
                        |total_reward, block_gas_used, price_and_cum_gas| {
                            total_reward.push(
                                percentiles
                                    .iter()
                                    .map(|p| {
                                        let threshold =
                                            ((block_gas_used as f64) * p / 100.0).round() as u64;
                                        price_and_cum_gas
                                            .iter()
                                            .find(|(_, cum_gas)| cum_gas >= &threshold)
                                            .or_else(|| price_and_cum_gas.last())
                                            .map(|(p, _)| p)
                                            .copied()
                                            .unwrap_or(0u128)
                                    })
                                    .collect::<Vec<_>>(),
                            )
                        },
                    )?;
                    if parent_hash.is_zero() || current_block_num == 0 {
                        break;
                    }
                    current_block_id = FeeHistoryBlockId::LoopBody(parent_hash);
                    current_block_num -= 1;
                }
                total_reward = Some(inner_total_reward);
            }
        }

        // all the collected vectors need to be reversed as the iteration was in latest to
        // earliest block order
        base_fees.reverse();
        gas_used_ratio.reverse();
        if let Some(ref mut inner_reward) = total_reward {
            inner_reward.reverse();
        }

        // EIP-4844 txs not planned for support
        let base_fee_per_blob_gas = vec![0u128; block_count as usize + 1];
        let blob_gas_used_ratio = vec![0f64; block_count as usize];

        Ok(FeeHistory {
            base_fee_per_gas: base_fees,
            gas_used_ratio,
            base_fee_per_blob_gas,
            blob_gas_used_ratio,
            oldest_block,
            reward: total_reward,
        })
    }

    pub fn estimate_gas(
        &self,
        transaction: TransactionRequest,
        block_number: BlockNumberOrTag,
    ) -> Result<u64> {
        let header = self.resolve_height_to_header(block_number)?;
        let outcome = simulate_transaction(
            transaction,
            &self.state_queries.resolver_at(header.hash)?,
            &self.evm_storage,
            &self.genesis_config,
            &self.base_token,
            header.number,
            &self.block_hash_lookup,
        );

        outcome.map(|outcome| {
            // Add 33% extra gas as a buffer.
            outcome.gas_used + (outcome.gas_used / 3)
        })
    }

    pub fn call(
        &self,
        transaction: TransactionRequest,
        block_number: BlockNumberOrTag,
    ) -> Result<Vec<u8>> {
        let header = self.resolve_height_to_header(block_number)?;
        let header_for_execution = HeaderForExecution {
            number: header.number,
            timestamp: header.timestamp,
            prev_randao: header.mix_hash,
            chain_id: self.genesis_config.chain_id,
        };
        call_transaction(
            transaction,
            &self.state_queries.resolver_at(header.hash)?,
            &self.evm_storage,
            header_for_execution,
            &self.genesis_config,
            &self.base_token,
            &self.block_hash_lookup,
        )
    }

    pub fn gas_price(&self) -> Result<u128> {
        let (block_base_fee, suggestion) = self.estimate_priority_fee()?;
        // legacy `eth_gasPrice` call includes base fee for block
        Ok(suggestion + block_base_fee as u128)
    }

    pub fn max_priority_fee_per_gas(&self) -> Result<u128> {
        let (_, suggestion) = self.estimate_priority_fee()?;
        // virtually the same as `eth_gasPrice` but for dynamic fee transactions, and doesn't
        // add back the base fee
        Ok(suggestion)
    }

    pub fn transaction_receipt(&self, tx_hash: B256) -> Result<Option<TransactionReceipt>> {
        self.receipt_queries
            .by_transaction_hash(&self.receipt_memory, tx_hash)
            .map_err(|_| Error::DatabaseState)
    }

    pub fn transaction_by_hash(&self, tx_hash: B256) -> Result<Option<TransactionResponse>> {
        self.transaction_queries
            .by_hash(&self.storage, tx_hash)
            .map_err(|_| Error::DatabaseState)
    }

    pub fn proof(
        &self,
        address: Address,
        storage_slots: Vec<U256>,
        height: BlockId,
    ) -> Result<ProofResponse> {
        let hash = self.hash_from_block_id(height)?;
        Ok(self.state_queries.proof_at(
            &self.evm_storage,
            address.to_move_address(),
            &storage_slots,
            hash,
        )?)
    }

    pub fn payload(&self, id: PayloadId) -> Result<MaybePayloadResponse> {
        self.payload_queries
            .by_id(&self.storage, id)
            .map_err(|_| Error::DatabaseState)
    }

    pub fn payload_by_block_hash(&self, block_hash: B256) -> Result<PayloadResponse> {
        self.payload_queries
            .by_hash(&self.storage, block_hash)
            .map_err(|_| Error::DatabaseState)?
            .ok_or(Error::User(UserError::InvalidBlockHash(block_hash)))
    }

    pub fn move_list_modules(
        &self,
        address: Address,
        height: BlockNumberOrTag,
        after: Option<&Identifier>,
        limit: u32,
    ) -> Result<Vec<Identifier>> {
        let hash = self.resolve_height_to_header(height)?.hash;
        Ok(self
            .state_queries
            .move_list_modules(address.to_move_address(), hash, after, limit)?)
    }

    pub fn move_list_resources(
        &self,
        address: Address,
        height: BlockNumberOrTag,
        after: Option<&StructTag>,
        limit: u32,
    ) -> Result<Vec<StructTag>> {
        let hash = self.resolve_height_to_header(height)?.hash;
        Ok(self
            .state_queries
            .move_list_resources(address.to_move_address(), hash, after, limit)?)
    }

    fn resolve_height_to_header(
        &self,
        height: BlockNumberOrTag,
    ) -> Result<alloy::rpc::types::eth::Header> {
        let fc = self
            .block_queries
            .get_forkchoice_state(&self.storage)
            .map_err(|_| Error::DatabaseState)?;
        let latest = self
            .block_queries
            .by_hash(&self.storage, fc.head_block_hash, false)
            .map_err(|_| Error::DatabaseState)?
            .ok_or(Error::User(UserError::InvalidBlockHash(fc.head_block_hash)))?;
        match height {
            Pending | Latest => Ok(latest.0.header),
            Finalized => {
                let finalized = self
                    .block_queries
                    .by_hash(&self.storage, fc.finalized_block_hash, false)
                    .map_err(|_| Error::DatabaseState)?
                    .ok_or(Error::User(UserError::InvalidBlockHash(
                        fc.finalized_block_hash,
                    )))?;
                Ok(finalized.0.header)
            }
            Safe => {
                let safe = self
                    .block_queries
                    .by_hash(&self.storage, fc.safe_block_hash, false)
                    .map_err(|_| Error::DatabaseState)?
                    .ok_or(Error::User(UserError::InvalidBlockHash(fc.safe_block_hash)))?;
                Ok(safe.0.header)
            }
            Earliest => {
                let height = 0;
                let hash = self
                    .block_queries
                    .height_to_hash(&self.storage, height)
                    .map_err(|_| Error::DatabaseState)?;
                let block = self
                    .block_queries
                    .by_hash(&self.storage, hash, false)
                    .map_err(|_| Error::DatabaseState)?
                    .ok_or(Error::User(UserError::InvalidBlockHeight(height)))?;
                Ok(block.0.header)
            }
            Number(invalid_height) if invalid_height > latest.0.header.number => {
                Err(Error::User(UserError::InvalidBlockHeight(invalid_height)))
            }
            Number(height) => {
                let hash = self
                    .block_queries
                    .height_to_hash(&self.storage, height)
                    .map_err(|_| Error::DatabaseState)?;
                let block = self
                    .block_queries
                    .by_hash(&self.storage, hash, false)
                    .map_err(|_| Error::DatabaseState)?
                    .ok_or(Error::User(UserError::InvalidBlockHeight(height)))?;
                Ok(block.0.header)
            }
        }
    }

    fn hash_from_block_id(&self, id: BlockId) -> Result<B256> {
        match id {
            BlockId::Number(height) => Ok(self.resolve_height_to_header(height)?.hash),
            BlockId::Hash(hash) => Ok(hash.block_hash),
        }
    }

    fn collect_fee_history_for_block<F>(
        &self,
        base_fees: &mut Vec<u128>,
        gas_used_ratios: &mut Vec<f64>,
        total_reward: &mut Vec<Vec<u128>>,
        block_id: FeeHistoryBlockId,
        mut acc_total_reward: F,
    ) -> Result<B256>
    where
        F: FnMut(&mut Vec<Vec<u128>>, u64, &[(u128, u64)]),
    {
        let curr_block = match block_id {
            FeeHistoryBlockId::RangeEnd(hash) | FeeHistoryBlockId::LoopBody(hash) => {
                self.block_by_hash(hash, false)?
            }
        };
        let Header {
            gas_limit,
            gas_used: block_gas_used,
            base_fee_per_gas,
            parent_hash,
            #[cfg(feature = "op-upgrade")]
            extra_data,
            #[cfg(not(feature = "op-upgrade"))]
                extra_data: _,
            ..
        } = curr_block.0.header.inner;

        // For the last block, instead of querying block repo again, we resort to direct calculation
        // so that we also account for the range ending with the latest block. This comes before
        // the remaining calculation as we're iterating in reverse
        if matches!(block_id, FeeHistoryBlockId::RangeEnd(_)) {
            #[cfg_attr(not(feature = "op-upgrade"), allow(unused_mut))]
            let mut gas_fee = Eip1559GasFee::new(
                DEFAULT_EIP1559_ELASTICITY_MULTIPLIER,
                DEFAULT_EIP1559_BASE_FEE_MAX_CHANGE_DENOMINATOR,
            );
            #[cfg(feature = "op-upgrade")]
            {
                use {alloy::primitives::U64, umi_blockchain::block::BaseFeeParameters};

                // OP uses this field for dynamic EIP-1559 parameters only. The format is
                // <https://specs.optimism.io/protocol/holocene/exec-engine.html#eip-1559-parameters-in-block-header>
                if extra_data.len() != 9 {
                    return Err(Error::extra_data_invariant_violation());
                };

                // As during block build the parameters are parsed from a byte-encoded field
                // in payload attributes, we have to do some conversions to read it from the
                // block header, most importantly skipping the version byte that is present
                // in the header, but absent from the attributes
                let encoded = U64::from_be_slice(&extra_data.slice(1..9));
                let params = BaseFeeParameters::decode(encoded)?;
                gas_fee.set_parameters_from_attrs(&params);
            }
            let next_block_base_fee = gas_fee.base_fee_per_gas(
                gas_limit,
                block_gas_used,
                base_fee_per_gas.unwrap_or_default(),
            );
            base_fees.push(next_block_base_fee.into());
        }

        base_fees.push(base_fee_per_gas.unwrap_or_default().into());

        // to account for weird edge cases in devnet/testnet environments, as defaulting
        // to 0.0 instead of NaN makes more sense
        let gas_used_ratio = if gas_limit == 0 {
            0.0
        } else {
            (block_gas_used as f64) / (gas_limit as f64)
        };
        gas_used_ratios.push(gas_used_ratio);

        let mut price_and_gas: Vec<(u128, u64)> = curr_block
            .0
            .transactions
            .into_hashes()
            .hashes()
            .map(|hash| {
                let rx = self
                    .transaction_receipt(hash)?
                    .ok_or(Error::DatabaseState)?;
                Ok((rx.inner.effective_gas_price, rx.inner.gas_used))
            })
            .collect::<Result<_>>()?;
        price_and_gas.sort_by_key(|&(price, _)| price);
        let price_and_cum_gas = price_and_gas
            .iter()
            .scan(0u64, |cum_gas, (price, gas)| {
                *cum_gas = (*cum_gas).saturating_add(*gas);
                Some((*price, *cum_gas))
            })
            .collect::<Vec<_>>();

        acc_total_reward(total_reward, block_gas_used, &price_and_cum_gas);
        Ok(parent_hash)
    }

    fn estimate_priority_fee(&self) -> Result<(u64, u128)> {
        let latest_block = self.block_by_height(BlockNumberOrTag::Latest, true)?;
        let Header {
            gas_limit: block_gas_limit,
            gas_used: block_gas_used,
            base_fee_per_gas: block_base_fee,
            ..
        } = latest_block.0.header.inner;

        let max_tx_gas_used = latest_block
            .0
            .transactions
            .hashes()
            .map(|hash| {
                let rx = self
                    .transaction_receipt(hash)?
                    .ok_or(Error::DatabaseState)?;
                Ok(rx.inner.gas_used)
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .max()
            .unwrap_or_default();

        let mut suggestion = 0;
        // Using the same heuristic as `op-geth` does, only doing some actual calculations if the
        // block is nearing congestion. While we could have a precise calculation due to having
        // direct access to mempool, unlike what OP is assuming, this is still good enough as it
        // doesn't get too involved.
        if block_gas_used + max_tx_gas_used > block_gas_limit {
            let mut tx_tips = latest_block
                .0
                .transactions
                .into_transactions()
                .map(|tx| {
                    tx.inner
                        .inner
                        .effective_tip_per_gas(block_base_fee.unwrap_or_default())
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>();
            tx_tips.sort();
            // This is the exact amount bump used by `op-geth`
            suggestion = tx_tips
                .get(tx_tips.len() / 2)
                .map(|median_tip| {
                    median_tip + (median_tip / PRIORITY_FEE_SUGGESTED_INCREASE_PERCENT)
                })
                .unwrap_or_default();
        }
        Ok((
            block_base_fee.unwrap_or_default(),
            suggestion.clamp(MIN_SUGGESTED_PRIORITY_FEE, MAX_SUGGESTED_PRIORITY_FEE),
        ))
    }
}
