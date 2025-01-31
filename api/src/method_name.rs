use {crate::jsonrpc::JsonRpcError, std::str::FromStr};

#[derive(Debug)]
pub enum MethodName {
    ForkChoiceUpdatedV3,
    GetPayloadV3,
    NewPayloadV3,
    SendRawTransaction,
    ChainId,
    GetBalance,
    GetBlockByHash,
    GetBlockByNumber,
    GetTransactionByHash,
    GetNonce,
    BlockNumber,
    FeeHistory,
    EstimateGas,
    Call,
    TransactionReceipt,
    GetProof,
}

impl FromStr for MethodName {
    type Err = JsonRpcError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "engine_forkchoiceUpdatedV3" => Self::ForkChoiceUpdatedV3,
            "engine_getPayloadV3" => Self::GetPayloadV3,
            "engine_newPayloadV3" => Self::NewPayloadV3,
            "eth_chainId" => Self::ChainId,
            "eth_getBalance" => Self::GetBalance,
            "eth_getTransactionCount" => Self::GetNonce,
            "eth_getTransactionByHash" => Self::GetTransactionByHash,
            "eth_getBlockByHash" => Self::GetBlockByHash,
            "eth_getBlockByNumber" => Self::GetBlockByNumber,
            "eth_feeHistory" => Self::FeeHistory,
            "eth_blockNumber" => Self::BlockNumber,
            "eth_sendRawTransaction" => Self::SendRawTransaction,
            "eth_estimateGas" => Self::EstimateGas,
            "eth_call" => Self::Call,
            "eth_getTransactionReceipt" => Self::TransactionReceipt,
            "eth_getProof" => Self::GetProof,
            other => {
                return Err(JsonRpcError::without_data(
                    -32601,
                    format!("Unsupported method: {other}"),
                ))
            }
        })
    }
}
