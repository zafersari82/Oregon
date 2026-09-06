mod common;

use common::{MemorySource, seed_snapshot};
use oregon_contract_state::{StateError, StateWrite, value_hash};
use oregon_execution::{
    ExecutionJournalV1, JournalContextV1, JournalError, JournalLimitsV1,
};
use oregon_primitives::Hash256;
use oregon_primitives::state_commitment::CommitmentDomainId;

fn context() -> JournalContextV1 {
    JournalContextV1 {
        chain_id: 7,
        height: 42,
        parent_block_hash: Hash256::from_bytes([0x11; 32]),
        txid: Hash256::from_bytes([0x22; 32]),
    }
}

#[test]
fn delete_shadows_persisted_base_value() {
    let mut source = MemorySource::default();
    let domain = CommitmentDomainId::Wasm;
    let snapshot = seed_snapshot(
        &mut source,
        domain,
        vec![StateWrite::put(b"key".to_vec(), b"persisted".to_vec())],
    );
    let snapshots = [snapshot];
    let mut journal = ExecutionJournalV1::new(
        &source,
        context(),
        &snapshots,
        JournalLimitsV1::default(),
    )
    .unwrap();

    assert_eq!(
        journal.read(domain, b"key").unwrap(),
        Some(b"persisted".to_vec())
    );
    journal.delete(domain, b"key").unwrap();
    assert_eq!(journal.read(domain, b"key").unwrap(), None);
}

#[test]
fn base_reads_reject_corrupt_authoritative_values() {
    let mut source = MemorySource::default();
    let domain = CommitmentDomainId::Wasm;
    let snapshot = seed_snapshot(
        &mut source,
        domain,
        vec![StateWrite::put(b"key".to_vec(), b"persisted".to_vec())],
    );
    let expected_hash = value_hash(domain, b"persisted").unwrap();
    source.values.insert(expected_hash, b"tampered".to_vec());

    let snapshots = [snapshot];
    let journal = ExecutionJournalV1::new(
        &source,
        context(),
        &snapshots,
        JournalLimitsV1::default(),
    )
    .unwrap();

    assert!(matches!(
        journal.read(domain, b"key"),
        Err(JournalError::State(StateError::ValueHashMismatch(found)))
            if found == expected_hash
    ));
}
