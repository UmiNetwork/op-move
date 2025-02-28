script {
    use 0x1::eth_token::get_metadata;
    use 0x1::fungible_asset_u256;
    use 0x1::primary_fungible_store_u256::ensure_primary_store_exists;
    use 0x1::signer::address_of;
    use 0x4200000000000000000000000000000000000007::l2_cross_domain_messenger::send_message;

    fun withdraw(owner: &signer, target: address, amount: u256) {
        let owner_addr = address_of(owner);
        let eth_metadata = get_metadata();
        let store = ensure_primary_store_exists(owner_addr, eth_metadata);
        let value = fungible_asset_u256::withdraw(owner, store, amount);
        send_message(owner, target, vector[], 21000, value);
    }
}
