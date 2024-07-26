use {crate::types::jsonrpc::JsonRpcError, std::str::FromStr};

#[derive(Debug)]
pub enum MethodName {
    ForkChoiceUpdatedV2,
    GetPayloadV2,
    NewPayloadV2,
    ForkChoiceUpdatedV3,
    GetPayloadV3,
    NewPayloadV3,
    SendRawTransaction,
}

impl FromStr for MethodName {
    type Err = JsonRpcError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "engine_forkchoiceUpdatedV3" => Ok(Self::ForkChoiceUpdatedV3),
            "engine_getPayloadV3" => Ok(Self::GetPayloadV3),
            "engine_newPayloadV3" => Ok(Self::NewPayloadV3),
            "eth_sendRawTransaction" => Ok(Self::SendRawTransaction),
            "engine_forkchoiceUpdatedV2" => Ok(Self::ForkChoiceUpdatedV2),
            "engine_getPayloadV2" => Ok(Self::GetPayloadV2),
            "engine_newPayloadV2" => Ok(Self::NewPayloadV2),
            other => Err(JsonRpcError {
                code: -32601,
                data: serde_json::Value::Null,
                message: format!("Unsupported method: {other}"),
            }),
        }
    }
}
