pub(crate) use alloy_primitives::{aliases::B2048, Address, Bytes, B256, U256, U64};

use {aptos_crypto::HashValue, move_core_types::account_address::AccountAddress};

pub(crate) trait ToEthAddress {
    fn to_eth_address(&self) -> Address;
}

impl ToEthAddress for AccountAddress {
    fn to_eth_address(&self) -> Address {
        let bytes = &self.as_slice()[12..];
        let bytes = bytes.try_into().expect("Slice should be 20 bytes");
        Address::new(bytes)
    }
}

pub(crate) trait ToMoveAddress {
    fn to_move_address(&self) -> AccountAddress;
}

impl<T: AsRef<[u8; 20]>> ToMoveAddress for T {
    fn to_move_address(&self) -> AccountAddress {
        // TODO: is there a way to make Move use 20-byte addresses?
        let mut bytes = [0; 32];
        bytes[12..32].copy_from_slice(self.as_ref());
        AccountAddress::new(bytes)
    }
}

pub(crate) trait ToB256 {
    fn to_h256(self) -> B256;
}

impl ToB256 for HashValue {
    fn to_h256(self) -> B256 {
        B256::from_slice(self.as_slice())
    }
}

pub(crate) trait ToU64 {
    fn to_u64(self) -> u64;
}

impl ToU64 for U64 {
    fn to_u64(self) -> u64 {
        self.into_limbs()[0]
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy_primitives::{address, hex},
    };

    #[test]
    fn conversion_from_hash_value_to_h256_and_back_produces_identical_value() {
        let bytes = hex!("123456789abcdef000000feedb1123535271351623521abcefdabdfc0000001f");
        let value = HashValue::from_slice(bytes).unwrap();
        let converted = value.to_h256();
        let actual_value = HashValue::from_slice(converted.as_slice()).unwrap();
        let expected_value = value;

        assert_eq!(actual_value, expected_value);
    }

    #[test]
    fn conversion_from_hash_value_to_h256_matches_original_bytes() {
        let bytes = hex!("123456789abcdef000000feedb1123535271351623521abcefdabdfc0000001f");
        let value = HashValue::from_slice(bytes).unwrap();
        let converted = value.to_h256();
        let actual_bytes = converted.as_slice();
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

    #[test]
    fn test_eth_from_move_address_match_at_intersection() {
        let move_address: AccountAddress =
            hex!("000000000000000000000000ffffffffffffffffffffffffffffffffffffffff").into();
        let eth_address = move_address.to_eth_address();
        let actual = eth_address.as_slice();
        let expected = &hex!("ffffffffffffffffffffffffffffffffffffffff");

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_eth_from_move_address_with_only_zero_bits_outside_intersection_and_back_produces_identical_value(
    ) {
        let expected_move_address: AccountAddress =
            hex!("000000000000000000000000ffffffffffffffffffffffffffffffffffffffff").into();
        let eth_address = expected_move_address.to_eth_address();
        let actual_move_address = eth_address.to_move_address();

        assert_eq!(actual_move_address, expected_move_address);
    }

    #[test]
    fn test_eth_from_move_address_with_non_zero_bits_outside_intersection_and_back_produces_different_value(
    ) {
        let expected_move_address: AccountAddress =
            hex!("100000000000000000000000ffffffffffffffffffffffffffffffffffffffff").into();
        let eth_address = expected_move_address.to_eth_address();
        let actual_move_address = eth_address.to_move_address();

        assert_ne!(actual_move_address, expected_move_address);
    }

    #[test]
    fn test_eth_address_to_move_address_round_trip() {
        // All Ethereum addresses should map to Move addresses, which
        // then map back to the same Ethereum address.

        let eth_address = {
            // Generate a random address based on the system time.
            use std::time::SystemTime;
            let ns = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let bytes = alloy_primitives::keccak256(ns.to_be_bytes());
            Address::from_word(bytes)
        };
        let move_address = eth_address.to_move_address();
        let rt_eth_address = move_address.to_eth_address();

        assert_eq!(
            eth_address, rt_eth_address,
            "Round trip eth to move address must cause no change"
        );
    }
}
