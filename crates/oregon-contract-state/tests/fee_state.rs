use oregon_contract_state::{
    DomainSnapshot, FEE_BASE_FEE_PER_WEIGHT_KEY_V1, FEE_BLOCK_WEIGHT_USED_KEY_V1,
    FEE_EXECUTION_FEE_TOTAL_KEY_V1, FEE_HEIGHT_KEY_V1, FEE_PRODUCER_COINBASE_TXID_KEY_V1,
    FEE_RESERVE_TRANSITION_ID_KEY_V1, FeeStateValuesV1, StateWrite, StateWriteSet,
    apply_write_set, empty_hashes, read_value,
};
use oregon_primitives::Hash256;
use oregon_primitives::state_commitment::CommitmentDomainId;

mod support;
use support::MemorySource;

fn values() -> FeeStateValuesV1 {
    FeeStateValuesV1 {
        height: 123,
        base_fee_per_weight: 17,
        block_weight_used: 456,
        execution_fee_total: 789,
        producer_coinbase_txid: Hash256::from_bytes([0x61; 32]),
        reserve_transition_id: Hash256::from_bytes([0x62; 32]),
    }
}

fn empty_snapshot(domain: CommitmentDomainId) -> DomainSnapshot {
    DomainSnapshot {
        domain,
        root: empty_hashes(domain)[0],
    }
}

#[test]
fn fee_state_raw_keys_are_exact() {
    assert_eq!(FEE_HEIGHT_KEY_V1, b"fee/v1/height");
    assert_eq!(
        FEE_BASE_FEE_PER_WEIGHT_KEY_V1,
        b"fee/v1/base_fee_per_weight"
    );
    assert_eq!(FEE_BLOCK_WEIGHT_USED_KEY_V1, b"fee/v1/block_weight_used");
    assert_eq!(
        FEE_EXECUTION_FEE_TOTAL_KEY_V1,
        b"fee/v1/execution_fee_total"
    );
    assert_eq!(
        FEE_PRODUCER_COINBASE_TXID_KEY_V1,
        b"fee/v1/producer_coinbase_txid"
    );
    assert_eq!(
        FEE_RESERVE_TRANSITION_ID_KEY_V1,
        b"fee/v1/reserve_transition_id"
    );
}

#[test]
fn fee_state_write_set_contains_exactly_six_canonical_values() {
    let values = values();
    let writes = values.write_set().unwrap();
    assert_eq!(writes.domain(), CommitmentDomainId::FeeState);
    assert_eq!(writes.len(), 6);

    let source = MemorySource::default();
    let transition = apply_write_set(
        &source,
        empty_snapshot(CommitmentDomainId::FeeState),
        &writes,
    )
    .unwrap();
    let snapshot = DomainSnapshot {
        domain: CommitmentDomainId::FeeState,
        root: transition.new_root,
    };
    let mut committed = MemorySource::default();
    committed.absorb(&transition);

    assert_eq!(
        read_value(&committed, snapshot, FEE_HEIGHT_KEY_V1).unwrap(),
        Some(123u64.to_le_bytes().to_vec())
    );
    assert_eq!(
        read_value(&committed, snapshot, FEE_BASE_FEE_PER_WEIGHT_KEY_V1).unwrap(),
        Some(17u64.to_le_bytes().to_vec())
    );
    assert_eq!(
        read_value(&committed, snapshot, FEE_BLOCK_WEIGHT_USED_KEY_V1).unwrap(),
        Some(456u64.to_le_bytes().to_vec())
    );
    assert_eq!(
        read_value(&committed, snapshot, FEE_EXECUTION_FEE_TOTAL_KEY_V1).unwrap(),
        Some(789u64.to_le_bytes().to_vec())
    );
    assert_eq!(
        read_value(&committed, snapshot, FEE_PRODUCER_COINBASE_TXID_KEY_V1).unwrap(),
        Some(vec![0x61; 32])
    );
    assert_eq!(
        read_value(&committed, snapshot, FEE_RESERVE_TRANSITION_ID_KEY_V1).unwrap(),
        Some(vec![0x62; 32])
    );
}

#[test]
fn identical_fee_state_writes_are_deterministic_and_domain_separated() {
    let values = values();
    let fee_writes = values.write_set().unwrap();
    let source = MemorySource::default();
    let fee_snapshot = empty_snapshot(CommitmentDomainId::FeeState);
    let first = apply_write_set(&source, fee_snapshot, &fee_writes).unwrap();
    let second = apply_write_set(&source, fee_snapshot, &values.write_set().unwrap()).unwrap();
    assert_eq!(first.new_root, second.new_root);
    assert_eq!(first.nodes, second.nodes);
    assert_eq!(first.values, second.values);

    let same_raw_writes = StateWriteSet::new(
        CommitmentDomainId::ExecutionAccounting,
        vec![
            StateWrite::put(FEE_HEIGHT_KEY_V1.to_vec(), 123u64.to_le_bytes().to_vec()),
            StateWrite::put(
                FEE_BASE_FEE_PER_WEIGHT_KEY_V1.to_vec(),
                17u64.to_le_bytes().to_vec(),
            ),
            StateWrite::put(
                FEE_BLOCK_WEIGHT_USED_KEY_V1.to_vec(),
                456u64.to_le_bytes().to_vec(),
            ),
            StateWrite::put(
                FEE_EXECUTION_FEE_TOTAL_KEY_V1.to_vec(),
                789u64.to_le_bytes().to_vec(),
            ),
            StateWrite::put(FEE_PRODUCER_COINBASE_TXID_KEY_V1.to_vec(), vec![0x61; 32]),
            StateWrite::put(FEE_RESERVE_TRANSITION_ID_KEY_V1.to_vec(), vec![0x62; 32]),
        ],
    )
    .unwrap();
    let accounting = apply_write_set(
        &source,
        empty_snapshot(CommitmentDomainId::ExecutionAccounting),
        &same_raw_writes,
    )
    .unwrap();

    assert_ne!(first.new_root, accounting.new_root);
}
