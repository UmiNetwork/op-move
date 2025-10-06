use {
    umi_server::{defaults, ServerRuntimes},
    umi_server_args::{CliLayer, ConfigBuilder, EnvLayer, FileLayer},
};

fn main() {
    let args = ConfigBuilder::new()
        .layer(defaults())
        .layer(FileLayer::toml())
        .layer(EnvLayer::new())
        .layer(CliLayer::new())
        .try_build()
        .expect("Must build config to run app");

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

    http_rt.block_on(umi_server::run_with_runtimes(
        args,
        ServerRuntimes {
            http: &http_rt,
            auth: &auth_rt,
        },
    ));
}
