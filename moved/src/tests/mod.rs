mod integration;
pub mod signer;

use crate::{validate_jwt, Claims};
use alloy_primitives::{address, Address};
use aptos_types::transaction::{EntryFunction, TransactionPayload};
use jsonwebtoken::{EncodingKey, Header};
use move_core_types::account_address::AccountAddress;
use move_core_types::ident_str;
use move_core_types::language_storage::{ModuleId, StructTag, TypeTag};
use std::time::SystemTime;

pub(crate) const EVM_ADDRESS: Address = address!("8fd379246834eac74b8419ffda202cf8051f7a03");

/// The address corresponding to this private key is 0x8fd379246834eac74B8419FfdA202CF8051F7A03
pub(crate) const PRIVATE_KEY: [u8; 32] = [0xaa; 32];

#[tokio::test]
async fn test_authorized_request() -> anyhow::Result<()> {
    std::env::set_var("JWT_SECRET", "00");
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
    let token = jsonwebtoken::encode(
        &Header::default(),
        &Claims { iat: now.as_secs() },
        &EncodingKey::from_secret(&hex::decode("00")?),
    )?;
    let filter = validate_jwt();
    let res = warp::test::request()
        .header("authorization", ["Bearer", &token].join(" "))
        .filter(&filter)
        .await;
    assert_eq!(res.unwrap(), token);
    Ok(())
}

#[tokio::test]
async fn test_unauthorized_requests() -> anyhow::Result<()> {
    std::env::set_var("JWT_SECRET", "00");
    let filter = validate_jwt();
    let res = warp::test::request().filter(&filter).await;
    assert!(res.is_err()); // Missing JWT token in the header

    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
    let token = jsonwebtoken::encode(
        &Header::default(),
        &Claims {
            iat: now.as_secs() - 100, // 100 seconds ago
        },
        &EncodingKey::from_secret(&hex::decode("00")?),
    )?;
    let res = warp::test::request()
        .header("authorization", ["Bearer", &token].join(" "))
        .filter(&filter)
        .await;
    assert!(res.is_err()); // Expired JWT token error
    Ok(())
}

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
