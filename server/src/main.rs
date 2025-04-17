#[tokio::main]
async fn main() {
    // TODO: think about channel size bound
    let max_buffered_commands = 1_000;
    let max_concurrent_queries = 4;

    moved_server::run(max_buffered_commands, max_concurrent_queries).await;
}
