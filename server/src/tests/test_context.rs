use {
    crate::{dependency, initialize_app},
    alloy::primitives::{hex, B256},
    moved_api::schema::{
        ForkchoiceUpdatedResponseV1, GetBlockResponse, GetPayloadResponseV3, PayloadStatusV1,
        Status,
    },
    moved_app::{Application, CommandQueue, Dependencies},
    moved_blockchain::payload::StatePayloadId,
    moved_genesis::config::GenesisConfig,
    serde::de::DeserializeOwned,
    std::sync::Arc,
    tokio::{sync::RwLock, task::JoinHandle},
};

const DEPOSIT_TX: &[u8] = &hex!("7ef8f8a032595a51f0561028c684fbeeb46c7221a34be9a2eedda60a93069dd77320407e94deaddeaddeaddeaddeaddeaddeaddeaddead00019442000000000000000000000000000000000000158080830f424080b8a4440a5e2000000000000000000000000000000000000000006807cdc800000000000000220000000000000000000000000000000000000000000000000000000000a68a3a000000000000000000000000000000000000000000000000000000000000000198663a8bf712c08273a02876877759b43dc4df514214cc2f6008870b9a8503380000000000000000000000008c67a7b8624044f8f672e9ec374dfa596f01afb9");

pub struct TestContext {
    pub queue: CommandQueue,
    pub app: Arc<RwLock<Application<dependency::Dependency>>>,
    head: B256,
    timestamp: u64,
    state_task: JoinHandle<()>,
}

impl TestContext {
    pub async fn new() -> anyhow::Result<Self> {
        let genesis_config = GenesisConfig::default();
        let app = initialize_app(genesis_config);
        let app = Arc::new(RwLock::new(app));
        let (queue, state) = moved_app::create(app.clone(), 10);

        let state_task = state.spawn();

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "eth_getBlockByNumber",
            "params": [
                "0x0",
                true
            ]
        });
        let genesis_block: GetBlockResponse = handle_request(request, &queue, &app).await?;
        let genesis_hash = genesis_block.0.header.hash;
        let timestamp = genesis_block.0.header.timestamp;

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "engine_forkchoiceUpdatedV3",
            "params": [
                {
                    "headBlockHash": format!("{}", genesis_hash),
                    "safeBlockHash": format!("{}", genesis_hash),
                    "finalizedBlockHash": format!("{}", genesis_hash)
                },
                null
            ]
        });
        let response: ForkchoiceUpdatedResponseV1 = handle_request(request, &queue, &app).await?;
        assert_eq!(response.payload_status.status, Status::Valid);

        Ok(Self {
            queue,
            app,
            state_task,
            head: genesis_hash,
            timestamp,
        })
    }

    pub async fn produce_block(&mut self) -> anyhow::Result<B256> {
        self.timestamp += 1;
        let head_hash = self.head;
        let timestamp = self.timestamp;
        let prev_rando = B256::random();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "engine_forkchoiceUpdatedV3",
            "params": [
                {
                    "headBlockHash": format!("{head_hash}"),
                    "safeBlockHash": format!("{head_hash}"),
                    "finalizedBlockHash": format!("{head_hash}")
                },
                {
                    "timestamp": format!("{timestamp:#x}"),
                    "prevRandao": format!("{prev_rando}"),
                    "suggestedFeeRecipient": "0x4200000000000000000000000000000000000011",
                    "withdrawals": [],
                    "parentBeaconBlockRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "transactions": [
                        hex::encode(DEPOSIT_TX)
                    ],
                    "gasLimit": "0x1c9c380"
                }
            ]
        });
        let response: ForkchoiceUpdatedResponseV1 =
            handle_request(request, &self.queue, &self.app).await?;
        let payload_id = response.payload_id.unwrap();

        self.queue.wait_for_pending_commands().await;

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 8,
            "method": "engine_getPayloadV3",
            "params": [
               String::from(payload_id),
            ]
        });
        let response: GetPayloadResponseV3 =
            handle_request(request, &self.queue, &self.app).await?;

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "engine_newPayloadV3",
            "params": [
               response.execution_payload,
               [],
               "0x0000000000000000000000000000000000000000000000000000000000000000"
            ]
        });
        let response: PayloadStatusV1 = handle_request(request, &self.queue, &self.app).await?;
        assert_eq!(response.status, Status::Valid);

        self.head = response.latest_valid_hash.unwrap();
        Ok(self.head)
    }

    pub async fn get_block_by_number(&self, number: u64) -> anyhow::Result<GetBlockResponse> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "eth_getBlockByNumber",
            "params": [
                format!("{number:#x}"),
                true
            ]
        });
        let block: GetBlockResponse = handle_request(request, &self.queue, &self.app).await?;
        Ok(block)
    }

    pub async fn shutdown(self) {
        drop(self.queue);
        self.state_task.await.unwrap();
    }
}

pub async fn handle_request<T: DeserializeOwned>(
    request: serde_json::Value,
    queue: &CommandQueue,
    app: &Arc<RwLock<Application<impl Dependencies>>>,
) -> anyhow::Result<T> {
    let response = moved_api::request::handle(
        request.clone(),
        queue.clone(),
        |_| true,
        &StatePayloadId,
        app,
    )
    .await;

    if let Some(error) = response.error {
        anyhow::bail!("Error response from request {request:?}: {error:?}");
    }

    let result: T = serde_json::from_value(response.result.expect("If not error then has result"))?;
    Ok(result)
}
