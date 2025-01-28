use {op_alloy::consensus::OpTxEnvelope, std::fmt::Debug};

#[derive(Debug, Clone)]
pub struct Transaction(pub OpTxEnvelope);

impl Transaction {
    pub fn new(inner: OpTxEnvelope) -> Self {
        Self(inner)
    }
}

pub trait TransactionRepository {
    type Err: Debug;
    type Storage;

    fn add(
        &mut self,
        storage: &mut Self::Storage,
        transaction: Transaction,
    ) -> Result<(), Self::Err>;

    fn extend(
        &mut self,
        storage: &mut Self::Storage,
        transactions: impl IntoIterator<Item = Transaction>,
    ) -> Result<(), Self::Err>;
}
