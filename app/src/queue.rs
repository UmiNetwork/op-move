use {
    crate::Command,
    std::pin::Pin,
    tokio::sync::{broadcast, mpsc},
};

#[derive(Debug, Clone)]
pub struct CommandQueue {
    sender: mpsc::Sender<Command>,
    killshot: broadcast::Sender<()>,
}

impl CommandQueue {
    /// Constructs new [`CommandQueue`] that sends [`Command`]s via `sender`.
    ///
    /// In case of a panic of the `sender` channel, a shutdown signal is sent through the
    /// `killshot`.
    pub fn new(sender: mpsc::Sender<Command>, killshot: broadcast::Sender<()>) -> Self {
        Self { sender, killshot }
    }

    /// Sends a [`Command`] to the background queue for asynchronous processing.
    pub async fn send(&self, msg: Command) {
        if self.sender.send(msg).await.is_err() {
            self.shutdown();
        }
    }

    /// Waits for all the commands in the queue to be processed.
    pub async fn wait_for_pending_commands(&self) {
        if self
            .sender
            .reserve_many(self.sender.max_capacity())
            .await
            .is_err()
        {
            self.shutdown();
        }
    }

    /// Signals the shutdown.
    pub fn shutdown(&self) {
        self.killshot.send(()).ok();
    }

    /// Subscribes to the shutdown signal receiver.
    pub fn shutdown_listener(&self) -> Pin<Box<dyn Future<Output = ()> + Send + Sync>> {
        let mut rx = self.killshot.subscribe();

        Box::pin(async move {
            rx.recv().await.ok();
        })
    }
}
