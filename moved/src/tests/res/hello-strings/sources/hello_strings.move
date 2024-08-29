module 0x8fd379246834eac74b8419ffda202cf8051f7a03::hello_strings {
    use 0x1::string::{Self, String};

    public entry fun main(name: String) {
        let greeting = string::utf8(b"Hello, ");
        string::append(&mut greeting, name);
    }
}
