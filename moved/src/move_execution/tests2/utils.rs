use super::*;
use crate::types::transactions::{DepositedTx, ExtendedTxEnvelope};
use alloy::primitives::{keccak256, FixedBytes};

pub enum TestBaseToken {
    Empty,
    Moved(MovedBaseTokenAccounts),
}

pub struct TestTransaction {
    tx: NormalizedExtendedTxEnvelope,
    tx_hash: B256,
    l1_cost: u64,
    base_token: TestBaseToken,
}

impl TestTransaction {
    pub fn new(tx: NormalizedExtendedTxEnvelope, tx_hash: B256) -> Self {
        Self {
            tx,
            tx_hash,
            l1_cost: 0,
            base_token: TestBaseToken::Empty,
        }
    }

    pub fn with_cost(&mut self, l1_cost: u64, base_token: MovedBaseTokenAccounts) {
        self.l1_cost = l1_cost;
        self.base_token = TestBaseToken::Moved(base_token);
    }
}

pub struct TestContext {
    pub state: InMemoryState,
    pub genesis_config: GenesisConfig,
    pub signer: Signer,
    pub move_address: AccountAddress,
}

impl TestContext {
    pub fn new() -> Self {
        let genesis_config = GenesisConfig::default();
        let mut state = InMemoryState::new();
        init_state(&genesis_config, &mut state);

        Self {
            state,
            genesis_config,
            signer: Signer::new(&PRIVATE_KEY),
            move_address: EVM_ADDRESS.to_move_address(),
        }
    }

    pub fn deploy_contract(&mut self, module_name: &str) -> ModuleId {
        let module_bytes = self.compile_module(module_name, self.move_address);
        let (tx_hash, tx) = create_transaction(&mut self.signer, TxKind::Create, module_bytes);
        let transaction = TestTransaction::new(tx, tx_hash);
        let outcome = self.execute_tx(transaction).unwrap();
        self.state.apply(outcome.changes).unwrap();

        let module_id = ModuleId::new(self.move_address, Identifier::new(module_name).unwrap());
        assert!(
            self.state
                .resolver()
                .get_module(&module_id)
                .unwrap()
                .is_some(),
            "Code should be deployed"
        );
        module_id
    }

    pub fn execute(&mut self, module_id: &ModuleId, function: &str, args: Vec<&MoveValue>) {
        let args = args.iter().map(|a| bcs::to_bytes(a).unwrap()).collect();
        let (tx_hash, tx) = create_test_tx(&mut self.signer, module_id, function, args);
        let transaction = TestTransaction::new(tx, tx_hash);
        let outcome = self.execute_tx(transaction).unwrap();
        self.state.apply(outcome.changes).unwrap();
    }

    pub fn execute_err(
        &mut self,
        module_id: &ModuleId,
        function: &str,
        args: Vec<&MoveValue>,
    ) -> crate::Error {
        let args = args.iter().map(|a| bcs::to_bytes(a).unwrap()).collect();
        let (tx_hash, tx) = create_test_tx(&mut self.signer, module_id, function, args);
        let transaction = TestTransaction::new(tx, tx_hash);
        self.execute_tx(transaction).unwrap_err()
    }

    pub fn deposit_eth(&mut self, to: Address, value: U256) {
        let tx = ExtendedTxEnvelope::DepositedTx(DepositedTx {
            to,
            value,
            source_hash: FixedBytes::default(),
            from: to,
            mint: U256::ZERO,
            gas: U64::from(u64::MAX),
            is_system_tx: false,
            data: Vec::new().into(),
        });
        let tx_hash = {
            let capacity = tx.length();
            let mut bytes = Vec::with_capacity(capacity);
            tx.encode(&mut bytes);
            B256::new(keccak256(bytes).0)
        };
        let transaction = TestTransaction::new(tx.try_into().unwrap(), tx_hash);
        self.execute_tx(transaction).unwrap();
    }

    pub fn get_resource<T: DeserializeOwned>(&self, module_name: &str, struct_name: &str) -> T {
        // Resource was created for a module struct
        let struct_tag = StructTag {
            address: self.move_address,
            module: Identifier::new(module_name).unwrap(),
            name: Identifier::new(struct_name).unwrap(),
            type_args: Vec::new(),
        };
        let data = self
            .state
            .resolver()
            .get_resource(&self.move_address, &struct_tag)
            .unwrap()
            .unwrap();
        bcs::from_bytes(data.as_ref()).unwrap()
    }

