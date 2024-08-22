use alloy_primitives::{keccak256, B256};

pub fn merkle_root(leaves: impl ExactSizeIterator<Item = B256>) -> B256 {
    let size = leaves.len().max(1);
    let nodes: Box<dyn Iterator<Item = B256>> = Box::new(leaves);
    let levels = 0..=size.ilog2();

    levels
        .fold(nodes, |nodes, _| Box::new(nodes.pair().map(Pair::combine)))
        .next()
        .unwrap_or(B256::ZERO)
}

trait PairIteratorExt<T, I: Iterator<Item = T>> {
    fn pair(self) -> PairIterator<T, I>;
}

impl<T, I: Iterator<Item = T>> PairIteratorExt<T, I> for I {
    fn pair(self) -> PairIterator<T, I> {
        PairIterator::new(self)
    }
}

impl Pair<B256> {
    pub fn combine(self) -> B256 {
        match self {
            Pair::Full(a, b) => keccak256(&[a, b].concat()),
            Pair::Partial(hash) => hash,
        }
    }
}

pub enum Pair<T> {
    Full(T, T),
    Partial(T),
}

pub struct PairIterator<T, I: Iterator<Item = T>> {
    inner: I,
}

impl<T, I: Iterator<Item = T>> PairIterator<T, I> {
    pub fn new(inner: I) -> Self {
        Self { inner }
    }
}

impl<T, I> Iterator for PairIterator<T, I>
where
    I: Iterator<Item = T>,
{
    type Item = Pair<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(first) = self.inner.next() {
            Some(if let Some(second) = self.inner.next() {
                Pair::Full(first, second)
            } else {
                Pair::Partial(first)
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_of_empty_tree_is_zero() {
        let expected_root = B256::ZERO;
        let actual_root = merkle_root([].into_iter());

        assert_eq!(actual_root, expected_root);
    }

    #[test]
    fn test_root_of_single_node_tree_is_hash_of_the_node() {
        let one = B256::from([1; 32]);
        let expected_root = one;
        let actual_root = merkle_root([one].into_iter());

        assert_eq!(actual_root, expected_root);
    }

    #[test]
    fn test_root_of_two_node_tree_is_hash_of_concatenated_hashes() {
        let one = B256::from([1; 32]);
        let two = B256::from([2; 32]);
        let expected_root = keccak256(&[one, two].concat());
        let actual_root = merkle_root([one, two].into_iter());

        assert_eq!(actual_root, expected_root);
    }

    #[test]
    fn test_root_of_three_node_tree_is_two_node_tree_hash_concatenated_to_hash_of_last_node() {
        let one = B256::from([1; 32]);
        let two = B256::from([2; 32]);
        let three = B256::from([3; 32]);
        let expected_root = keccak256(&[keccak256(&[one, two].concat()), three].concat());
        let actual_root = merkle_root([one, two, three].into_iter());

        assert_eq!(actual_root, expected_root);
    }

    #[test]
    fn test_root_of_four_node_tree_is_hash_of_two_two_node_tree_hashes_concatenated() {
        let one = B256::from([1; 32]);
        let two = B256::from([2; 32]);
        let three = B256::from([3; 32]);
        let four = B256::from([4; 32]);
        let expected_root = keccak256(
            &[
                keccak256(&[one, two].concat()),
                keccak256(&[three, four].concat()),
            ]
            .concat(),
        );
        let actual_root = merkle_root([one, two, three, four].into_iter());

        assert_eq!(actual_root, expected_root);
    }

    #[test]
    fn test_root_of_five_node_tree_is_hash_of_four_node_tree_and_last_leaf_concatenated() {
        let one = B256::from([1; 32]);
        let two = B256::from([2; 32]);
        let three = B256::from([3; 32]);
        let four = B256::from([4; 32]);
        let five = B256::from([5; 32]);
        let expected_root = keccak256(
            &[
                keccak256(
                    &[
                        keccak256(&[one, two].concat()),
                        keccak256(&[three, four].concat()),
                    ]
                    .concat(),
                ),
                five,
            ]
            .concat(),
        );
        let actual_root = merkle_root([one, two, three, four, five].into_iter());

        assert_eq!(actual_root, expected_root);
    }
}
