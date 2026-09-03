use oregon_primitives::Transaction;

use crate::{UtxoEntry, UtxoError};

pub trait SpendVerifier {
    fn verify_spend(
        &self,
        transaction: &Transaction,
        input_index: usize,
        prevout: &UtxoEntry,
    ) -> Result<(), UtxoError>;
}
