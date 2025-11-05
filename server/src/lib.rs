use {
    alloy::consensus::{proofs::state_root_unhashed, EMPTY_OMMER_ROOT_HASH, EMPTY_ROOT_HASH},
    jsonwebtoken::{DecodingKey, Validation},
    serde::Serialize,
    std::{
        future::Future,
        net::{Ipv4Addr, SocketAddr, SocketAddrV4},
        num::NonZeroUsize,
        path::Path,
        str::FromStr,
        time::SystemTime,
    },
    tracing::level_filters::LevelFilter,
    tracing_subscriber::{fmt::format::FmtSpan, EnvFilter},
    umi_api::{
        jsonrpc::JsonRpcResponse,
        method_name::MethodName,
        request::{RequestModifiers, SerializationKind},
    },
    umi_app::{Application, ApplicationReader, CommandQueue, Dependencies},
    umi_blockchain::{
        block::{Block, BlockHash, BlockQueries, ExtendedBlock, Header},
        payload::{NewPayloadId, StatePayloadId},
    },
    umi_genesis::config::GenesisConfig,
    umi_server_args::{
        Config, DatabaseBackend, DefaultLayer, OptionalAuthSocket, OptionalConfig,
        OptionalDatabase, OptionalGenesis, OptionalHttpSocket,
    },
    umi_shared::{
        hex,
        primitives::{ToSaturatedU64, B2048, B256, B64, U256},
    },
    warp::{
        http::{header::CONTENT_TYPE, HeaderMap, HeaderValue},
        hyper::Response,
        reply::Reply,
        Filter, Rejection,
    },
};

mod allow;
mod dependency;
#[cfg(test)]
mod tests;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Claims {
    iat: u64,
}

#[derive(Debug, Serialize)]
pub struct ServerLog<'a> {
    pub request: &'a serde_json::Value,
    pub op_move_response: &'a JsonRpcResponse,
}

#[derive(Debug)]
pub struct ServerRuntimes<'a> {
    pub http: &'a tokio::runtime::Runtime,
    pub auth: &'a tokio::runtime::Runtime,
}

pub fn defaults() -> DefaultLayer {
    let default_genesis_config = GenesisConfig::default();
    let umi_root_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Cargo manifest has a parent");
    DefaultLayer::new(OptionalConfig {
        auth: Some(OptionalAuthSocket {
            addr: Some(SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::new(0, 0, 0, 0),
                8551,
            ))),
            jwt_secret: None,
        }),
        http: Some(OptionalHttpSocket {
            addr: Some(SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::new(0, 0, 0, 0),
                8545,
            ))),
        }),
        db: Some(OptionalDatabase {
            backend: Some(DatabaseBackend::InMemory),
            dir: Some(Path::new("db").into()),
            purge: Some(false),
        }),
        genesis: Some(OptionalGenesis {
            chain_id: Some(42069),
            initial_state_root: Some(default_genesis_config.initial_state_root),
            treasury: Some(default_genesis_config.treasury), // TODO: fill in the real address,
            l2_contract_genesis: Some(
                umi_root_path.join("server/src/tests/optimism/packages/contracts-bedrock/deployments/genesis.json")
                    .into(),
            ),
            token_list: Some(
                umi_root_path.join(
                    "execution/src/tests/res/bridged_tokens_test.json",
                )
                .into(),
            ),
        }),
        max_buffered_commands: Some(1_000),
    })
}

const JWT_VALID_DURATION_IN_SECS: u64 = 60;

pub fn set_workers_count() -> (usize, usize) {
    let core_count = std::thread::available_parallelism()
        .map(NonZeroUsize::get)
        .unwrap_or(1);
    // Leaving some leeway to avoid saturation
    let reserve = core_count.saturating_div(8).max(1);
    let usable = core_count.saturating_sub(reserve);

    if usable < 4 {
        return (1, 1);
    }

    let auth_ratio: f32 = 0.30;
    let auth_workers = (((usable as f32) * auth_ratio).round() as usize).clamp(2, usable - 2);
    let http_workers = usable.saturating_sub(auth_workers).max(2);
    (auth_workers, http_workers)
}

