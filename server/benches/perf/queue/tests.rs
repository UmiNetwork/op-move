use {
    crate::queue::input,
    criterion::{
        criterion_group, measurement::WallTime, BatchSize, BenchmarkGroup, BenchmarkId, Criterion,
        Throughput,
    },
    moved_app::{Application, DependenciesThreadSafe},
    moved_genesis::config::GenesisConfig,
    moved_server::initialize_app,
    std::process::Termination,
    tokio::runtime::Runtime,
};

fn build_1000_blocks(
    current: &Runtime,
    bencher: &mut BenchmarkGroup<WallTime>,
    app: Box<Application<impl DependenciesThreadSafe>>,
    buffer_size: u32,
) {
    bencher.throughput(Throughput::Elements(*input::BLOCKS_1000_LEN));
    bencher.sample_size(100);
    bencher.bench_with_input(BenchmarkId::from_parameter(buffer_size), &buffer_size, {
        |b, _size| {
            b.iter_batched(
                || {
                    let (queue, actor) = moved_app::create(app, buffer_size);

                    let handle = current.spawn(async move { actor.spawn().await.unwrap() });

                    (queue, handle, input::blocks_1000())
                },
                |(queue, handle, input)| {
                    current.block_on(async move {
                        for msg in input {
                            queue.send(msg).await;
                        }
                        drop(queue);
                        handle.await.unwrap()
                    })
                },
                BatchSize::PerIteration,
            );
        }
    });
}

fn bench_build_1000_blocks_with_queue_size(bencher: &mut Criterion) -> impl Termination {
    let current = tokio::runtime::Builder::new_multi_thread().build().unwrap();
    let mut group = bencher.benchmark_group("Build 1000 blocks with queue size");

    for buffer_size in [1000000, 10000, 6000, 5000, 1000, 500, 200, 100, 1]
        .into_iter()
        .rev()
    {
        let (mut app, _app_reader) = initialize_app(GenesisConfig::default());

        app.genesis_update(input::GENESIS);

        let app = Box::new(app);

        build_1000_blocks(&current, &mut group, app, buffer_size);
    }
}

criterion_group!(benches, bench_build_1000_blocks_with_queue_size);
