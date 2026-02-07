use {
    crate::{json_utils, jsonrpc::JsonRpcError},
    alloy::primitives::Address,
    umi_app::{Command, CommandQueue},
    umi_shared::primitives::U256,
};

pub async fn execute(
    request: serde_json::Value,
    queue: CommandQueue,
) -> Result<serde_json::Value, JsonRpcError> {
    let params = json_utils::get_params_list(&request);
    let (address, amount) = match params {
        [] => Err(JsonRpcError::not_enough_params_error(request)),
        [x, y] => {
            let address: Address = json_utils::deserialize(x)?;
            let amount: U256 = json_utils::deserialize(y)?;
            Ok((address, amount))
        }
        _ => Err(JsonRpcError::too_many_params_error(request)),
    }?;
    let msg = Command::FaucetDeposit {
        address,
        amount: amount.saturating_to(),
    };
    queue.send(msg).await;
    Ok(serde_json::Value::String("Success".into()))
}