    pub fn get_balance(&self, address: Address) -> U256 {
        quick_get_eth_balance(&address.to_move_address(), self.state.resolver())
    }

    fn execute_tx(&mut self, tx: TestTransaction) -> crate::Result<TransactionExecutionOutcome> {
        match tx.base_token {
            TestBaseToken::Empty => execute_transaction(
                &tx.tx,
                &tx.tx_hash,
                self.state.resolver(),
                &self.genesis_config,
                tx.l1_cost,
                &(),
                HeaderForExecution::default(),
            ),
            TestBaseToken::Moved(moved_base_token) => execute_transaction(
                &tx.tx,
                &tx.tx_hash,
                self.state.resolver(),
                &self.genesis_config,
                tx.l1_cost,
                &moved_base_token,
                HeaderForExecution::default(),
            ),
        }
    }

    fn compile_module(&self, module_name: &str, address: AccountAddress) -> Vec<u8> {
        let module_bytes = ModuleCompileJob::new(module_name, &address)
            .compile()
            .unwrap();
        module_bytes_to_tx_data(module_bytes)
    }
}

pub fn encode_move_args<T: serde::Serialize>(args: &[T]) -> Vec<Vec<u8>> {
    args.iter().map(|arg| bcs::to_bytes(arg).unwrap()).collect()
}

// Serialize module bytes to be used as a transaction payload
fn module_bytes_to_tx_data(module_bytes: Vec<u8>) -> Vec<u8> {
    bcs::to_bytes(&ScriptOrModule::Module(Module::new(module_bytes))).unwrap()
}

pub fn create_test_tx(
    signer: &mut Signer,
    module_id: &ModuleId,
    function: &str,
    args: Vec<Vec<u8>>,
) -> (B256, NormalizedExtendedTxEnvelope) {
    let entry_fn = EntryFunction::new(
        module_id.clone(),
        Identifier::new(function).unwrap(),
        Vec::new(),
        args,
    );

    create_transaction(
        signer,
        TxKind::Call(EVM_ADDRESS),
        bcs::to_bytes(&entry_fn).unwrap(),
    )
}

pub fn create_transaction(
    signer: &mut Signer,
    to: TxKind,
    input: Vec<u8>,
) -> (B256, NormalizedExtendedTxEnvelope) {
    create_transaction_with_value(signer, to, input, U256::ZERO)
}

pub fn create_transaction_with_value(
    signer: &mut Signer,
    to: TxKind,
    input: Vec<u8>,
    value: U256,
) -> (B256, NormalizedExtendedTxEnvelope) {
    let mut tx = TxEip1559 {
        chain_id: CHAIN_ID,
        nonce: signer.nonce,
        gas_limit: u64::MAX,
        max_fee_per_gas: 0,
        max_priority_fee_per_gas: 0,
        to,
        value,
        access_list: Default::default(),
        input: input.into(),
    };
    signer.nonce += 1;
    let signature = signer.inner.sign_transaction_sync(&mut tx).unwrap();
    let signed_tx = TxEnvelope::Eip1559(tx.into_signed(signature));
    let tx_hash = *signed_tx.tx_hash();
    let normalized_tx = NormalizedExtendedTxEnvelope::Canonical(signed_tx.try_into().unwrap());

    (tx_hash, normalized_tx)
}

trait CompileJob {
    fn targets(&self) -> Vec<String>;
    fn deps(&self) -> Vec<String>;
    fn named_addresses(&self) -> BTreeMap<String, NumericalAddress>;

    fn known_attributes(&self) -> BTreeSet<String> {
        BTreeSet::new()
    }

    fn compile(&self) -> anyhow::Result<Vec<u8>> {
        let targets = self.targets();
        let error_context = format!("Failed to compile {targets:?}");
        let compiler = Compiler::from_files(
            targets,
            self.deps(),
            self.named_addresses(),
            Flags::empty(),
            &self.known_attributes(),
        );
        let (_, result) = compiler.build().context(error_context)?;
        let compiled_unit = result.unwrap().0.pop().unwrap().into_compiled_unit();
        let bytes = compiled_unit.serialize(None);
        Ok(bytes)
    }
}

struct ModuleCompileJob {
    targets_inner: Vec<String>,
    named_addresses_inner: BTreeMap<String, NumericalAddress>,
}

