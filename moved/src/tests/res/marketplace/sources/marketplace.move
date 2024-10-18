module 0x8fd379246834eac74b8419ffda202cf8051f7a03::marketplace {
    use 0x1::error;
    use 0x1::eth_token::get_metadata;
    use 0x1::fungible_asset::{Self, FungibleAsset};
    use 0x1::primary_fungible_store::ensure_primary_store_exists;
    use 0x1::signer::address_of;
    use 0x1::string::String;
    use 0x1::vector::{push_back, remove};

    const EINCORRECT_PAYMENT_AMOUNT: u64 = 1;

    struct Listing has drop, store {
        price: u64,
        thing: String,
        seller: address,
    }

    struct Listings has key {
        inner: vector<Listing>,
    }

    // Initialize the marketplace
    public entry fun init(owner: &signer) {
        let owner_addr = address_of(owner);
        if (!exists<Listings>(owner_addr)) {
            let listings = Listings { inner: vector[] };
            move_to(owner, listings);
        }
    }

    // List something to sell
    public entry fun list(
        market: address,
        price: u64,
        thing: String,
        seller: &signer
    ) acquires Listings {
        let listings = borrow_global_mut<Listings>(market);
        let seller = address_of(seller);
        let new_entry = Listing {
            price,
            thing,
            seller,
        };
        push_back(&mut listings.inner, new_entry);
    }

    // Buy something that is currently listed
    public fun buy(
        market: address,
        index: u64,
        payment: FungibleAsset
    ) acquires Listings {
        let listings = borrow_global_mut<Listings>(market);
        let entry = remove(&mut listings.inner, index);

        assert!(
            fungible_asset::amount(&payment) == entry.price,
            error::invalid_argument(EINCORRECT_PAYMENT_AMOUNT)
        );

        let eth_metadata = get_metadata();
        let store = ensure_primary_store_exists(entry.seller, eth_metadata);
        fungible_asset::deposit(store, payment);
    }
}