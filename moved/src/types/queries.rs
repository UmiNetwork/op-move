use {
    alloy::{
        primitives::Bytes,
        rlp::BufMut,
        rpc::types::{EIP1186AccountProofResponse, EIP1186StorageProof},
    },
    borsh::{BorshDeserialize, BorshSerialize},
    jmt::{
        proof::{SparseMerkleProof, INTERNAL_DOMAIN_SEPARATOR, LEAF_DOMAIN_SEPARATOR},
        KeyHash, ValueHash,
    },
    sha3::Keccak256,
};

pub type ProofResponse = EIP1186AccountProofResponse;
pub type StorageProof = EIP1186StorageProof;

/// Similar to `jmt::SparseMerkleProof`, but with public fields so that the
/// proof can be transformed to suit our needs.
#[derive(Debug, PartialEq, Eq, Clone, BorshDeserialize, BorshSerialize)]
pub struct JmtProof {
    leaf: Option<JmtLeaf>,
    siblings: Vec<JmtNode>,
}

impl<'a> From<&'a SparseMerkleProof<Keccak256>> for JmtProof {
    fn from(value: &'a SparseMerkleProof<Keccak256>) -> Self {
        // We have to do this conversion via borsh serialization because
        // SparseMerkleProof has private fields with no getters.
        let encoded = borsh::to_vec(value).expect("Proof must serialize");
        borsh::from_slice(&encoded).expect("JmtProof and SparseMerkleProof have the same layout")
    }
}

impl<'a> From<&'a JmtProof> for SparseMerkleProof<Keccak256> {
    fn from(value: &'a JmtProof) -> Self {
        // We have to do this conversion via borsh serialization because
        // SparseMerkleProof has private fields and no constructor.
        let encoded = borsh::to_vec(value).expect("Proof must serialize");
        borsh::from_slice(&encoded).expect("JmtProof and SparseMerkleProof have the same layout")
    }
}

impl JmtProof {
    // This function creates a list of RLP-encoded nodes in the proof. Thus it is
    // compatible with the format needed in EIP-1186 Merkle proofs. It is intentional
    // that each node is encoded as a 3-element list. This encoding ensures it cannot
    // be confused with a normal Ethereum Merkle proof because MPT nodes are either length
    // 17 (branch nodes consisting of 16 siblings, together with an optional value) or
    // length 2 (value nodes or extension nodes).
    pub fn encode_for_response(&self) -> Vec<Bytes> {
        let mut result = Vec::with_capacity(1 + self.siblings.len());

        let mut leaf_encoding = Vec::new();
        self.leaf
            .as_ref()
            .map(|leaf| leaf.rlp_encode(&mut leaf_encoding))
            .unwrap_or_else(|| {
                alloy_rlp::encode_list::<&[u8], &[u8]>(
                    &[LEAF_DOMAIN_SEPARATOR, &[], &[]],
                    &mut leaf_encoding,
                )
            });
        result.push(leaf_encoding.into());

        for sibling in &self.siblings {
            let mut out = Vec::new();
            sibling.rlp_encode(&mut out);
            result.push(out.into());
        }

        result
    }
}

#[derive(Debug, PartialEq, Eq, Clone, BorshDeserialize, BorshSerialize)]
pub enum JmtNode {
    Null,
    Internal(JmtInternalNode),
    Leaf(JmtLeaf),
}

impl JmtNode {
    pub fn rlp_encode(&self, out: &mut impl BufMut) {
        match self {
            Self::Internal(node) => node.rlp_encode(out),
            Self::Leaf(node) => node.rlp_encode(out),
            Self::Null => alloy_rlp::encode_list::<&[u8], &[u8]>(&[b"NULL", &[], &[]], out),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, BorshDeserialize, BorshSerialize)]
pub struct JmtLeaf {
    pub key_hash: KeyHash,
    pub value_hash: ValueHash,
}

impl JmtLeaf {
    pub fn rlp_encode(&self, out: &mut impl BufMut) {
        alloy_rlp::encode_list::<&[u8], &[u8]>(
            &[
                LEAF_DOMAIN_SEPARATOR,
                self.key_hash.0.as_slice(),
                self.value_hash.0.as_slice(),
            ],
            out,
        );
    }
}

#[derive(Debug, PartialEq, Eq, Clone, BorshDeserialize, BorshSerialize)]
pub struct JmtInternalNode {
    pub left: [u8; 32],
    pub right: [u8; 32],
}

impl JmtInternalNode {
    pub fn rlp_encode(&self, out: &mut impl BufMut) {
        alloy_rlp::encode_list::<&[u8], &[u8]>(
            &[
                INTERNAL_DOMAIN_SEPARATOR,
                self.left.as_slice(),
                self.right.as_slice(),
            ],
            out,
        );
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            move_execution::evm_native::{type_utils::account_info_struct_tag, EVM_NATIVE_ADDRESS},
            primitives::KeyHashable,
        },
        aptos_types::state_store::state_key::StateKey,
        jmt::{mock::MockTreeStore, JellyfishMerkleTree},
    };

    #[test]
    fn test_jmt_proof_roundtrip() {
        let store = MockTreeStore::default();
        let struct_tag = account_info_struct_tag(&Default::default());
        let key = StateKey::resource(&EVM_NATIVE_ADDRESS, &struct_tag).unwrap();
        let version = 0;
        let value_set = vec![(key.key_hash(), None)];
        let trie = JellyfishMerkleTree::<_, Keccak256>::new(&store);
        let (_, update) = trie.put_value_set(value_set, version).unwrap();
        store.write_tree_update_batch(update).unwrap();

        let trie = JellyfishMerkleTree::<_, Keccak256>::new(&store);
        let (_, proof) = trie.get_with_proof(key.key_hash(), version).unwrap();

        let converted: JmtProof = (&proof).into();
        let round_trip: SparseMerkleProof<Keccak256> = (&converted).into();

        assert_eq!(proof, round_trip);
    }
}
