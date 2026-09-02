#[cfg(test)]
mod tests {
    use super::*;
    use crate::DecodeLimits;

    #[test]
    fn version_one_minimum_transaction_round_trips_exactly() {
        let tx = Transaction {
            version: 1,
            inputs: vec![],
            outputs: vec![],
            lock_time: 0,
        };

        let encoded = tx.encode();
        let decoded = Transaction::decode(&encoded, &DecodeLimits::default()).unwrap();
        assert_eq!(decoded, tx);
        assert_eq!(decoded.encode(), encoded);
    }

    #[test]
    fn version_zero_is_consensus_invalid() {
        let bytes = [
            0x00, 0x00, // version 0
            0x00, // input count
            0x00, // output count
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // lock_time
        ];

        assert_eq!(
            Transaction::decode(&bytes, &DecodeLimits::default()),
            Err(PrimitiveError::InvalidVersion(0))
        );
    }

    #[test]
    fn witness_bytes_commit_to_transaction_id() {
        let previous_txid = Hash256::from_bytes([0x11; 32]);
        let base = Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_txid,
                previous_output_index: 3,
                sequence: 7,
                witness: vec![vec![0xaa]],
            }],
            outputs: vec![TxOutput {
                value: Amount::from_base_units(42).unwrap(),
                locking_program: vec![0x51],
            }],
            lock_time: 9,
        };

        let mut changed = base.clone();
        changed.inputs[0].witness[0][0] = 0xab;

        assert_ne!(base.txid(), changed.txid());
    }
}
