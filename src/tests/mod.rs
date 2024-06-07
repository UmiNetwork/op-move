use aptos_types::account_address::AccountAddress;
use aptos_types::transaction::{EntryFunction, TransactionPayload};
use move_core_types::ident_str;
use move_core_types::language_storage::{ModuleId, StructTag, TypeTag};

#[tokio::test]
async fn test_entry_function_payload() -> anyhow::Result<()> {
    let to = AccountAddress::TEN;
    let amount: u64 = 100;
    let eth_coin_type: TypeTag = TypeTag::Struct(Box::new(StructTag {
        address: AccountAddress::ONE,
        module: ident_str!("Ethereum").to_owned(),
        name: ident_str!("ETH").to_owned(),
        type_args: vec![],
    }));
    let payload = TransactionPayload::EntryFunction(EntryFunction::new(
        ModuleId::new(
            // TODO: Use 20 byte address length
            AccountAddress::new([
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 1,
            ]),
            ident_str!("coin").to_owned(),
        ),
        ident_str!("transfer").to_owned(),
        vec![eth_coin_type],
        vec![bcs::to_bytes(&to).unwrap(), bcs::to_bytes(&amount).unwrap()],
    ));
    let serialized_payload = bcs::to_bytes(&payload)?;
    assert_eq!(serialized_payload[0], 2); // Starting with 2 indicates an entry function
    Ok(())
}
