mod entry;
mod error;
mod state;
mod verifier;

pub use entry::{COINBASE_MATURITY, UtxoEntry};
pub use error::UtxoError;
pub use state::UtxoState;
pub use verifier::SpendVerifier;

#[cfg(test)]
mod tests {
    use oregon_primitives::{Amount, Hash256, OutPoint, Transaction, TxInput, TxOutput};

    use super::{COINBASE_MATURITY, SpendVerifier, UtxoEntry, UtxoError, UtxoState};

    fn output_with_value(value: u64) -> TxOutput {
        TxOutput {
            value: Amount::from_base_units(value).unwrap(),
            locking_program: vec![0x01, 0x02],
        }
    }

    fn output() -> TxOutput {
        output_with_value(42)
    }

    fn outpoint(tag: u8, index: u32) -> OutPoint {
        OutPoint {
            txid: Hash256::from_bytes([tag; 32]),
            index,
        }
    }

    fn input(previous: OutPoint) -> TxInput {
        TxInput {
            previous_txid: previous.txid,
            previous_output_index: previous.index,
            sequence: 0,
            witness: vec![],
        }
    }

    fn spend(inputs: Vec<OutPoint>, outputs: &[u64]) -> Transaction {
        Transaction {
            version: 1,
            inputs: inputs.into_iter().map(input).collect(),
            outputs: outputs.iter().copied().map(output_with_value).collect(),
            lock_time: 0,
        }
    }

    fn entry(value: u64) -> UtxoEntry {
        UtxoEntry {
            output: output_with_value(value),
            creation_height: 1,
            is_coinbase: false,
        }
    }

    struct AcceptAll;

    impl SpendVerifier for AcceptAll {
        fn verify_spend(
            &self,
            _transaction: &Transaction,
            _input_index: usize,
            _prevout: &UtxoEntry,
        ) -> Result<(), UtxoError> {
            Ok(())
        }
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

    #[test]
    fn valid_spend_consumes_input_creates_outputs_and_returns_fee() {
        let previous = outpoint(0x11, 0);
        let mut state = UtxoState::new();
        state.insert_test_utxo(previous, entry(100));
        let tx = spend(vec![previous], &[60, 30]);

        let fee = state
            .apply_normal_transaction(&tx, 20, &AcceptAll)
            .expect("valid state transition");

        assert_eq!(fee, 10);
        assert!(state.get(&previous).is_none());
        assert!(state
            .get(&OutPoint { txid: tx.txid(), index: 0 })
            .is_some());
        assert!(state
            .get(&OutPoint { txid: tx.txid(), index: 1 })
            .is_some());
    }

    #[test]
    fn missing_utxo_is_rejected_without_state_change() {
        let present = outpoint(0x21, 0);
        let missing = outpoint(0x22, 0);
        let mut state = UtxoState::new();
        state.insert_test_utxo(present, entry(100));
        let before = state.clone();
        let tx = spend(vec![present, missing], &[90]);

        assert_eq!(
            state.apply_normal_transaction(&tx, 20, &AcceptAll),
            Err(UtxoError::MissingUtxo(missing))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn duplicate_input_is_rejected_without_state_change() {
        let previous = outpoint(0x31, 0);
        let mut state = UtxoState::new();
        state.insert_test_utxo(previous, entry(100));
        let before = state.clone();
        let tx = spend(vec![previous, previous], &[90]);

        assert_eq!(
            state.apply_normal_transaction(&tx, 20, &AcceptAll),
            Err(UtxoError::DuplicateInput(previous))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn outputs_cannot_exceed_inputs() {
        let previous = outpoint(0x41, 0);
        let mut state = UtxoState::new();
        state.insert_test_utxo(previous, entry(100));
        let before = state.clone();
        let tx = spend(vec![previous], &[101]);

        assert_eq!(
            state.apply_normal_transaction(&tx, 20, &AcceptAll),
            Err(UtxoError::OutputValueExceedsInput)
        );
        assert_eq!(state, before);
    }

    #[test]
    fn verifier_rejection_is_atomic() {
        let previous = outpoint(0x51, 0);
        let mut state = UtxoState::new();
        state.insert_test_utxo(previous, entry(100));
        let before = state.clone();
        let tx = spend(vec![previous], &[90]);

        assert_eq!(
            state.apply_normal_transaction(&tx, 20, &RejectAll),
            Err(UtxoError::SpendAuthorizationFailed)
        );
        assert_eq!(state, before);
    }
}
