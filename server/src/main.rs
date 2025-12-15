use {
    metrics_exporter_prometheus::PrometheusBuilder,
    umi_server::{defaults, ServerRuntimes},
    umi_server_args::{CliLayer, ConfigBuilder, EnvLayer, FileLayer},
};

fn main() {
    let command = ConfigBuilder::new()
        .layer(defaults())
        .layer(FileLayer::toml())
        .layer(EnvLayer::new())
        .layer(CliLayer::new())
        .try_build()
        .expect("Must build config to run app");

    let args = match command {
        umi_server_args::Command::Run(args) => args,
        umi_server_args::Command::PrintGenesisRoot(genesis) => {
            println!("Starting state root computation (this may take a while) ...");
            let given_root = genesis.initial_state_root;
            let root = umi_server::compute_genesis_state_root(genesis);
            println!("{root}");
            if given_root != root {
                println!("WARN: given root not equal to computed root.");
            }
            return;
        }
    };

    PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], 9000))
        .install()
        .expect("Must have Prometheus sink installed");

    umi_server::set_global_tracing_subscriber();

    let (auth_count, http_count) = umi_server::set_workers_count();

    let http_rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(http_count)
        .thread_name("rt-http")
        .enable_all()
        .build()
        .expect("Must build http runtime to run app");

    let auth_rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(auth_count)
        .thread_name("rt-auth")
        .enable_all()
        .build()
        .expect("Must build auth runtime to run app");

    http_rt.spawn(
        tokio_metrics::RuntimeMetricsReporterBuilder::default()
            // Default 30s is too coarse
            .with_interval(std::time::Duration::from_secs(5))
            .describe_and_run(),
    );

    http_rt.block_on(umi_server::run_with_runtimes(
        args,
        ServerRuntimes {
            http: &http_rt,
            auth: &auth_rt,
        },
    ));
}
