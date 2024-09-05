use {
    aptos_crypto::HashValue, ethers_core::types::H256,
    move_core_types::account_address::AccountAddress,
};

pub(crate) trait ToMoveAddress {
    fn to_move_address(&self) -> AccountAddress;
}

impl<T: AsRef<[u8; 20]>> ToMoveAddress for T {
    fn to_move_address(&self) -> AccountAddress {
        // TODO: is there a way to make Move use 32-byte addresses?
        let mut bytes = [0; 32];
        bytes[12..32].copy_from_slice(self.as_ref());
        AccountAddress::new(bytes)
    }
}

pub(crate) trait ToH256 {
    fn to_h256(self) -> H256;
}

impl ToH256 for HashValue {
    fn to_h256(self) -> H256 {
        H256::from_slice(self.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use {super::*, alloy::hex, alloy_primitives::address};

    #[test]
    fn conversion_from_hash_value_to_h256_and_back_produces_identical_value() {
        let bytes = hex!("123456789abcdef000000feedb1123535271351623521abcefdabdfc0000001f");
        let value = HashValue::from_slice(bytes).unwrap();
        let converted = value.to_h256();
        let actual_value = HashValue::from_slice(converted.as_fixed_bytes()).unwrap();
        let expected_value = value;

        assert_eq!(actual_value, expected_value);
    }

    #[test]
    fn conversion_from_hash_value_to_h256_matches_original_bytes() {
        let bytes = hex!("123456789abcdef000000feedb1123535271351623521abcefdabdfc0000001f");
        let value = HashValue::from_slice(bytes).unwrap();
        let converted = value.to_h256();
        let actual_bytes = converted.as_fixed_bytes();
        let expected_bytes = &bytes;

        assert_eq!(actual_bytes, expected_bytes);
    }

    #[test]
    fn test_move_from_eth_address_match_at_intersection() {
        let eth_address = address!("ffffffffffffffffffffffffffffffffffffffff");
        let move_address = eth_address.to_move_address();
        let actual = &move_address.into_bytes()[12..];
        let expected = &hex!("ffffffffffffffffffffffffffffffffffffffff");

        assert_eq!(actual, expected);
    }
}
