use self::types::{
    jsonrpc::{JsonRpcError, JsonRpcResponse},
    method_name::MethodName,
};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::str::from_utf8;
use warp::hyper::{body::Bytes, Body, Response};
use warp::path::FullPath;
use warp::{Filter, Rejection};
use warp_reverse_proxy::{extract_request_data_filter, proxy_to_and_forward_response, Headers};
use warp_reverse_proxy::{Method, QueryParameters};

mod json_utils;
mod methods;
mod types;

#[tokio::main]
async fn main() {
    let http_server_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 8545));
    let http_route = warp::any().and(extract_request_data_filter()).and_then(
        |path, query, method, headers, body: Bytes| {
            println!("Http: {}", from_utf8(&body).expect("Conversion error"));
            proxy(path, query, method, headers, body, "9545")
        },
    );

    let auth_server_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 8551));
    let auth_route = warp::any().and(extract_request_data_filter()).and_then(
        |path, query, method, headers, body: Bytes| {
            println!("Auth: {}", from_utf8(&body).expect("Conversion error"));
            proxy(path, query, method, headers, body, "9551")
        },
    );

    tokio::join!(
        warp::serve(http_route).run(http_server_addr),
        warp::serve(auth_route).run(auth_server_addr),
    );
}

async fn proxy(
    path: FullPath,
    query: QueryParameters,
    method: Method,
    headers: Headers,
    body: Bytes,
    port: &str,
) -> Result<Response<Body>, Rejection> {
    proxy_to_and_forward_response(
        format!("http://0.0.0.0:{}", port),
        "".to_string(),
        path,
        query,
        method,
        headers,
        body,
    )
    .await
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
