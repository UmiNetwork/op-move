mod convert;
mod eth_trie;
mod in_memory;
mod model;
mod response;
#[cfg(any(feature = "test-doubles", test))]
mod test_doubles;
#[cfg(test)]
mod tests;

#[cfg(any(feature = "test-doubles", test))]
pub use test_doubles::MockStateQueries;
pub use {
    eth_trie::EthTrieStateQueries,
    in_memory::InMemoryStateQueries,
    model::{
        Balance, BlockHeight, HashToStateRootIndex, Nonce, ProofResponse, StateQueries,
        StorageProof, Version, evm_storage_root_from_trie_and_resolver,
        proof_from_trie_and_resolver,
    },
    response::*,
};
