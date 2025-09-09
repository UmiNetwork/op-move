use {
    alloy::primitives::{Address, U256},
    jsonwebtoken::{EncodingKey, Header},
    reqwest::{Client, Method},
    serde::de::DeserializeOwned,
    std::time::SystemTime,
    umi_api::jsonrpc::JsonRpcResponse,
};

pub struct UmiClient {
    url: String,
    inner: Client,
    jwt_secret: Option<EncodingKey>,
}

impl UmiClient {
    pub fn new(jwt_secret: Option<EncodingKey>) -> Self {
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
        }
    }

    pub async fn eth_get_balance(&self, address: Address) -> anyhow::Result<U256> {
        self.rpc_request(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getBalance",
            "params": [address, "latest"]
        }))
        .await
    }

    async fn rpc_request<T: DeserializeOwned>(
        &self,
        payload: &serde_json::Value,
    ) -> anyhow::Result<T> {
        let mut request = self.inner.request(Method::POST, &self.url).json(payload);

        if let Some(key) = &self.jwt_secret {
            let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
            let token = jsonwebtoken::encode(
                &Header::default(),
                &serde_json::json!({
                    "iat": now,
                }),
                key,
            )?;
            request = request.header("authorization", format!("Bearer {token}"));
        }

        let response = request.send().await?.error_for_status()?;
        let output: JsonRpcResponse = response.json().await?;

        if let Some(error) = output.error {
            anyhow::bail!("Error response from request {payload:?}: {error:?}");
        }

        let result: T =
            serde_json::from_value(output.result.expect("If not error then has result"))?;
        Ok(result)
    }
}
