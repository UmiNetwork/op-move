module 0x8fd379246834eac74b8419ffda202cf8051f7a03::signer_struct {
    use 0x1::option::Option;

    // We need to use `Option` here instead of our custom type
    // because allowed structs on entry functions are restricted.
    public entry fun main(_input: Option<signer>) {}
}
