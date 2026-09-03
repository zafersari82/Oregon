//! RED phase: non-genesis structural block consensus tests.

#[cfg(test)]
mod tests {
    use oregon_primitives::{
        Amount, Block, BlockHeader, Hash256, Transaction, TxInput, TxOutput, transaction_root,
        write_varint,
    };

    use super::*;
    use crate::{ConsensusError, ConsensusParams, Target};

    fn params() -> ConsensusParams {
        ConsensusParams::new(
            Target::from_le_bytes([0xff; 32]).unwrap(),
            Target::from_le_bytes([0x7f; 32]).unwrap(),
            [0x42; 32],
        )
        .unwrap()
    }

    fn coinbase(height: u64) -> Transaction {
        let mut height_bytes = Vec::new();
        write_varint(height, &mut height_bytes);
        Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_txid: Hash256::from_bytes([0; 32]),
                previous_output_index: u32::MAX,
                sequence: u32::MAX,
                witness: vec![height_bytes],
            }],
            outputs: vec![TxOutput {
                value: Amount::from_base_units(1).unwrap(),
                locking_program: vec![],
            }],
            lock_time: 0,
        }
    }

    fn normal_transaction(witness_bytes: usize) -> Transaction {
        Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_txid: Hash256::from_bytes([0x11; 32]),
                previous_output_index: 0,
                sequence: 0,
                witness: vec![vec![0x55; witness_bytes]],
            }],
            outputs: vec![TxOutput {
                value: Amount::from_base_units(1).unwrap(),
                locking_program: vec![],
            }],
            lock_time: 0,
        }
    }

    fn block(transactions: Vec<Transaction>) -> Block {
        let root = transaction_root(&transactions).unwrap();
        Block {
            header: BlockHeader {
                version: 1,
                previous_block: Hash256::from_bytes([0x22; 32]),
                transaction_root: root,
                timestamp: 1_800_000_600,
                difficulty_commitment: [0x33; 32],
                nonce: 9,
            },
            transactions,
        }
    }

    #[test]
    fn valid_small_block_passes() {
        let block = block(vec![coinbase(2), normal_transaction(0)]);
        assert_eq!(
            validate_non_genesis_block_structure(
                &block,
                2,
                Amount::from_base_units(0).unwrap(),
                &params(),
            ),
            Ok(())
        );
    }

    #[test]
    fn changed_merkle_root_fails() {
        let mut block = block(vec![coinbase(2), normal_transaction(0)]);
        block.header.transaction_root = Hash256::from_bytes([0x99; 32]);
        assert_eq!(
            validate_non_genesis_block_structure(
                &block,
                2,
                Amount::from_base_units(0).unwrap(),
                &params(),
            ),
            Err(ConsensusError::MerkleRootMismatch)
        );
    }

    #[test]
    fn second_coinbase_form_fails() {
        let block = block(vec![coinbase(2), coinbase(2)]);
        assert_eq!(
            validate_non_genesis_block_structure(
                &block,
                2,
                Amount::from_base_units(0).unwrap(),
                &params(),
            ),
            Err(ConsensusError::MultipleCoinbase)
        );
    }

    #[test]
    fn normal_transaction_null_outpoint_fails() {
        let null_input = TxInput {
            previous_txid: Hash256::from_bytes([0; 32]),
            previous_output_index: u32::MAX,
            sequence: 0,
            witness: vec![],
        };
        let non_null_input = TxInput {
            previous_txid: Hash256::from_bytes([0x44; 32]),
            previous_output_index: 1,
            sequence: 0,
            witness: vec![],
        };
        let normal = Transaction {
            version: 1,
            inputs: vec![null_input, non_null_input],
            outputs: vec![TxOutput {
                value: Amount::from_base_units(1).unwrap(),
                locking_program: vec![],
            }],
            lock_time: 0,
        };
        let block = block(vec![coinbase(2), normal]);

        assert_eq!(
            validate_non_genesis_block_structure(
                &block,
                2,
                Amount::from_base_units(0).unwrap(),
                &params(),
            ),
            Err(ConsensusError::NullOutpointInNormalTransaction)
        );
    }

    #[test]
    fn transaction_over_102400_bytes_fails() {
        let block = block(vec![coinbase(2), normal_transaction(102_400)]);
        assert!(block.transactions[1].encode().len() > 102_400);
        assert!(block.encode().len() < 1_048_576);
        assert_eq!(
            validate_non_genesis_block_structure(
                &block,
                2,
                Amount::from_base_units(0).unwrap(),
                &params(),
            ),
            Err(ConsensusError::TransactionTooLarge(1))
        );
    }

    #[test]
    fn block_over_1048576_bytes_fails_before_transaction_checks() {
        let mut transactions = vec![coinbase(2)];
        for _ in 0..12 {
            transactions.push(normal_transaction(90_000));
        }
        assert!(transactions.iter().all(|tx| tx.encode().len() <= 102_400));
        let block = block(transactions);
        assert!(block.encode().len() > 1_048_576);
        assert_eq!(
            validate_non_genesis_block_structure(
                &block,
                2,
                Amount::from_base_units(0).unwrap(),
                &params(),
            ),
            Err(ConsensusError::BlockTooLarge)
        );
    }

    #[test]
    fn height_zero_is_invalid() {
        let block = block(vec![coinbase(2)]);
        assert_eq!(
            validate_non_genesis_block_structure(
                &block,
                0,
                Amount::from_base_units(0).unwrap(),
                &params(),
            ),
            Err(ConsensusError::InvalidHeight)
        );
    }
}
