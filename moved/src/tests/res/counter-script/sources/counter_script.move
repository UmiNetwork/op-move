script {
    use 0x1::signer;
    use 0x8fd379246834eac74b8419ffda202cf8051f7a03::counter::{publish, get_count, increment};

    fun main(owner: &signer, start_count: u64) {
        let owner_address = signer::address_of(owner);

        // Create the counter
        publish(owner, start_count);

        // Confirm the counter starts with the right value
        let value = get_count(owner_address);
        assert!(value == start_count, 0);

        // Increment the value and check it again
        increment(owner_address);
        let new_value = get_count(owner_address);
        assert!(new_value == start_count + 1, 1);
    }
}
