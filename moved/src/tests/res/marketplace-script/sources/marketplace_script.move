script {
    use 0x1::eth_token::get_metadata;
    use 0x1::fungible_asset_u256;
    use 0x1::primary_fungible_store_u256::ensure_primary_store_exists;
    use 0x1::signer::address_of;
    use 0x8fd379246834eac74b8419ffda202cf8051f7a03::marketplace::buy;

    fun buy_something(market: address, index: u64, amount: u256, owner: &signer) {
        let owner_addr = address_of(owner);
        let eth_metadata = get_metadata();
        let store = ensure_primary_store_exists(owner_addr, eth_metadata);
        let payment = fungible_asset_u256::withdraw(owner, store, amount);
        buy(market, index, payment)
    }
}