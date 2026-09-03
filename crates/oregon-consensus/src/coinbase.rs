//! RED phase: tests define Oregon v1 coinbase and one-time founder rules.

#[cfg(test)]
mod tests {
    use oregon_primitives::{
        Amount, FOUNDER_ALLOCATION_BASE_UNITS, Hash256, Transaction, TxInput, TxOutput,
        write_varint,
    };

    use super::*;
    use crate::{ConsensusError, ConsensusParams, Target, block_subsidy};

    fn params() -> ConsensusParams {
        ConsensusParams::new(
            Target::from_le_bytes([0xff; 32]).unwrap(),
            Target::from_le_bytes([0x7f; 32]).unwrap(),
            [0x42; 32],
        )
        .unwrap()
    }

    fn founder_program(params: &ConsensusParams) -> Vec<u8> {
        let mut program = vec![crate::params::KEY_COMMIT_V1];
        program.extend_from_slice(&params.founder_key_commitment);
        program
    }

    fn coinbase(height: u64, outputs: Vec<TxOutput>) -> Transaction {
        let mut height_bytes = Vec::new();
        write_varint(height, &mut height_bytes);
        Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_txid: Hash256::from_bytes([0u8; 32]),
                previous_output_index: u32::MAX,
                sequence: u32::MAX,
                witness: vec![height_bytes],
            }],
            outputs,
            lock_time: 0,
        }
    }

    #[test]
    fn height_one_founder_output_is_exact() {
        let params = params();
        let tx = coinbase(
            1,
            vec![TxOutput {
                value: Amount::from_base_units(FOUNDER_ALLOCATION_BASE_UNITS).unwrap(),
                locking_program: founder_program(&params),
            }],
        );
        assert_eq!(
            validate_coinbase(&tx, 1, Amount::from_base_units(0).unwrap(), &params),
            Ok(())
        );
    }

    #[test]
    fn founder_value_or_index_mutation_is_rejected() {
        let params = params();
        let wrong_value = coinbase(
            1,
            vec![TxOutput {
                value: Amount::from_base_units(FOUNDER_ALLOCATION_BASE_UNITS - 1).unwrap(),
                locking_program: founder_program(&params),
            }],
        );
        assert_eq!(
            validate_coinbase(
                &wrong_value,
                1,
                Amount::from_base_units(0).unwrap(),
                &params
            ),
            Err(ConsensusError::InvalidFounderOutput)
        );

        let founder_at_index_one = coinbase(
            1,
            vec![
                TxOutput {
                    value: Amount::from_base_units(1).unwrap(),
                    locking_program: vec![],
                },
                TxOutput {
                    value: Amount::from_base_units(FOUNDER_ALLOCATION_BASE_UNITS).unwrap(),
                    locking_program: founder_program(&params),
                },
            ],
        );
        assert_eq!(
            validate_coinbase(
                &founder_at_index_one,
                1,
                Amount::from_base_units(0).unwrap(),
                &params
            ),
            Err(ConsensusError::InvalidFounderOutput)
        );
    }

    #[test]
    fn height_two_has_no_special_founder_mint() {
        let params = params();
        let tx = coinbase(
            2,
            vec![TxOutput {
                value: Amount::from_base_units(FOUNDER_ALLOCATION_BASE_UNITS).unwrap(),
                locking_program: founder_program(&params),
            }],
        );
        assert_eq!(
            validate_coinbase(&tx, 2, Amount::from_base_units(0).unwrap(), &params),
            Err(ConsensusError::CoinbaseOverClaim)
        );
    }

    #[test]
    fn canonical_height_witness_is_required() {
        let params = params();
        let mut tx = coinbase(
            1,
            vec![TxOutput {
                value: Amount::from_base_units(FOUNDER_ALLOCATION_BASE_UNITS).unwrap(),
                locking_program: founder_program(&params),
            }],
        );
        tx.inputs[0].witness[0].push(0);
        assert_eq!(
            validate_coinbase(&tx, 1, Amount::from_base_units(0).unwrap(), &params),
            Err(ConsensusError::InvalidCoinbase)
        );
    }

    #[test]
    fn underclaim_is_valid_but_overclaim_is_invalid() {
        let params = params();
        let fees = Amount::from_base_units(100).unwrap();
        let ceiling = block_subsidy(2).unwrap().base_units() + fees.base_units();

        let under = coinbase(
            2,
            vec![TxOutput {
                value: Amount::from_base_units(ceiling - 1).unwrap(),
                locking_program: vec![],
            }],
        );
        assert_eq!(validate_coinbase(&under, 2, fees, &params), Ok(()));

        let over = coinbase(
            2,
            vec![TxOutput {
                value: Amount::from_base_units(ceiling + 1).unwrap(),
                locking_program: vec![],
            }],
        );
        assert_eq!(
            validate_coinbase(&over, 2, fees, &params),
            Err(ConsensusError::CoinbaseOverClaim)
        );
    }
}
