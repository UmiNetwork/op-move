use crate::{block, payload, receipt, state, transaction, trie};

pub const DATABASES: [&str; 9] = [
    block::DB,
    block::HEIGHT_DB,
    state::DB,
    state::HEIGHT_DB,
    trie::DB,
    trie::ROOT_DB,
    transaction::DB,
    receipt::DB,
    payload::DB,
];

#[cfg(test)]
mod tests {
    use {super::*, std::collections::HashSet};

    #[test]
    fn test_databases_have_unique_names() {
        let expected_unique_len = DATABASES.len();
        let actual_unique_len = HashSet::from(DATABASES).len();

        assert_eq!(actual_unique_len, expected_unique_len);
    }
}
