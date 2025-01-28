use crate::{
    block::{BLOCK_COLUMN_FAMILY, HEIGHT_COLUMN_FAMILY},
    transaction::TRANSACTION_COLUMN_FAMILY,
    trie::{ROOT_COLUMN_FAMILY, TRIE_COLUMN_FAMILY},
};

pub const COLUMN_FAMILIES: [&str; 5] = [
    HEIGHT_COLUMN_FAMILY,
    TRIE_COLUMN_FAMILY,
    ROOT_COLUMN_FAMILY,
    BLOCK_COLUMN_FAMILY,
    TRANSACTION_COLUMN_FAMILY,
];

#[cfg(test)]
mod tests {
    use {super::*, std::collections::HashSet};

    #[test]
    fn test_column_families_have_unique_names() {
        let expected_unique_len = COLUMN_FAMILIES.len();
        let actual_unique_len = HashSet::from(COLUMN_FAMILIES).len();

        assert_eq!(actual_unique_len, expected_unique_len);
    }
}
