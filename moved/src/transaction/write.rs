use {op_alloy::consensus::OpTxEnvelope, std::fmt::Debug};

#[derive(Debug, Clone)]
pub struct Transaction(pub OpTxEnvelope);

pub trait TransactionRepository {
    type Err: Debug;
    type Storage;

    fn add(&mut self, storage: &mut Self::Storage, tx: Transaction) -> Result<(), Self::Err>;
}
