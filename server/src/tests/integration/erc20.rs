use {
    super::*,
    move_vm_runtime::session::SerializedReturnValues,
    moved_evm_ext::{extract_evm_result, EVM_NATIVE_ADDRESS},
    moved_shared::primitives::ToEthAddress,
};

alloy::sol!(
    #[sol(rpc)]
    Erc20,
    "src/tests/res/ERC20.json"
);
mod erc20_factory {
    alloy::sol!(
        #[sol(rpc)]
        OptimismMintableERC20Factory,
        "src/tests/res/OptimismMintableERC20Factory.json"
    );
}

mod bridge {
    alloy::sol!(
        #[sol(rpc)]
        L1StandardBridge,
        "src/tests/res/L1StandardBridge.json"
    );
}

const NAME: &str = "Gold";
const SYMBOL: &str = "AU";

/// Create a new ERC-20 token on the L1 chain, returning its address.
/// For convenience, this function also calls `approve` on the new
/// ERC-20 token allowing the `L1StandardBridgeProxy` to spend the newly
/// created tokens.
pub async fn deploy_l1_token(from_wallet: &PrivateKeySigner, rpc_url: &str) -> Result<Address> {
    let from_address = from_wallet.address();
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(EthereumWallet::from(from_wallet.to_owned()))
        .on_http(Url::parse(rpc_url)?);

    let contract = Erc20::deploy(
        provider,
        NAME.into(),
        SYMBOL.into(),
        from_address,
        U256::MAX,
    )
    .await?;

    let bridge_address = Address::from_str(&get_deployed_address("L1StandardBridgeProxy")?)?;
    contract
        .approve(bridge_address, U256::MAX)
        .send()
        .await?
        .watch()
        .await?;

    Ok(*contract.address())
}

/// Use the `OptimismMintableERC20Factory` to create a new ERC-20 token on the L2.
/// This token is used for bridging the L1 token with the given address to the L2.
pub async fn deploy_l2_token(
    from_wallet: &PrivateKeySigner,
    l1_address: Address,
    rpc_url: &str,
) -> Result<Address> {
    let factory_address = alloy::primitives::address!("4200000000000000000000000000000000000012");
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(EthereumWallet::from(from_wallet.to_owned()))
        .on_http(Url::parse(rpc_url)?);

    let contract = erc20_factory::OptimismMintableERC20Factory::new(factory_address, provider);
    let receipt = contract
        .createOptimismMintableERC20(l1_address, NAME.into(), SYMBOL.into())
        .send()
        .await?
        .get_receipt()
        .await?;
    let event = receipt
        .inner
        .logs()
        .iter()
        .find(|log| log.address() == factory_address)
        .expect("OptimismMintableERC20Factory emits log");
    let event = event
        .log_decode::<erc20_factory::OptimismMintableERC20Factory::StandardL2TokenCreated>()
        .expect("Event is type StandardL2TokenCreated");
    let l2_token_address = event.data().localToken;
    Ok(l2_token_address)
}

/// Submits a transaction to the `L1StandardBridgeProxy` to deposit L1 ERC-20 tokens (`l1_address`)
/// into the L2, where the address of the token on the L2 is also specified (`l2_address`).
pub async fn deposit_l1_token(
    from_wallet: &PrivateKeySigner,
    l1_address: Address,
    l2_address: Address,
    amount: U256,
    rpc_url: &str,
) -> Result<()> {
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(EthereumWallet::from(from_wallet.to_owned()))
        .on_http(Url::parse(rpc_url)?);

    let bridge_address = Address::from_str(&get_deployed_address("L1StandardBridgeProxy")?)?;
    let bridge_contract = bridge::L1StandardBridge::new(bridge_address, provider);
    let receipt = bridge_contract
        .depositERC20(l1_address, l2_address, amount, 21_000, Default::default())
        .send()
        .await?
        .get_receipt()
        .await?;
    assert!(receipt.inner.is_success(), "ERC-20 deposit should succeed");
    Ok(())
}

pub async fn l2_erc20_balance_of(token: Address, account: Address, rpc_url: &str) -> Result<U256> {
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .on_http(Url::parse(rpc_url)?);

    let args = vec![
        // The caller here does not matter because it is a view call.
        MoveValue::Address(EVM_NATIVE_ADDRESS)
            .simple_serialize()
            .unwrap(),
        MoveValue::Address(token.to_move_address())
            .simple_serialize()
            .unwrap(),
        MoveValue::Address(account.to_move_address())
            .simple_serialize()
            .unwrap(),
    ];
    let function_call = EntryFunction::new(
        ModuleId::new(EVM_NATIVE_ADDRESS, ident_str!("erc20").into()),
        ident_str!("balance_of").into(),
        Vec::new(),
        args,
    );
    let tx_data = TransactionData::EntryFunction(function_call);
    let data = bcs::to_bytes(&tx_data)?;
    let eth_call_result = CallBuilder::new_raw(provider, data.into())
        .to(EVM_NATIVE_ADDRESS.to_eth_address())
        .call()
        .await?;

    let return_values = SerializedReturnValues {
        mutable_reference_outputs: Vec::new(),
        return_values: bcs::from_bytes(&eth_call_result)?,
    };
    let evm_result = extract_evm_result(return_values);

    Ok(U256::from_be_slice(&evm_result.output))
}