impl ModuleCompileJob {
    pub fn new(package_name: &str, address: &AccountAddress) -> Self {
        let named_address_mapping: std::collections::BTreeMap<_, _> = std::iter::once((
            package_name.to_string(),
            NumericalAddress::new(address.into(), NumberFormat::Hex),
        ))
        .chain(custom_framework_named_addresses())
        .chain(aptos_framework::named_addresses().clone())
        .collect();

        let base_dir = format!("src/tests/res/{package_name}").replace('_', "-");
        let targets = vec![format!("{base_dir}/sources/{package_name}.move")];

        Self {
            targets_inner: targets,
            named_addresses_inner: named_address_mapping,
        }
    }
}

impl CompileJob for ModuleCompileJob {
    fn targets(&self) -> Vec<String> {
        self.targets_inner.clone()
    }

    fn deps(&self) -> Vec<String> {
        let mut framework = aptos_framework::testnet_release_bundle()
            .files()
            .expect("Must be able to find Aptos Framework files");
        let genesis_base = "../genesis-builder/framework/aptos-framework/sources";
        framework.append(&mut vec![
            format!("{genesis_base}/fungible_asset_u256.move"),
            format!("{genesis_base}/primary_fungible_store_u256.move"),
        ]);
        add_custom_framework_paths(&mut framework);
        framework
    }

    fn named_addresses(&self) -> BTreeMap<String, NumericalAddress> {
        self.named_addresses_inner.clone()
    }
}

struct ScriptCompileJob {
    targets_inner: Vec<String>,
    deps_inner: Vec<String>,
}

impl ScriptCompileJob {
    pub fn new(script_name: &str, local_deps: &[&str]) -> Self {
        let base_dir = format!("src/tests/res/{script_name}").replace('_', "-");
        let targets = vec![format!("{base_dir}/sources/{script_name}.move")];

        let local_deps = local_deps.iter().map(|package_name| {
            let base_dir = format!("src/tests/res/{package_name}").replace('_', "-");
            format!("{base_dir}/sources/{package_name}.move")
        });
        let deps = {
            let mut framework = aptos_framework::testnet_release_bundle()
                .files()
                .expect("Must be able to find Aptos Framework files");
            let genesis_base = "../genesis-builder/framework/aptos-framework/sources";
            framework.append(&mut vec![
                format!("{genesis_base}/fungible_asset_u256.move"),
                format!("{genesis_base}/primary_fungible_store_u256.move"),
            ]);

            add_custom_framework_paths(&mut framework);
            local_deps.for_each(|d| framework.push(d));

            framework
        };

        Self {
            targets_inner: targets,
            deps_inner: deps,
        }
    }
}

impl CompileJob for ScriptCompileJob {
    fn targets(&self) -> Vec<String> {
        self.targets_inner.clone()
    }

    fn deps(&self) -> Vec<String> {
        self.deps_inner.clone()
    }

    fn named_addresses(&self) -> BTreeMap<String, NumericalAddress> {
        let mut result = aptos_framework::named_addresses().clone();
        for (name, address) in custom_framework_named_addresses() {
            result.insert(name, address);
        }
        result
    }
}

fn custom_framework_named_addresses() -> impl Iterator<Item = (String, NumericalAddress)> {
    [
        (
            "EthToken".into(),
            NumericalAddress::parse_str("0x1").unwrap(),
        ),
        ("Evm".into(), NumericalAddress::parse_str("0x1").unwrap()),
        (
            "evm_admin".into(),
            NumericalAddress::parse_str("0x1").unwrap(),
        ),
        (
            "L2CrossDomainMessenger".into(),
            NumericalAddress::parse_str("0x4200000000000000000000000000000000000007").unwrap(),
        ),
    ]
    .into_iter()
}

fn add_custom_framework_paths(files: &mut Vec<String>) {
    add_framework_path("eth-token", "EthToken", files);
    add_framework_path("evm", "Evm", files);
}

fn add_framework_path(folder_name: &str, source_name: &str, files: &mut Vec<String>) {
    let base_path = Path::new(std::env!("CARGO_MANIFEST_DIR"));
    let eth_token_path = base_path
        .join(format!(
            "../genesis-builder/framework/{folder_name}/sources/{source_name}.move"
        ))
        .canonicalize()
        .unwrap();
    files.push(eth_token_path.to_string_lossy().into());
}
