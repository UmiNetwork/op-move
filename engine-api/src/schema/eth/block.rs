pub use alloy::eips::BlockNumberOrTag;

use {
    alloy::rpc,
    moved::types::state::BlockResponse,
    serde::{Deserialize, Serialize},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetBlockResponse(pub rpc::types::Block<rpc::types::Transaction>);

impl From<BlockResponse> for GetBlockResponse {
    fn from(value: BlockResponse) -> Self {
        Self(value)
    }
}
