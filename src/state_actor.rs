use {
    crate::types::{engine_api::PayloadId, state::StateMessage},
    ethers_core::types::H256,
    tokio::{sync::mpsc::Receiver, task::JoinHandle},
};

#[derive(Debug)]
pub struct StateActor {
    rx: Receiver<StateMessage>,
    head: H256,
    payload_id: PayloadId,
}

impl StateActor {
    pub fn new(rx: Receiver<StateMessage>) -> Self {
        Self {
            rx,
            head: Default::default(),
            payload_id: PayloadId(Default::default()),
        }
    }

    pub fn spawn(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(msg) = self.rx.recv().await {
                match msg {
                    StateMessage::UpdateHead { block_hash } => {
                        self.head = block_hash;
                    }
                    StateMessage::SetPayloadId { id } => {
                        self.payload_id = id;
                    }
                    StateMessage::StartBlockBuild {
                        payload,
                        response_channel,
                    } => {
                        response_channel.send(self.payload_id.clone()).ok();
                        // TODO: do something with the payload to produce a new block
                    }
                }
            }
        })
    }
}
