//! Contains an actor that will trigger block production once per second.

use {
    crate::{client::UmiClient, stats::Datapoint},
    alloy::{
        consensus::Sealed,
        primitives::{Address, B256, TxKind, U64, U256},
    },
    jsonwebtoken::EncodingKey,
    op_alloy::consensus::{OpTxEnvelope, TxDeposit},
    std::time::{Duration, SystemTime},
    tokio::{
        sync::{broadcast::Receiver, mpsc::UnboundedSender},
        task::JoinHandle,
    },
    umi_api::schema::{ForkchoiceStateV1, GetPayloadResponseV3, PayloadAttributesV3, PayloadId},
};

/// Time to wait before checking if we build another block.
/// A new block build is only started if the timestamp (in seconds) is different
/// from the previous block build.
const INTERVAL: Duration = Duration::from_millis(50);

/// Time to wait before sending an RPC request again.
const RETRY_INTERVAL: Duration = Duration::from_millis(10);

/// Number of time to try an RPC method before giving up.
const MAX_TRIES: usize = 5;

const SUGGESTED_FEE_RECIPIENT: Address =
    alloy::primitives::address!("0x4200000000000000000000000000000000000011");
const PARENT_BEACON_BLOCK_ROOT: B256 = B256::ZERO;
const GAS_LIMIT: u64 = 30_000_000;
const DEPOSIT_FROM: Address =
    alloy::primitives::address!("0xdeaddeaddeaddeaddeaddeaddeaddeaddead0001");
const DEPOSIT_TO: Address =
    alloy::primitives::address!("0x4200000000000000000000000000000000000015");
const DEPOSIT_INPUT: &str = include_str!("res/deposit_input.hex");

pub struct BlockProduction {
    client: UmiClient,
    shutdown: Receiver<()>,
    head_block_hash: B256,
    head_timestamp: u64,
}

impl BlockProduction {
    pub fn new(
        genesis_block_hash: B256,
        jwt_secret: EncodingKey,
        stats_channel: UnboundedSender<Datapoint>,
        shutdown: Receiver<()>,
    ) -> Self {
        Self {
            client: UmiClient::new(stats_channel, Some(jwt_secret)),
            shutdown,
            head_block_hash: genesis_block_hash,
            head_timestamp: 0,
        }
    }

    pub fn spawn(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                // Don't send two forkchoice updates in the same second.
                if self.head_timestamp == now {
                    tokio::select! {
                        _ = tokio::time::sleep(INTERVAL) => continue,
                        _ = self.shutdown.recv() => break,
                    }
                }

                self.head_timestamp = now;

                let fc_state = ForkchoiceStateV1 {
                    head_block_hash: self.head_block_hash,
                    safe_block_hash: self.head_block_hash,
                    finalized_block_hash: self.head_block_hash,
                };

                let deposit_transaction = {
                    let input = hex::decode(DEPOSIT_INPUT.trim()).expect("Is correct hex");
                    let tx = TxDeposit {
                        source_hash: alloy::primitives::keccak256(now.to_be_bytes()),
                        from: DEPOSIT_FROM,
                        to: TxKind::Call(DEPOSIT_TO),
                        mint: None,
                        value: U256::ZERO,
                        gas_limit: 1_000_000,
                        is_system_transaction: false,
                        input: input.into(),
                    };
                    let envelope = OpTxEnvelope::Deposit(Sealed::new(tx));
                    alloy::rlp::encode(envelope).into()
                };

                let attrs = PayloadAttributesV3 {
                    timestamp: U64::from_limbs([now]),
                    prev_randao: alloy::primitives::keccak256(self.head_block_hash),
                    suggested_fee_recipient: SUGGESTED_FEE_RECIPIENT,
                    withdrawals: Vec::new(),
                    parent_beacon_block_root: PARENT_BEACON_BLOCK_ROOT,
                    transactions: vec![deposit_transaction],
                    gas_limit: U64::from_limbs([GAS_LIMIT]),
                    eip1559_params: None,
                    no_tx_pool: None,
                };

                // Race the forkchoice update call against the shutdown channel to ensure
                // shutdown commands are not blocked by waiting on a response from the server.
                let maybe_response = tokio::select! {
                    x = self.client.engine_forkchoice_update(fc_state, Some(attrs)) => x,
                    _ = self.shutdown.recv() => break,
                };

                let response = match maybe_response {
                    Ok(response) => response,
                    Err(e) => {
                        println!("WARN: error in forkchoice update: {e:?}");
                        continue;
                    }
                };
                let Some(payload_id) = response.payload_id else {
                    println!("WARN: forkchoice update failed to provide payload_id");
                    continue;
                };

                let response = match self.get_payload_with_retry(payload_id).await {
                    Ok(response) => response,
                    Err(e) => {
                        println!("WARN: error in get payload: {e:?}");
                        continue;
                    }
                };

                self.head_block_hash = response.execution_payload.block_hash;
            }
        })
    }

    async fn get_payload_with_retry(
        &self,
        payload_id: PayloadId,
    ) -> anyhow::Result<GetPayloadResponseV3> {
        let mut try_count = 0;
        loop {
            try_count += 1;
            match self.client.engine_get_payload(payload_id).await {
                Ok(response) => {
                    return Ok(response);
                }
                Err(e) => {
                    if try_count >= MAX_TRIES {
                        return Err(e);
                    }
                    tokio::time::sleep(RETRY_INTERVAL).await;
                }
            }
        }
    }
}
