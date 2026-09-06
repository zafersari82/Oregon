use oregon_contract_state::{
    DomainSnapshot, ExecutionAccountingWritesV1, apply_write_set, balance_key,
    empty_hashes, encode_accounting_u64, encode_reserve_outpoint, read_value,
    reserve_outpoint_key, sequence_key, total_execution_balance_key,
};
use oregon_primitives::execution_address::{ExecutionAddress, ExecutionAddressKind};
use oregon_primitives::state_commitment::CommitmentDomainId;
use oregon_primitives::{Hash256, OutPoint};

mod support;
use support::MemorySource;

fn address(tag: u8) -> ExecutionAddress {
    ExecutionAddress::new(ExecutionAddressKind::Oregon, [tag; 32]).unwrap()
}

fn empty_snapshot(domain: CommitmentDomainId) -> DomainSnapshot {
    DomainSnapshot {
        domain,
        root: empty_hashes(domain)[0],
    }
}

#[test]
fn accounting_keys_and_value_codecs_are_exact() {
    let account = address(0x21);

    let mut expected_balance = b"acct/v1/balance/".to_vec();
    expected_balance.extend_from_slice(&account.to_bytes());
    assert_eq!(balance_key(&account), expected_balance);

    let mut expected_sequence = b"acct/v1/sequence/".to_vec();
    expected_sequence.extend_from_slice(&account.to_bytes());
    assert_eq!(sequence_key(&account), expected_sequence);

    assert_eq!(
        total_execution_balance_key(),
        b"acct/v1/total_execution_balance"
    );
    assert_eq!(reserve_outpoint_key(), b"acct/v1/reserve_outpoint");
    assert_eq!(encode_accounting_u64(42), 42u64.to_le_bytes().to_vec());
    assert_eq!(encode_reserve_outpoint(None), vec![0x00]);

    let outpoint = OutPoint {
        txid: Hash256::from_bytes([0x34; 32]),
        index: 0x7856_3412,
    };
    let mut expected_outpoint = vec![0x01];
    expected_outpoint.extend_from_slice(outpoint.txid.as_bytes());
    expected_outpoint.extend_from_slice(&outpoint.index.to_le_bytes());
    assert_eq!(encode_reserve_outpoint(Some(outpoint)), expected_outpoint);
}

#[test]
fn accounting_write_set_uses_execution_accounting_domain_and_zero_balance_is_absence() {
    let keep = address(0x31);
    let remove = address(0x32);
    let reserve = OutPoint {
        txid: Hash256::from_bytes([0x44; 32]),
        index: 7,
    };
    let writes = ExecutionAccountingWritesV1 {
        balances: vec![(keep, Some(900)), (remove, Some(0))],
        sequences: vec![(keep, 12), (remove, 13)],
        total_execution_balance: 900,
        reserve_outpoint: Some(reserve),
    }
    .write_set()
    .unwrap();

    assert_eq!(writes.domain(), CommitmentDomainId::ExecutionAccounting);
    assert_eq!(writes.len(), 6);

    let source = MemorySource::default();
    let transition = apply_write_set(
        &source,
        empty_snapshot(CommitmentDomainId::ExecutionAccounting),
        &writes,
    )
    .unwrap();
    let snapshot = DomainSnapshot {
        domain: CommitmentDomainId::ExecutionAccounting,
        root: transition.new_root,
    };
    let mut committed = MemorySource::default();
    committed.absorb(&transition);

    assert_eq!(
        read_value(&committed, snapshot, &balance_key(&keep)).unwrap(),
        Some(900u64.to_le_bytes().to_vec())
    );
    assert_eq!(
        read_value(&committed, snapshot, &balance_key(&remove)).unwrap(),
        None
    );
    assert_eq!(
        read_value(&committed, snapshot, &sequence_key(&keep)).unwrap(),
        Some(12u64.to_le_bytes().to_vec())
    );
    assert_eq!(
        read_value(&committed, snapshot, total_execution_balance_key()).unwrap(),
        Some(900u64.to_le_bytes().to_vec())
    );
    assert_eq!(
        read_value(&committed, snapshot, reserve_outpoint_key()).unwrap(),
        Some(encode_reserve_outpoint(Some(reserve)))
    );
}

#[test]
fn identical_accounting_writes_have_identical_smt_roots_independent_of_input_order() {
    let left = address(0x41);
    let right = address(0x42);
    let reserve = OutPoint {
        txid: Hash256::from_bytes([0x55; 32]),
        index: 3,
    };

    let first = ExecutionAccountingWritesV1 {
        balances: vec![(left, Some(10)), (right, Some(20))],
        sequences: vec![(left, 1), (right, 2)],
        total_execution_balance: 30,
        reserve_outpoint: Some(reserve),
    }
    .write_set()
    .unwrap();
    let reversed = ExecutionAccountingWritesV1 {
        balances: vec![(right, Some(20)), (left, Some(10))],
        sequences: vec![(right, 2), (left, 1)],
        total_execution_balance: 30,
        reserve_outpoint: Some(reserve),
    }
    .write_set()
    .unwrap();

    let source = MemorySource::default();
    let snapshot = empty_snapshot(CommitmentDomainId::ExecutionAccounting);
    let first_transition = apply_write_set(&source, snapshot, &first).unwrap();
    let reversed_transition = apply_write_set(&source, snapshot, &reversed).unwrap();

    assert_eq!(first_transition.new_root, reversed_transition.new_root);
    assert_eq!(first_transition.nodes, reversed_transition.nodes);
    assert_eq!(first_transition.values, reversed_transition.values);
}
