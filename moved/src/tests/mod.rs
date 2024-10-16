pub mod signer;

use alloy_primitives::{address, Address};

pub const EVM_ADDRESS: Address = address!("8fd379246834eac74b8419ffda202cf8051f7a03");

/// The address corresponding to this private key is 0x8fd379246834eac74B8419FfdA202CF8051F7A03
pub const PRIVATE_KEY: [u8; 32] = [0xaa; 32];

pub const ALT_EVM_ADDRESS: Address = address!("88f9b82462f6c4bf4a0fb15e5c3971559a316e7f");

/// The address corresponding to this private key is 0x88f9b82462f6c4bf4a0fb15e5c3971559a316e7f
pub const ALT_PRIVATE_KEY: [u8; 32] = [0xbb; 32];
