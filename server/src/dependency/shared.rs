use super::*;

pub(super) type StateActor<A, B, C, D, E, F, G> = moved_app::StateActor<
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    BlockQueries,
    SharedStorage,
    StateQueries,
    TransactionRepository,
    TransactionQueries,
    ReceiptStorage,
    ReceiptRepository,
    ReceiptQueries,
    PayloadQueries,
    StorageTrieRepository,
>;
pub(super) type OnTxBatch<A, B, C, D, E, F, G> =
    moved_app::OnTxBatch<StateActor<A, B, C, D, E, F, G>>;
pub(super) type OnTx<A, B, C, D, E, F, G> = moved_app::OnTx<StateActor<A, B, C, D, E, F, G>>;
pub(super) type OnPayload<A, B, C, D, E, F, G> =
    moved_app::OnPayload<StateActor<A, B, C, D, E, F, G>>;
