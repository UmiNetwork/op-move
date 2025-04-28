use criterion::criterion_main;

mod queue;

criterion_main!(queue::benches);