pub fn set_global_tracing_subscriber() {
    // TODO: config options for logging (debug level, output to file, etc)

    // Default to debug level logging, except for hyper and alloy because they are too verbose.
    let filter = EnvFilter::default()
        .add_directive(LevelFilter::DEBUG.into())
        .add_directive("hyper=warn".parse().expect("Is valid directive"))
        .add_directive("alloy=info".parse().expect("Is valid directive"));

    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_thread_names(true)
        .with_env_filter(filter)
        .with_ansi(false)
        .with_span_events(FmtSpan::FULL)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");
}

pub async fn run_with_runtimes(args: Config, rts: ServerRuntimes<'_>) {
    let genesis_config = GenesisConfig::try_new(
        args.genesis.chain_id,
        args.genesis.initial_state_root,
        args.genesis.treasury,
        args.genesis.l2_contract_genesis.as_ref(),
        args.genesis.token_list.as_ref(),
    )
    .expect("Must construct genesis config to run the app");

    let deps = dependency::dependencies(args.db);
    let reader = {
        let genesis_config = genesis_config.clone();
        let deps = deps.reader();
        move || ApplicationReader::new(deps, &genesis_config)
    };
    let app = move || Application::new(deps, &genesis_config).with_genesis(&genesis_config);
    let jwt = DecodingKey::from_secret(
        hex::decode(args.auth.jwt_secret)
            .expect("JWT secret must be valid")
            .as_slice(),
    );

    umi_app::run(
        (reader, app),
        args.max_buffered_commands,
        |queue, reader| async move {
            let queue_http = queue.clone();
            let reader_http = reader.clone();
            let addr_http = args.http.addr;

            let http = rts.http.spawn(serve(
                addr_http,
                &queue_http,
                &reader_http,
                &allow::http,
                None,
            ));

            let queue_auth = queue.clone();
            let reader_auth = reader.clone();
            let addr_auth = args.auth.addr;

            let auth = rts.auth.spawn(serve(
                addr_auth,
                &queue_auth,
                &reader_auth,
                &allow::auth,
                Some(jwt),
            ));

            queue.shutdown_listener().await;
            let _ = (http.await, auth.await);
        },
    )
    .await
}

pub fn server_filter(
    queue: &CommandQueue,
    reader: &ApplicationReader<'static, dependency::ReaderDependency>,
    is_allowed: &'static (impl Fn(&MethodName) -> bool + Send + Sync),
    jwt: Option<DecodingKey>,
) -> impl Filter<Extract = impl Reply> + Clone {
    let services = (queue.clone(), reader.clone());
    let content_type =
        HeaderMap::from_iter([(CONTENT_TYPE, HeaderValue::from_static("application/json"))]);

    let get_method_auto_response = warp::get().map(warp::reply);
    let app_state = warp::any().map(move || services.clone());
    let jwt_validation = validate_jwt(jwt);
    let request_body = warp::body::json::<serde_json::Value>();
    let evm_path = warp::path!("evm").map(|| SerializationKind::Evm);
    let root_path = warp::any().map(|| SerializationKind::Bcs);
    let serialization_kind = evm_path.or(root_path).unify();

    get_method_auto_response
        .or(serialization_kind
            .and(warp::header::optional::<Msec>("X-Req-Start-Ms"))
            .and(app_state)
            .and(jwt_validation)
            .and(request_body)
            .and_then(
                move |serialization_tag, request_start, (queue, reader), _, request| {
                    handle_request(
                        queue,
                        serialization_tag,
                        request_start,
                        request,
                        is_allowed,
                        &StatePayloadId,
                        reader,
                    )
                },
            ))
        .with(warp::reply::with::headers(content_type))
        .with(warp::cors().allow_any_origin())
}

#[derive(Debug)]
struct Msec(u128);

impl FromStr for Msec {
    type Err = <f64 as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self((s.parse::<f64>()? * 1000.0) as u128))
    }
}

impl From<Msec> for u128 {
    fn from(value: Msec) -> Self {
        value.0
    }
}

