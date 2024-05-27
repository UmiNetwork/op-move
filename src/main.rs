use {
    self::types::{
        jsonrpc::{JsonRpcError, JsonRpcResponse},
        method_name::MethodName,
    },
    std::net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    warp::Filter,
};

mod json_utils;
mod methods;
mod types;

#[tokio::main]
async fn main() {
    let server_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 8545));

    let json_rpc = warp::body::json().then(|request: serde_json::Value| async move {
        let response = handle_request(request).await;
        warp::reply::json(&response)
    });

    let route = json_rpc.or(warp::any().map(warp::reply));
    warp::serve(route).run(server_addr).await;
}

async fn handle_request(request: serde_json::Value) -> JsonRpcResponse {
    let id = json_utils::get_field(&request, "id");
    let jsonrpc = json_utils::get_field(&request, "jsonrpc");
    let result = match inner_handle_request(request).await {
        Ok(r) => r,
        Err(e) => {
            return JsonRpcResponse {
                id,
                jsonrpc,
                result: None,
                error: Some(e),
            }
        }
    };
    JsonRpcResponse {
        id,
        jsonrpc,
        result: Some(result),
        error: None,
    }
}

async fn inner_handle_request(
    request: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let method: MethodName = match json_utils::get_field(&request, "method") {
        serde_json::Value::String(m) => m.parse()?,
        _ => {
            return Err(JsonRpcError {
                code: -32601,
                data: serde_json::Value::Null,
                message: "Invalid/missing method".into(),
            });
        }
    };

    match method {
        MethodName::ForkChoiceUpdatedV3 => methods::forkchoice_updated::execute_v3(request),
        MethodName::GetPayloadV3 => methods::get_payload::execute_v3(request),
        MethodName::NewPayloadV3 => methods::new_payload::execute_v3(request),
        MethodName::ForkChoiceUpdatedV2 => todo!(),
        MethodName::GetPayloadV2 => todo!(),
        MethodName::NewPayloadV2 => todo!(),
    }
}
