mod entry;
mod error;
mod verifier;

pub use entry::{COINBASE_MATURITY, UtxoEntry};
pub use error::UtxoError;
pub use verifier::SpendVerifier;

#[cfg(test)]
mod tests {
    use oregon_primitives::{Amount, Transaction, TxOutput};

    use super::{COINBASE_MATURITY, SpendVerifier, UtxoEntry, UtxoError};

    fn output() -> TxOutput {
        TxOutput {
            value: Amount::from_base_units(42).unwrap(),
            locking_program: vec![0x01, 0x02],
        }
    }

    #[test]
    fn non_coinbase_is_immediately_spendable() {
        let entry = UtxoEntry {
            output: output(),
            creation_height: 10,
            is_coinbase: false,
        };
        assert!(entry.is_spendable_at(10));
    }

    #[test]
    fn coinbase_requires_exactly_120_blocks_of_maturity() {
        assert_eq!(COINBASE_MATURITY, 120);
        let entry = UtxoEntry {
            output: output(),
            creation_height: 10,
            is_coinbase: true,
        };
        assert!(!entry.is_spendable_at(129));
        assert!(entry.is_spendable_at(130));
    }

    struct RejectAll;

    impl SpendVerifier for RejectAll {
        fn verify_spend(
            &self,
            _transaction: &Transaction,
            _input_index: usize,
            _prevout: &UtxoEntry,
        ) -> Result<(), UtxoError> {
            Err(UtxoError::SpendAuthorizationFailed)
        }
    }

    #[test]
    fn verifier_can_reject_a_spend() {
        let verifier = RejectAll;
        let tx = Transaction {
            version: 1,
            inputs: vec![],
            outputs: vec![],
            lock_time: 0,
        };
        let entry = UtxoEntry {
            output: output(),
            creation_height: 0,
            is_coinbase: false,
        };
        assert_eq!(
            verifier.verify_spend(&tx, 0, &entry),
            Err(UtxoError::SpendAuthorizationFailed)
        );
    }
}