fn serve(
    addr: SocketAddr,
    queue: &CommandQueue,
    reader: &ApplicationReader<'static, dependency::ReaderDependency>,
    is_allowed: &'static (impl Fn(&MethodName) -> bool + Send + Sync),
    jwt: Option<DecodingKey>,
) -> impl Future<Output = ()> + Send + 'static {
    let route = server_filter(queue, reader, is_allowed, jwt);
    warp::serve(route)
        .bind_with_graceful_shutdown(addr, queue.shutdown_listener())
        .1
}

/// An extension trait adds features for applying genesis state to an empty blockchain state.
pub trait GenesisStateExt: Sized {
    /// Determines if the blockchain state is empty.
    ///
    /// Empty blockchain state is defined as a tree with zero nodes, not even genesis.
    fn is_state_empty(&self) -> bool;

    /// Applies genesis blockchain state changes onto `self`.
    fn initialize_genesis_state(&mut self, genesis_config: &GenesisConfig);

    /// Applies genesis blockchain state changes onto `self`, but only if the state is empty.
    fn initialize_genesis_state_if_empty(&mut self, genesis_config: &GenesisConfig) {
        if self.is_state_empty() {
            self.initialize_genesis_state(genesis_config);
        }
    }

    /// Returns `self` that has genesis state changes applied. The implementation should not apply
    /// the genesis changes if the state is not empty.
    fn with_genesis(mut self, genesis_config: &GenesisConfig) -> Self {
        self.initialize_genesis_state_if_empty(genesis_config);
        self
    }
}

impl<'db, D: Dependencies<'db>> GenesisStateExt for Application<'db, D> {
    fn is_state_empty(&self) -> bool {
        let fc = self
            .block_queries
            .get_forkchoice_state(&self.storage_reader)
            .expect("Must access block queries to run app");
        fc.head_block_hash == B256::ZERO
    }

    fn initialize_genesis_state(&mut self, genesis_config: &GenesisConfig) {
        let (genesis_changes, evm_storage_changes) = {
            #[cfg(test)]
            {
                umi_genesis_image::load()
            }
            #[cfg(not(test))]
            {
                umi_genesis::build(
                    &umi_genesis::UmiVm::new(genesis_config),
                    genesis_config,
                    &self.evm_storage,
                )
            }
        };
        umi_genesis::apply(
            genesis_changes,
            evm_storage_changes,
            genesis_config,
            &mut self.state,
            &mut self.evm_storage,
        );

        #[cfg(feature = "op-upgrade")]
        let withdrawals_root = {
            use {
                umi_blockchain::state::evm_storage_root_from_trie_and_resolver, umi_state::State,
            };

            let storage_root = evm_storage_root_from_trie_and_resolver(
                umi_app::L2_TO_L1_MESSAGE_PASSER_ADDRESS,
                self.state.resolver(),
                &self.evm_storage,
            )
            .expect("Should be able to retrieve L2ToL1MessagePasser storage root");
            Some(storage_root)
        };
        #[cfg(not(feature = "op-upgrade"))]
        // Has to be `keccak256(rlp(empty_string_code))`
        let withdrawals_root = Some(alloy::consensus::constants::EMPTY_WITHDRAWALS);

        let genesis_block =
            create_genesis_block(&self.block_hash, genesis_config, withdrawals_root);
        self.genesis_update(genesis_block)
            .expect("Must add genesis block to state");
    }
}

pub fn initialize_app(
    args: umi_server_args::Database,
    genesis_config: &GenesisConfig,
) -> (
    Application<'static, dependency::Dependency>,
    ApplicationReader<'static, dependency::ReaderDependency>,
) {
    let (mut app, app_reader) = dependency::create(args, genesis_config);
    app.initialize_genesis_state_if_empty(genesis_config);
    (app, app_reader)
}

