use oregon_consensus::{
    ConsensusError, ConsensusParams, Target, block_subsidy, validate_coinbase,
    validate_coinbase_with_execution_fees_v1,
};
use oregon_primitives::execution_reserve::EXECUTION_RESERVE_LOCKING_PROGRAM_V1;
use oregon_primitives::{Amount, Hash256, Transaction, TxInput, TxOutput, write_varint};

fn params() -> ConsensusParams {
    ConsensusParams::new(
        Target::from_le_bytes([0xff; 32]).unwrap(),
        Target::from_le_bytes([0x7f; 32]).unwrap(),
        [0x42; 32],
    )
    .unwrap()
}

fn output(value: u64, locking_program: Vec<u8>) -> TxOutput {
    TxOutput {
        value: Amount::from_base_units(value).unwrap(),
        locking_program,
    }
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

fn amount(value: u64) -> Amount {
    Amount::from_base_units(value).unwrap()
}

#[test]
fn active_validator_still_allows_subsidy_underclaim_but_execution_fee_helper_never_allows_fee_underclaim()
 {
    let params = params();
    let native_fees = amount(100);
    let execution_fees = amount(50);
    let tx = coinbase(2, vec![output(99, vec![0x01]), output(50, vec![0x02])]);

    assert_eq!(validate_coinbase(&tx, 2, amount(150), &params), Ok(()));
    assert_eq!(
        validate_coinbase_with_execution_fees_v1(&tx, 2, native_fees, execution_fees, &params),
        Err(ConsensusError::InvalidCoinbase)
    );
}

#[test]
fn nonzero_execution_fees_require_an_exact_final_dedicated_output() {
    let params = params();
    let subsidy = block_subsidy(2).unwrap().base_units();

    let missing = coinbase(2, vec![output(subsidy + 150, vec![0x01])]);
    assert_eq!(
        validate_coinbase_with_execution_fees_v1(&missing, 2, amount(100), amount(50), &params),
        Err(ConsensusError::InvalidCoinbase)
    );

    let wrong_value = coinbase(
        2,
        vec![output(subsidy + 100, vec![0x01]), output(49, vec![0x02])],
    );
    assert_eq!(
        validate_coinbase_with_execution_fees_v1(&wrong_value, 2, amount(100), amount(50), &params),
        Err(ConsensusError::InvalidCoinbase)
    );
}

#[test]
fn reserve_program_cannot_receive_execution_fee_payout() {
    let params = params();
    let subsidy = block_subsidy(2).unwrap().base_units();
    let tx = coinbase(
        2,
        vec![
            output(subsidy + 100, vec![0x01]),
            output(50, EXECUTION_RESERVE_LOCKING_PROGRAM_V1.to_vec()),
        ],
    );

    assert_eq!(
        validate_coinbase_with_execution_fees_v1(&tx, 2, amount(100), amount(50), &params),
        Err(ConsensusError::InvalidCoinbase)
    );
}

#[test]
fn combined_native_and_execution_fees_still_obey_the_existing_coinbase_ceiling() {
    let params = params();
    let subsidy = block_subsidy(2).unwrap().base_units();
    let tx = coinbase(
        2,
        vec![output(subsidy + 101, vec![0x01]), output(50, vec![0x02])],
    );

    assert_eq!(
        validate_coinbase_with_execution_fees_v1(&tx, 2, amount(100), amount(50), &params),
        Err(ConsensusError::CoinbaseOverClaim)
    );
}

#[test]
fn zero_execution_fees_need_no_dedicated_final_output() {
    let params = params();
    let subsidy = block_subsidy(2).unwrap().base_units();
    let tx = coinbase(2, vec![output(subsidy + 100, vec![0x01])]);

    assert_eq!(
        validate_coinbase_with_execution_fees_v1(&tx, 2, amount(100), amount(0), &params),
        Ok(())
    );
}

#[test]
fn exact_execution_fee_output_and_total_fee_claim_are_valid() {
    let params = params();
    let subsidy = block_subsidy(2).unwrap().base_units();
    let tx = coinbase(
        2,
        vec![output(subsidy + 100, vec![0x01]), output(50, vec![0x02])],
    );

    assert_eq!(
        validate_coinbase_with_execution_fees_v1(&tx, 2, amount(100), amount(50), &params),
        Ok(())
    );
}
