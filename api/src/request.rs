use {
    crate::{
        json_utils,
        jsonrpc::{JsonRpcError, JsonRpcResponse},
        method_name::MethodName,
    },
    metrics::{counter, gauge, histogram},
    std::time::{SystemTime, UNIX_EPOCH},
    umi_app::{ApplicationReader, CommandQueue, Dependencies},
    umi_blockchain::payload::NewPayloadId,
};

/// Tag if the serialization needs fixing.
/// The `Evm` tag is used for compatibility with EVM tooling (e.g. Forge).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerializationKind {
    Bcs,
    Evm,
}

/// Dependency injection that can change how requests are handled.
pub struct RequestModifiers<'a, A, P> {
    is_allowed: A,
    payload_id: &'a P,
    request_start_ms: u128,
    serialization_tag: SerializationKind,
}

impl<'a, A, P> RequestModifiers<'a, A, P>
where
    A: Fn(&MethodName) -> bool,
    P: NewPayloadId,
{
    pub fn new(
        is_allowed: A,
        payload_id: &'a P,
        serialization_tag: SerializationKind,
        request_start_ms: u128,
    ) -> Self {
        Self {
            is_allowed,
            payload_id,
            request_start_ms,
            serialization_tag,
        }
    }
}

#[tracing::instrument(level = "debug", skip(queue, modifiers, app))]
pub async fn handle<'reader, A, P>(
    request: serde_json::Value,
    queue: CommandQueue,
    modifiers: RequestModifiers<'_, A, P>,
    app: ApplicationReader<'reader, impl Dependencies<'reader>>,
) -> JsonRpcResponse
where
    A: Fn(&MethodName) -> bool,
    P: NewPayloadId,
{
    let id = json_utils::get_field(&request, "id");
    let jsonrpc = json_utils::get_field(&request, "jsonrpc");

    match inner_handle_request(request, queue, modifiers, &app).await {
        Ok(r) => JsonRpcResponse {
            id,
            jsonrpc,
            result: Some(r),
            error: None,
        },
        Err(e) => JsonRpcResponse {
            id,
            jsonrpc,
            result: None,
            error: Some(e),
        },
    }
}

async fn inner_handle_request<'reader, A, P>(
    request: serde_json::Value,
    queue: CommandQueue,
    modifiers: RequestModifiers<'_, A, P>,
    app: &ApplicationReader<'reader, impl Dependencies<'reader>>,
) -> Result<serde_json::Value, JsonRpcError>
where
    A: Fn(&MethodName) -> bool,
    P: NewPayloadId,
{
    use {crate::methods::*, MethodName::*};

    let RequestModifiers {
        is_allowed,
        payload_id,
        serialization_tag,
        request_start_ms,
    } = modifiers;
    let method_str = json_utils::get_field(&request, "method")
        .as_str()
        .ok_or(JsonRpcError::missing_method(request.clone()))?
        .to_owned();
    let method: MethodName = method_str.parse()?;

    if !is_allowed(&method) {
        return Err(JsonRpcError::missing_method(request));
    }

    let t0 = std::time::Instant::now();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System clock adjustment can't invalidate Unix epoch")
        .as_millis();
    let signed_skew_ms = (now as i128) - (request_start_ms as i128);
    let nonneg_queue_ms = signed_skew_ms.max(0) as u128;

    counter!("rpc_requests_total", "method" => method_str.clone()).increment(1);
    gauge!("rpc_inflight_requests", "method" => method_str.clone()).increment(1.0);
    histogram!("ingress_to_app_seconds", "method" => method_str.clone())
        .record(nonneg_queue_ms as f64 / 1000.0);
    gauge!("ingress_time_skew_seconds", "method" => method_str.clone())
        .set(signed_skew_ms as f64 / 1000.0);

    let resp = match method {
        ForkChoiceUpdatedV3 => {
            forkchoice_updated::execute_v3(request, queue, app, payload_id).await
        }
        GetPayloadV3 => get_payload::execute_v3(request, app).await,
        NewPayloadV3 => new_payload::execute_v3(request, app).await,
        // Despite having a new field in V4 (execution requests), it's always empty and `op-geth`
        // itself falls back to V3 here
        GetPayloadV4 => get_payload::execute_v3(request, app).await,
        NewPayloadV4 => new_payload::execute_v4(request, app).await,

        SendRawTransaction => {
            send_raw_transaction::execute(request, queue, serialization_tag).await
        }
        ChainId => chain_id::execute(request, app, chain_id::ChainIdFormat::Hex).await,
        NetVersion => chain_id::execute(request, app, chain_id::ChainIdFormat::Decimal).await,
        GetBalance => get_balance::execute(request, app).await,
        GetCode => get_code::execute(request, app).await,
        GetNonce => get_nonce::execute(request, app).await,
        GetStorageAt => get_storage_at::execute(request, app).await,
        GetTransactionByHash => get_transaction_by_hash::execute(request, app).await,
        GetBlockByHash => get_block_by_hash::execute(request, app).await,
        GetBlockByNumber => get_block_by_number::execute(request, app).await,
        GetModule => get_module::execute(request, app).await,
        GetResource => get_resource::execute(request, app).await,
        GetTableItem => get_table_item::execute(request, app).await,
        BlockNumber => block_number::execute(request, app).await,
        FeeHistory => fee_history::execute(request, app).await,
        EstimateGas => estimate_gas::execute(request, app, serialization_tag).await,
        Call => call::execute(request, app, serialization_tag).await,
        TransactionReceipt => get_transaction_receipt::execute(request, app).await,
        GetProof => get_proof::execute(request, app).await,
        GasPrice => gas_price::execute(request, app).await,
        MaxPriorityFeePerGas => max_priority_fee_per_gas::execute(request, app).await,
        ClientVersion => client_version::execute(request, app).await,
        ListModules => list_modules::execute(request, app).await,
        ListResources => list_resources::execute(request, app).await,
        // eth_accounts: Returns a list of addresses owned by client.
        // We do not support owning addresses in our client, so always return an empty list.
        Accounts => Ok(serde_json::Value::Array(Vec::new())),
    };

    gauge!("rpc_inflight_requests", "method" => method_str.clone()).decrement(1.0);
    let _ = resp.as_ref().map_err(|e| {
        let err_code = format!("{}", e.code);
        counter!("rpc_errors_total", "method" => method_str.clone(), "code" => err_code)
            .increment(1);
    });
    histogram!("rpc_request_duration_seconds", "method" => method_str)
        .record(t0.elapsed().as_secs_f64());

    resp
}
