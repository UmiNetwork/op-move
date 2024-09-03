//! Based on https://aptos.dev/en/build/smart-contracts/objects

module 0x8fd379246834eac74b8419ffda202cf8051f7a03::object_playground {
    use 0x1::signer;
    use std::string::{Self, String};
    use 0x1::object::{Self, Object, ObjectCore};

    // Not authorized!
    const E_NOT_AUTHORIZED: u64 = 1;
 
    struct MyStruct1 has key {
        message: String,
    }
  
    struct MyStruct2 has key {
        message: String,
    }
 
    entry fun create_and_transfer(caller: &signer, destination: address) {
        // Create object
        let caller_address = signer::address_of(caller);
        let constructor_ref = object::create_object(caller_address);
        let object_signer = object::generate_signer(&constructor_ref);

        // Set up the object by creating 2 resources in it
        move_to(&object_signer, MyStruct1 {
            message: string::utf8(b"hello")
        });
        move_to(&object_signer, MyStruct2 {
            message: string::utf8(b"world")
        });

        // Transfer to destination
        let object = object::object_from_constructor_ref<ObjectCore>(
            &constructor_ref
        );
        object::transfer(caller, object, destination);
    }

    entry fun check_struct1_owner(caller: &signer, object: Object<MyStruct1>) {
        check_owner_is_caller(caller, object);
    }

    entry fun check_struct2_owner(caller: &signer, object: Object<MyStruct2>) {
        check_owner_is_caller(caller, object);
    }

    fun check_owner_is_caller<T: key>(caller: &signer, object: Object<T>) {
        assert!(
            object::is_owner(object, signer::address_of(caller)),
            E_NOT_AUTHORIZED
        );
    }
}
