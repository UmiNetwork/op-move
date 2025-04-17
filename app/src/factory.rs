use {
    crate::{Application, CommandActor, DependenciesThreadSafe, queue::CommandQueue},
    std::sync::Arc,
    tokio::sync::{RwLock, broadcast, mpsc},
};

pub fn create<T: DependenciesThreadSafe>(
    app: Arc<RwLock<Application<T>>>,
    buffer: u32,
) -> (CommandQueue, CommandActor<T>) {
    let (ktx, _) = broadcast::channel(1);
    let (tx, rx) = mpsc::channel(buffer as usize);

    (CommandQueue::new(tx, ktx), CommandActor::new(rx, app))
}
