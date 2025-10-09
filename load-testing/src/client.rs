use {
    crate::stats::Datapoint,
    alloy::primitives::{Address, U256},
    jsonwebtoken::{EncodingKey, Header},
    reqwest::{Client, Method},
    serde::de::DeserializeOwned,
    std::time::{Instant, SystemTime},
    tokio::sync::mpsc::UnboundedSender,
    umi_api::{
        jsonrpc::JsonRpcResponse,
        schema::{
            ForkchoiceStateV1, ForkchoiceUpdatedResponseV1, GetBlockResponse, GetPayloadResponseV3,
            PayloadAttributesV3, PayloadId,
        },
    },
};

pub struct UmiClient {
    url: String,
    inner: Client,
    jwt_secret: Option<EncodingKey>,
    stats_channel: UnboundedSender<Datapoint>,
}

impl UmiClient {
    pub fn new(stats_channel: UnboundedSender<Datapoint>, jwt_secret: Option<EncodingKey>) -> Self {
        let inner = Client::new();
        let port = if jwt_secret.is_some() {
            // Use authenticated port
            8551
        } else {
            // Use normal port
            8545
        };
        Self {
            url: format!("http://127.0.0.1:{port}"),
            inner,
            jwt_secret,
            stats_channel,
        }
    }

    pub async fn eth_get_balance(&self, address: Address) -> anyhow::Result<U256> {
        let method = "eth_getBalance";
        self.rpc_request(
            method,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": method,
                "params": [address, "latest"]
            }),
        )
        .await
    }

    pub async fn get_block_by_number(&self, number: u64) -> anyhow::Result<GetBlockResponse> {
        let method = "eth_getBlockByNumber";
        self.rpc_request(
            method,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": method,
                "params": [
                    format!("{number:#x}"),
                    true
                ]
            }),
        )
        .await
    }

    pub async fn engine_forkchoice_update(
        &self,
        fc_state: ForkchoiceStateV1,
        attrs: Option<PayloadAttributesV3>,
    ) -> anyhow::Result<ForkchoiceUpdatedResponseV1> {
        let method = "engine_forkchoiceUpdatedV3";
        self.rpc_request(
            method,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": method,
                "params": [
                    fc_state,
                    attrs,
                ]
            }),
        )
        .await
    }

    pub async fn engine_get_payload(
        &self,
        payload_id: PayloadId,
    ) -> anyhow::Result<GetPayloadResponseV3> {
        let method = "engine_getPayloadV3";
        self.rpc_request(
            method,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": method,
                "params": [
                    payload_id
                ]
            }),
        )
        .await
    }

    async fn rpc_request<T: DeserializeOwned>(
        &self,
        rpc_method: &'static str,
        payload: &serde_json::Value,
    ) -> anyhow::Result<T> {
        let mut request = self.inner.request(Method::POST, &self.url).json(payload);

        if let Some(key) = &self.jwt_secret {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_secs();
            let token = jsonwebtoken::encode(
                &Header::default(),
                &serde_json::json!({
                    "iat": now,
                }),
                key,
            )?;
            request = request.header("authorization", format!("Bearer {token}"));
        }

        let now = Instant::now();
        let response = request.send().await?;
        let duration = now.elapsed();
        self.stats_channel.send(Datapoint {
            rpc_method,
            timestamp: now,
            duration,
        })?;

        let response = response.error_for_status()?;
        let output: JsonRpcResponse = response.json().await?;

        if let Some(error) = output.error {
            anyhow::bail!("Error response from request {payload:?}: {error:?}");
        }

        let result: T =
            serde_json::from_value(output.result.expect("If not error then has result"))?;
        Ok(result)
    }
}
