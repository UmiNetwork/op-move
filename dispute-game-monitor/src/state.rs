use {
    alloy::primitives::Address,
    std::{
        collections::VecDeque,
        time::{Duration, SystemTime},
    },
};

pub struct State {
    pub game_count: u64,
    pub awaiting_resolution: VecDeque<(Address, SystemTime)>,
}

impl State {
    pub fn new(game_count: u64, awaiting_resolution: VecDeque<(Address, SystemTime)>) -> Self {
        Self {
            game_count,
            awaiting_resolution,
        }
    }

    pub fn empty_queue(game_count: u64) -> Self {
        Self {
            game_count,
            awaiting_resolution: VecDeque::new(),
        }
    }

    pub fn push(&mut self, address: Address, timestamp: u64) {
        self.awaiting_resolution.push_back((
            address,
            SystemTime::UNIX_EPOCH + Duration::from_secs(timestamp),
        ));
    }
}