fn create_genesis_block(
    block_hash: &impl BlockHash,
    genesis_config: &GenesisConfig,
    withdrawals_root: Option<B256>,
) -> ExtendedBlock {
    // As defined in <https://specs.optimism.io/protocol/isthmus/exec-engine.html#header-validity-rules>,
    // i.e. a hash of an empty string
    #[cfg(feature = "op-upgrade")]
    let requests_hash = Some(umi_app::EMPTY_REQUESTS_HASH);
    #[cfg(not(feature = "op-upgrade"))]
    let requests_hash = None;
    let genesis_header = Header {
        base_fee_per_gas: genesis_config
            .l2_contract_genesis
            .base_fee_per_gas
            .map(ToSaturatedU64::to_saturated_u64),
        blob_gas_used: genesis_config.l2_contract_genesis.blob_gas_used,
        difficulty: genesis_config.l2_contract_genesis.difficulty,
        excess_blob_gas: genesis_config.l2_contract_genesis.excess_blob_gas,
        extra_data: genesis_config.l2_contract_genesis.extra_data.clone(),
        gas_limit: genesis_config.l2_contract_genesis.gas_limit,
        gas_used: 0,
        logs_bloom: B2048::ZERO.into(),
        mix_hash: genesis_config.l2_contract_genesis.mix_hash,
        nonce: B64::from(genesis_config.l2_contract_genesis.nonce),
        number: genesis_config.l2_contract_genesis.number.unwrap_or(0),
        parent_beacon_block_root: Some(B256::ZERO),
        parent_hash: B256::ZERO,
        receipts_root: EMPTY_ROOT_HASH,
        state_root: state_root_unhashed(genesis_config.l2_contract_genesis.alloc.clone()),
        timestamp: genesis_config.l2_contract_genesis.timestamp,
        transactions_root: EMPTY_ROOT_HASH,
        withdrawals_root,
        beneficiary: genesis_config.l2_contract_genesis.coinbase,
        ommers_hash: EMPTY_OMMER_ROOT_HASH,
        requests_hash,
    };
    let hash = block_hash.block_hash(&genesis_header);
    let genesis_block = Block::new(genesis_header, Vec::new());

    genesis_block
        .into_extended_with_hash(hash)
        .with_value(U256::ZERO)
}

pub fn validate_jwt(
    secret: Option<DecodingKey>,
) -> impl Filter<Extract = (Option<String>,), Error = Rejection> + Clone {
    let is_unprotected = secret.is_none();

    warp::header::<String>("authorization")
        .map(Some)
        .or_else(move |err| async move {
            if is_unprotected {
                Ok((None,))
            } else {
                Err(err)
            }
        })
        .and_then(move |token: Option<String>| {
            let secret = secret.clone();

            async move {
                let Some((secret, token)) = secret.zip(token) else {
                    return Ok(None);
                };
                // Token is embedded as a string in the form of `Bearer the.actual.token`
                let token = token.trim_start_matches("Bearer ").to_string();
                let mut validation = Validation::default();
                // OP node only sends `issued at` claims in the JWT token
                validation.set_required_spec_claims(&["iat"]);
                let decoded = jsonwebtoken::decode::<Claims>(&token, &secret, &validation);
                let iat = decoded.map_err(|_| warp::reject::reject())?.claims.iat;
                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .expect("Current system time should be available")
                    .as_secs();
                if now > iat + JWT_VALID_DURATION_IN_SECS {
                    return Err(warp::reject::reject());
                }
                Ok(Some(token))
            }
        })
}

async fn handle_request<'reader>(
    queue: CommandQueue,
    serialization_tag: SerializationKind,
    request_start: Option<Msec>,
    request: serde_json::Value,
    is_allowed: &impl Fn(&MethodName) -> bool,
    payload_id: &impl NewPayloadId,
    app: ApplicationReader<'reader, impl Dependencies<'reader>>,
) -> Result<warp::reply::Response, Rejection> {
    let modifiers = RequestModifiers::new(
        is_allowed,
        payload_id,
        serialization_tag,
        request_start.map(Msec::into),
    );
    let op_move_response =
        umi_api::request::handle(request.clone(), queue.clone(), modifiers, app).await;
    let log = ServerLog {
        request: &request,
        op_move_response: &op_move_response,
    };
    serde_json::to_string(&log)
        .map(|json| tracing::info!("{json}"))
        .ok();

    let body = hyper::Body::from(
        serde_json::to_vec(&op_move_response).expect("Must be able to serialize response"),
    );
    Ok(Response::new(body))
}
