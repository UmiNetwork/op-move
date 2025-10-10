use {crate::Msec, std::str::FromStr, test_case::test_case};

#[test_case("125.5", 125500)]
#[test_case("125", 125000)]
#[test_case("125.55554", 125555)]
fn test_msec_parses_with_three_decimal_places_and_converts_to_uint(
    msec: &str,
    expected_value: u128,
) {
    let actual_value = u128::from(Msec::from_str(msec).unwrap());

    assert_eq!(actual_value, expected_value);
}

#[test]
fn test_msec_fails_to_parse_from_non_decimal_numeric_strings() {
    let actual_error = Msec::from_str("fds45sd").unwrap_err().to_string();

    assert_eq!(actual_error, "invalid float literal");
}
