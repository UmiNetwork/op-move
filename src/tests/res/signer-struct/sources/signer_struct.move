module 0x8fd379246834eac74b8419ffda202cf8051f7a03::signer_struct {
    struct ContainsSigner has drop {
        inner: signer,
    }

    public entry fun main(_input: ContainsSigner) {}
}
