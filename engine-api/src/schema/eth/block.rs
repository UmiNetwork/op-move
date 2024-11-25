pub use alloy::eips::BlockNumberOrTag;

use {
    moved::types::state::{BlockResponse, RpcBlock},
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetBlockResponse(pub RpcBlock);

impl From<BlockResponse> for GetBlockResponse {
    fn from(value: BlockResponse) -> Self {
        Self(value.0)
    }
}
