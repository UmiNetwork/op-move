/// From https://aptos.dev/move/book/global-storage-operators#example-counter
module 0x8fd379246834eac74b8419ffda202cf8051f7a03::counter {
    use 0x1::aptos_hash::keccak256;
    use 0x1::hash::sha3_256;

    /// Resource that wraps an integer counter
    struct Counter has key { i: u64 }

    /// Publish a `Counter` resource with value `i` under the given `account`
    public entry fun publish(account: &signer, i: u64) {
      // Test out some native functions
      assert!(sha3_256(b"hello") == x"3338be694f50c5f338814986cdf0686453a888b84f424d792af4b9202398f392", 0);
      assert!(keccak256(b"hello") == x"1c8aff950685c2ed4bc3174f3472287b56d9517b9c948127319a09a7a36deac8", 1);

      // "Pack" (create) a Counter resource. This is a privileged operation that
      // can only be done inside the module that declares the `Counter` resource
      move_to(account, Counter { i })
    }

    /// Read the value in the `Counter` resource stored at `addr`
    public fun get_count(addr: address): u64 acquires Counter {
        borrow_global<Counter>(addr).i
    }

    /// Increment the value of `addr`'s `Counter` resource
    public entry fun increment(addr: address) acquires Counter {
        let c_ref = &mut borrow_global_mut<Counter>(addr).i;
        *c_ref = *c_ref + 1
    }

    /// Return `true` if `addr` contains a `Counter` resource
    public fun counter_exists(addr: address): bool {
        exists<Counter>(addr)
    }
}
