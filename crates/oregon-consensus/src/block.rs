use oregon_primitives::{Amount, Block, Hash256, transaction_root};

use crate::{
    ConsensusError, ConsensusParams,
    coinbase::{is_coinbase_form, validate_coinbase},
    params::{MAX_BLOCK_BYTES, MAX_TRANSACTION_BYTES},
};

pub fn validate_non_genesis_block_skeleton(
    block: &Block,
    height: u64,
) -> Result<(), ConsensusError> {
    if height == 0 {
        return Err(ConsensusError::InvalidHeight);
    }
    if block.encode().len() > MAX_BLOCK_BYTES {
        return Err(ConsensusError::BlockTooLarge);
    }
    if block.transactions.is_empty() {
        return Err(ConsensusError::EmptyNonGenesisBlock);
    }

    for (index, transaction) in block.transactions.iter().enumerate() {
        if transaction.encode().len() > MAX_TRANSACTION_BYTES {
            return Err(ConsensusError::TransactionTooLarge(index));
        }
    }

    let root =
        transaction_root(&block.transactions).map_err(|_| ConsensusError::MerkleRootMismatch)?;
    if root != block.header.transaction_root {
        return Err(ConsensusError::MerkleRootMismatch);
    }

    if !is_coinbase_form(&block.transactions[0]) {
        return Err(ConsensusError::InvalidCoinbase);
    }

    let null_txid = Hash256::from_bytes([0u8; 32]);
    for (index, transaction) in block.transactions.iter().enumerate().skip(1) {
        if transaction.inputs.is_empty() {
            return Err(ConsensusError::EmptyNormalTransactionInputs(index));
        }
        if transaction.outputs.is_empty() {
            return Err(ConsensusError::EmptyNormalTransactionOutputs(index));
        }
        if is_coinbase_form(transaction) {
            return Err(ConsensusError::MultipleCoinbase);
        }
        if transaction.inputs.iter().any(|input| {
            input.previous_txid == null_txid && input.previous_output_index == u32::MAX
        }) {
            return Err(ConsensusError::NullOutpointInNormalTransaction);
        }
    }

    Ok(())
}

pub fn validate_non_genesis_block_structure(
    block: &Block,
    height: u64,
    total_fees: Amount,
    params: &ConsensusParams,
) -> Result<(), ConsensusError> {
    validate_non_genesis_block_skeleton(block, height)?;
    validate_coinbase(&block.transactions[0], height, total_fees, params)?;
    Ok(())
}

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
    fn normal_helper_accepts_valid_normal_transaction() {
        assert_eq!(validate_normal_transaction_skeleton(&normal_transaction(0)), Ok(()));
    }

    #[test]
    fn normal_helper_rejects_empty_inputs() {
        let tx = Transaction {
            version: 1,
            inputs: vec![],
            outputs: vec![TxOutput {
                value: Amount::from_base_units(1).unwrap(),
                locking_program: vec![],
            }],
            lock_time: 0,
        };
        assert_eq!(
            validate_normal_transaction_skeleton(&tx),
            Err(NormalTransactionError::EmptyInputs)
        );
    }

    #[test]
    fn normal_helper_rejects_empty_outputs() {
        let tx = Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_txid: Hash256::from_bytes([0x11; 32]),
                previous_output_index: 0,
                sequence: 0,
                witness: vec![],
            }],
            outputs: vec![],
            lock_time: 0,
        };
        assert_eq!(
            validate_normal_transaction_skeleton(&tx),
            Err(NormalTransactionError::EmptyOutputs)
        );
    }

    #[test]
    fn normal_helper_rejects_coinbase_form() {
        assert_eq!(
            validate_normal_transaction_skeleton(&coinbase(2)),
            Err(NormalTransactionError::CoinbaseForm)
        );
    }

    #[test]
    fn normal_helper_rejects_null_outpoint() {
        let tx = Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_txid: Hash256::from_bytes([0; 32]),
                previous_output_index: u32::MAX,
                sequence: 0,
                witness: vec![],
            }],
            outputs: vec![TxOutput {
                value: Amount::from_base_units(1).unwrap(),
                locking_program: vec![],
            }],
            lock_time: 0,
        };
        assert_eq!(
            validate_normal_transaction_skeleton(&tx),
            Err(NormalTransactionError::NullOutpoint)
        );
    }

    #[test]
    fn normal_helper_rejects_oversized_transaction() {
        let tx = normal_transaction(102_400);
        assert!(tx.encode().len() > MAX_TRANSACTION_BYTES);
        assert_eq!(
            validate_normal_transaction_skeleton(&tx),
            Err(NormalTransactionError::TooLarge)
        );
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
