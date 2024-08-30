use {
    crate::iter::{PairIteratorExt, PairOrSingle},
    alloy_primitives::{keccak256, B256},
};

/// This trait is defined by a single [`Self::merkle_root`] operation.
pub trait MerkleRootExt {
    /// Calculates a root hash of merkle tree.
    ///
    /// See [`root`] for the algorithm documentation.
    fn merkle_root(self) -> B256;
}

impl<I: ExactSizeIterator<Item = B256>> MerkleRootExt for I {
    fn merkle_root(self) -> B256 {
        root(self)
    }
}

/// Calculates a root hash of a merkle tree built from `leaves`. The `leaves` are considered hashes
/// of the actual values.
///
/// The algorithm has a time complexity of *O*(*n* \* log(*n*)) and space complexity of *O*(*1*).
pub fn root(leaves: impl ExactSizeIterator<Item = B256>) -> B256 {
    let size = leaves.len().max(1);
    let nodes: Box<dyn Iterator<Item = B256>> = Box::new(leaves);
    let levels = 0..=size.ilog2();

    levels
        .fold(nodes, |nodes, _| {
            Box::new(nodes.pair().map(PairOrSingle::concat_and_hash_pair))
        })
        .next()
        .unwrap_or(B256::ZERO)
}

impl PairOrSingle<B256> {
    /// If [`Self`] is a pair of two values, it concatenates them and hashes using [`keccak256`].
    /// Otherwise, it is left intact.
    fn concat_and_hash_pair(self) -> B256 {
        match self {
            PairOrSingle::Pair(a, b) => keccak256([a, b].concat()),
            PairOrSingle::Single(hash) => hash,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_of_empty_tree_is_zero() {
        let expected_root = B256::ZERO;
        let actual_root = root([].into_iter());

        assert_eq!(actual_root, expected_root);
    }

    #[test]
    fn test_root_of_single_node_tree_is_hash_of_the_node() {
        let hash1 = B256::from([1; 32]);
        let expected_root = hash1;
        let actual_root = [hash1].into_iter().merkle_root();

        assert_eq!(actual_root, expected_root);
    }

    #[test]
    fn test_root_of_two_node_tree_is_hash_of_concatenated_hashes() {
        let hash1 = B256::from([1; 32]);
        let hash2 = B256::from([2; 32]);
        let expected_root = keccak256([hash1, hash2].concat());
        let actual_root = [hash1, hash2].into_iter().merkle_root();

        assert_eq!(actual_root, expected_root);
    }

    #[test]
    fn test_root_of_three_node_tree_is_two_node_tree_hash_concatenated_to_hash_of_last_node() {
        let hash1 = B256::from([1; 32]);
        let hash2 = B256::from([2; 32]);
        let hash3 = B256::from([3; 32]);
        let expected_root = keccak256([keccak256([hash1, hash2].concat()), hash3].concat());
        let actual_root = [hash1, hash2, hash3].into_iter().merkle_root();

        assert_eq!(actual_root, expected_root);
    }

    #[test]
    fn test_root_of_four_node_tree_is_hash_of_two_two_node_tree_hashes_concatenated() {
        let hash1 = B256::from([1; 32]);
        let hash2 = B256::from([2; 32]);
        let hash3 = B256::from([3; 32]);
        let hash4 = B256::from([4; 32]);
        let expected_root = keccak256(
            [
                keccak256([hash1, hash2].concat()),
                keccak256([hash3, hash4].concat()),
            ]
            .concat(),
        );
        let actual_root = [hash1, hash2, hash3, hash4].into_iter().merkle_root();

        assert_eq!(actual_root, expected_root);
    }

    #[test]
    fn test_root_of_five_node_tree_is_hash_of_four_node_tree_and_last_leaf_concatenated() {
        let hash1 = B256::from([1; 32]);
        let hash2 = B256::from([2; 32]);
        let hash3 = B256::from([3; 32]);
        let hash4 = B256::from([4; 32]);
        let hash5 = B256::from([5; 32]);
        let expected_root = keccak256(
            [
                keccak256(
                    [
                        keccak256([hash1, hash2].concat()),
                        keccak256([hash3, hash4].concat()),
                    ]
                    .concat(),
                ),
                hash5,
            ]
            .concat(),
        );
        let actual_root = [hash1, hash2, hash3, hash4, hash5]
            .into_iter()
            .merkle_root();

        assert_eq!(actual_root, expected_root);
    }
}
