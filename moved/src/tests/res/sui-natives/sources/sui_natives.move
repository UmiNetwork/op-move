module 0x8fd379246834eac74b8419ffda202cf8051f7a03::sui_natives {
    use 0x21::hash::sha3_256;

    /// Test out hashing native functions
    entry fun hashing() {
        assert!(sha3_256(b"hello") == x"3338be694f50c5f338814986cdf0686453a888b84f424d792af4b9202398f392", 0);
    }
}
