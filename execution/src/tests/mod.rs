pub use utils::*;

use {
    super::*,
    crate::{
        eth_token::quick_get_eth_balance,
        tests::signer::Signer,
        transaction::{
            DepositedTx, ExtendedTxEnvelope, NormalizedExtendedTxEnvelope, ScriptOrModule,
            TransactionData,
        },
    },
    alloy::{
        consensus::{transaction::TxEip1559, SignableTransaction, TxEnvelope},
        network::TxSignerSync,
        primitives::{address, hex, keccak256, Address, Bytes, FixedBytes, TxKind},
        rlp::Encodable,
    },
    anyhow::Context,
    aptos_types::{
        contract_event::ContractEventV2,
        transaction::{EntryFunction, Module, Script, TransactionArgument},
    },
    move_binary_format::{
        file_format::{
            AbilitySet, FieldDefinition, IdentifierIndex, ModuleHandleIndex, SignatureToken,
            StructDefinition, StructFieldInformation, StructHandle, StructHandleIndex,
            TypeSignature,
        },
        CompiledModule,
    },
    move_compiler::{
        shared::{NumberFormat, NumericalAddress},
        Compiler, Flags,
    },
    move_core_types::{
        account_address::AccountAddress,
        identifier::Identifier,
        language_storage::{ModuleId, StructTag},
        resolver::ModuleResolver,
        value::{MoveStruct, MoveValue},
    },
    move_vm_runtime::module_traversal::{TraversalContext, TraversalStorage},
    move_vm_types::gas::UnmeteredGasMeter,
    moved_shared::primitives::{ToMoveAddress, ToMoveU256, B256, U256, U64},
    moved_state::{InMemoryState, State},
    regex::Regex,
    serde::de::DeserializeOwned,
    std::{
        collections::{BTreeMap, BTreeSet},
        fs::read_to_string,
        path::Path,
    },
};

mod counter;
mod data_type;
mod erc20;
mod evm_native;
mod framework;
mod gas_cost;
mod marketplace;
mod natives;
mod signer;
mod transaction;
mod transfer;
mod utils;

pub const EVM_ADDRESS: Address = address!("8fd379246834eac74b8419ffda202cf8051f7a03");

/// The address corresponding to this private key is 0x8fd379246834eac74B8419FfdA202CF8051F7A03
pub const PRIVATE_KEY: [u8; 32] = [0xaa; 32];

pub const ALT_EVM_ADDRESS: Address = address!("88f9b82462f6c4bf4a0fb15e5c3971559a316e7f");

/// The address corresponding to this private key is 0x88f9b82462f6c4bf4a0fb15e5c3971559a316e7f
pub const ALT_PRIVATE_KEY: [u8; 32] = [0xbb; 32];
