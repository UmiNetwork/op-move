module 0x8fd379246834eac74b8419ffda202cf8051f7a03::natives {
    use 0x1::aptos_hash::keccak256;
    use 0x1::hash::sha3_256;

    /// Test out hashing native functions
    public entry fun hashing() {
        assert!(keccak256(b"hello") == x"1c8aff950685c2ed4bc3174f3472287b56d9517b9c948127319a09a7a36deac8", 1);
        assert!(sha3_256(b"hello") == x"3338be694f50c5f338814986cdf0686453a888b84f424d792af4b9202398f392", 0);
    }
}
