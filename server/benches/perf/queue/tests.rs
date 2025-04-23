use {
    crate::queue::input,
    criterion::{
        criterion_group, measurement::WallTime, BatchSize, BenchmarkGroup, BenchmarkId, Criterion,
        Throughput,
    },
    moved_genesis::config::GenesisConfig,
    moved_server::initialize_app,
    std::{process::Termination, sync::Arc},
    tokio::sync::RwLock,
};

fn build_1000_blocks(bencher: &mut BenchmarkGroup<WallTime>, buffer_size: u32) {
    bencher.throughput(Throughput::Elements(*input::BLOCKS_1000_LEN));

    let app = initialize_app(GenesisConfig::default());
    let app = Arc::new(RwLock::new(app));

    let current = tokio::runtime::Builder::new_multi_thread().build().unwrap();

    let (current, handle) = {
        let (queue, actor) = moved_app::create(app, buffer_size);

        let handle = current.spawn(async move { actor.spawn().await });

        bencher.bench_with_input(
            BenchmarkId::from_parameter(buffer_size),
            &buffer_size,
            |b, _size| {
                b.iter_batched(
                    input::blocks_1000,
                    |input| {
                        let queue = queue.clone();

                        current.block_on(async move {
                            for msg in input {
                                queue.send(msg).await;
                            }

                            queue.wait_for_pending_commands().await
                        })
                    },
                    BatchSize::PerIteration,
                );
            },
        );

        (current, handle)
    };

    current.block_on(handle).unwrap().unwrap();
}

fn bench_build_1000_blocks_with_queue_size(bencher: &mut Criterion) -> impl Termination {
    let mut group = bencher.benchmark_group("Build 1000 blocks with queue size");
    for buffer_size in [
        1000000,
        10000,
        6000,
        5000,
        *input::BLOCKS_1000_LEN as u32,
        1000,
        500,
        200,
        100,
        1,
    ] {
        build_1000_blocks(&mut group, buffer_size);
    }
}

criterion_group!(benches, bench_build_1000_blocks_with_queue_size);
