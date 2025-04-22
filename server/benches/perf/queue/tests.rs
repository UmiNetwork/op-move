use {
    crate::queue::input,
    moved_genesis::config::GenesisConfig,
    moved_server::initialize_app,
    paste::paste,
    std::{process::Termination, sync::Arc},
    test::Bencher,
    tokio::sync::RwLock,
};

fn build_1000_blocks(bencher: &mut Bencher, buffer_size: u32) {
    let app = initialize_app(GenesisConfig::default());
    let app = Arc::new(RwLock::new(app));

    let current = tokio::runtime::Builder::new_multi_thread().build().unwrap();

    let (current, handle) = {
        let (queue, actor) = moved_app::create(app, buffer_size);

        let handle = current.block_on(async move { actor.spawn() });

        bencher.iter(|| {
            let queue = queue.clone();

            current.block_on(async move {
                for msg in input::blocks_1000() {
                    queue.send(msg).await;
                }

                queue.wait_for_pending_commands().await
            })
        });

        (current, handle)
    };

    current.block_on(async move {
        handle.await.unwrap();
    });
}

macro_rules! generate_bench {
    ($($buffer_size:expr$(,)?)*) => {$(paste!{
        #[bench]
        fn [<bench_build_1000_blocks_with_queue_size_ $buffer_size>](bencher: &mut Bencher) -> impl Termination {
            build_1000_blocks(bencher, $buffer_size);
        }
    })*};
}

generate_bench!(10000, 6000, 5000, 1000, 500, 200, 100);
