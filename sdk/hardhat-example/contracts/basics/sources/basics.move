/// Sui module example
module 0x8fd379246834eac74b8419ffda202cf8051f7a03::basics {
    use sui::{clock::Clock, event};

    public struct TimeEvent has copy, drop, store {
        timestamp_ms: u64,
    }

    entry fun access(clock: &Clock) {
        event::emit(TimeEvent { timestamp_ms: clock.timestamp_ms() });
    }
}
