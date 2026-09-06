mod common;

use std::collections::BTreeMap;

use common::{MemorySource, empty_snapshot, seed_snapshot};
use oregon_contract_state::{
    DomainSnapshot, MAX_STATE_KEY_BYTES, MAX_STATE_VALUE_BYTES, StateError, StateWrite, read_value,
    value_hash,
};
use oregon_execution::{
    ExecutionJournalV1, JournalContextV1, JournalError, JournalIntentV1, JournalLimitsV1,
};
use oregon_primitives::Hash256;
use oregon_primitives::state_commitment::CommitmentDomainId;
use proptest::prelude::*;

fn context() -> JournalContextV1 {
    JournalContextV1 {
        chain_id: 7,
        height: 42,
        parent_block_hash: Hash256::from_bytes([0x11; 32]),
        txid: Hash256::from_bytes([0x22; 32]),
    }
}

#[test]
fn nested_commit_remains_revertible_by_its_parent() {
    let source = MemorySource::default();
    let domain = CommitmentDomainId::Wasm;
    let snapshots = [empty_snapshot(domain)];
    let mut journal = ExecutionJournalV1::new(
        &source,
        context(),
        &snapshots,
        JournalLimitsV1::default(),
    )
    .unwrap();

    journal.put(domain, b"key", b"parent").unwrap();
    journal.begin_frame().unwrap();
    journal.put(domain, b"key", b"child").unwrap();
    journal.begin_frame().unwrap();
    journal.put(domain, b"key", b"grandchild").unwrap();
    journal.commit_frame().unwrap();
    journal.revert_frame().unwrap();

    assert_eq!(journal.read(domain, b"key").unwrap(), Some(b"parent".to_vec()));
}

#[test]
fn reads_follow_overlay_visibility_and_keep_siblings_isolated() {
    let mut source = MemorySource::default();
    let domain = CommitmentDomainId::Wasm;
    let snapshot = seed_snapshot(
        &mut source,
        domain,
        vec![StateWrite::put(b"base".to_vec(), b"persisted".to_vec())],
    );
    let snapshots = [snapshot];
    let mut journal = ExecutionJournalV1::new(
        &source,
        context(),
        &snapshots,
        JournalLimitsV1::default(),
    )
    .unwrap();

    assert_eq!(journal.read(domain, b"base").unwrap(), Some(b"persisted".to_vec()));
    journal.put(domain, b"root", b"r").unwrap();
    journal.begin_frame().unwrap();
    assert_eq!(journal.read(domain, b"root").unwrap(), Some(b"r".to_vec()));
    journal.put(domain, b"only-child", b"x").unwrap();
    journal.revert_frame().unwrap();

    journal.begin_frame().unwrap();
    assert_eq!(journal.read(domain, b"only-child").unwrap(), None);
    journal.revert_frame().unwrap();
}

#[test]
fn deletion_and_present_empty_are_distinct_and_last_write_wins() {
    let source = MemorySource::default();
    let domain = CommitmentDomainId::Wasm;
    let snapshots = [empty_snapshot(domain)];
    let mut journal = ExecutionJournalV1::new(
        &source,
        context(),
        &snapshots,
        JournalLimitsV1::default(),
    )
    .unwrap();

    journal.put(domain, b"key", b"first").unwrap();
    journal.delete(domain, b"key").unwrap();
    assert_eq!(journal.read(domain, b"key").unwrap(), None);
    journal.put(domain, b"key", b"").unwrap();
    assert_eq!(journal.read(domain, b"key").unwrap(), Some(Vec::new()));
    journal.put(domain, b"key", b"final").unwrap();
    assert_eq!(journal.read(domain, b"key").unwrap(), Some(b"final".to_vec()));
}

#[test]
fn equivalent_legal_traces_produce_the_same_root() {
    let source = MemorySource::default();
    let domain = CommitmentDomainId::Wasm;
    let snapshots = [empty_snapshot(domain)];

    let mut direct = ExecutionJournalV1::new(
        &source,
        context(),
        &snapshots,
        JournalLimitsV1::default(),
    )
    .unwrap();
    direct.put(domain, b"key", b"value").unwrap();
    let direct = direct.finalize(JournalIntentV1::Committed).unwrap();

    let mut nested = ExecutionJournalV1::new(
        &source,
        context(),
        &snapshots,
        JournalLimitsV1::default(),
    )
    .unwrap();
    nested.begin_frame().unwrap();
    nested.put(domain, b"key", b"temporary").unwrap();
    nested.put(domain, b"key", b"value").unwrap();
    nested.commit_frame().unwrap();
    let nested = nested.finalize(JournalIntentV1::Committed).unwrap();

    assert_eq!(direct.roots, nested.roots);
    assert_eq!(direct.roots.len(), 1);
    assert_ne!(direct.roots[0].old_root, direct.roots[0].new_root);
}

#[test]
fn constructor_rejects_invalid_snapshot_sets_before_use() {
    let source = MemorySource::default();
    let limits = JournalLimitsV1::default();

    assert!(matches!(
        ExecutionJournalV1::new(&source, context(), &[], limits),
        Err(JournalError::EmptySnapshots)
    ));

    let wasm = empty_snapshot(CommitmentDomainId::Wasm);
    assert!(matches!(
        ExecutionJournalV1::new(&source, context(), &[wasm, wasm], limits),
        Err(JournalError::DuplicateSnapshotDomain(CommitmentDomainId::Wasm))
    ));

    for domain in [CommitmentDomainId::NativeUtxo, CommitmentDomainId::Evm] {
        assert!(matches!(
            ExecutionJournalV1::new(&source, context(), &[empty_snapshot(domain)], limits),
            Err(JournalError::UnsupportedDomain(found)) if found == domain
        ));
    }

    let seven = [
        empty_snapshot(CommitmentDomainId::Wasm),
        empty_snapshot(CommitmentDomainId::ExecutionAccounting),
        empty_snapshot(CommitmentDomainId::ExecutionReceipts),
        empty_snapshot(CommitmentDomainId::AsyncOutbox),
        empty_snapshot(CommitmentDomainId::AsyncConsumed),
        empty_snapshot(CommitmentDomainId::FeeState),
        empty_snapshot(CommitmentDomainId::Wasm),
    ];
    assert!(matches!(
        ExecutionJournalV1::new(&source, context(), &seven, limits),
        Err(JournalError::TooManySnapshots(7))
    ));
}

#[test]
fn root_lifecycle_and_unconfigured_domains_fail_closed() {
    let source = MemorySource::default();
    let snapshots = [empty_snapshot(CommitmentDomainId::Wasm)];
    let mut journal = ExecutionJournalV1::new(
        &source,
        context(),
        &snapshots,
        JournalLimitsV1::default(),
    )
    .unwrap();

    assert!(matches!(journal.commit_frame(), Err(JournalError::RootFrameLifecycle)));
    assert!(matches!(journal.revert_frame(), Err(JournalError::RootFrameLifecycle)));
    assert!(matches!(
        journal.read(CommitmentDomainId::AsyncOutbox, b"x"),
        Err(JournalError::UnconfiguredDomain(CommitmentDomainId::AsyncOutbox))
    ));

    journal.begin_frame().unwrap();
    assert!(matches!(
        journal.finalize(JournalIntentV1::Committed),
        Err(JournalError::OpenChildFrames)
    ));
}

#[test]
fn structural_limits_are_exact_and_frame_count_is_not_refunded() {
    assert!(matches!(
        JournalLimitsV1::new(0, 1, 1, 1),
        Err(JournalError::InvalidLimits)
    ));
    assert!(JournalLimitsV1::new(64, 4096, 65_536, 16_777_216).is_ok());
    assert!(matches!(
        JournalLimitsV1::new(65, 4096, 65_536, 16_777_216),
        Err(JournalError::InvalidLimits)
    ));

    let source = MemorySource::default();
    let snapshots = [empty_snapshot(CommitmentDomainId::Wasm)];
    let limits = JournalLimitsV1::new(2, 3, 16, 128).unwrap();
    let mut journal = ExecutionJournalV1::new(&source, context(), &snapshots, limits).unwrap();

    journal.begin_frame().unwrap();
    assert!(matches!(journal.begin_frame(), Err(JournalError::DepthLimitExceeded)));
    journal.revert_frame().unwrap();
    journal.begin_frame().unwrap();
    journal.revert_frame().unwrap();
    assert!(matches!(journal.begin_frame(), Err(JournalError::FrameLimitExceeded)));
}

#[test]
fn retained_entry_and_byte_limits_count_each_live_frame_copy() {
    let source = MemorySource::default();
    let domain = CommitmentDomainId::Wasm;
    let snapshots = [empty_snapshot(domain)];
    let limits = JournalLimitsV1::new(4, 16, 2, 4).unwrap();
    let mut journal = ExecutionJournalV1::new(&source, context(), &snapshots, limits).unwrap();

    journal.put(domain, b"k", b"v").unwrap();
    journal.begin_frame().unwrap();
    journal.put(domain, b"k", b"v").unwrap();
    assert!(matches!(
        journal.put(domain, b"b", b""),
        Err(JournalError::EntryLimitExceeded)
    ));
    assert_eq!(journal.read(domain, b"b").unwrap(), None);

    assert!(matches!(
        journal.put(domain, b"k", b"xx"),
        Err(JournalError::ByteLimitExceeded)
    ));
    assert_eq!(journal.read(domain, b"k").unwrap(), Some(b"v".to_vec()));
}

#[test]
fn key_and_value_bounds_reject_one_over_without_changing_state() {
    let source = MemorySource::default();
    let domain = CommitmentDomainId::Wasm;
    let snapshots = [empty_snapshot(domain)];
    let mut journal = ExecutionJournalV1::new(
        &source,
        context(),
        &snapshots,
        JournalLimitsV1::default(),
    )
    .unwrap();

    let max_key = vec![b'k'; MAX_STATE_KEY_BYTES];
    journal.put(domain, &max_key, b"").unwrap();
    let too_large_key = vec![b'k'; MAX_STATE_KEY_BYTES + 1];
    assert!(matches!(
        journal.put(domain, &too_large_key, b"x"),
        Err(JournalError::State(StateError::KeyTooLarge(n))) if n == MAX_STATE_KEY_BYTES + 1
    ));

    let max_value = vec![0x55; MAX_STATE_VALUE_BYTES];
    journal.put(domain, b"value", &max_value).unwrap();
    let too_large_value = vec![0x66; MAX_STATE_VALUE_BYTES + 1];
    assert!(matches!(
        journal.put(domain, b"oversize", &too_large_value),
        Err(JournalError::State(StateError::ValueTooLarge(n))) if n == MAX_STATE_VALUE_BYTES + 1
    ));
    assert_eq!(journal.read(domain, b"oversize").unwrap(), None);
}

fn assert_corrupt_old_value_rejected(final_write: StateWrite) {
    let domain = CommitmentDomainId::Wasm;
    let mut source = MemorySource::default();
    let snapshot = seed_snapshot(
        &mut source,
        domain,
        vec![StateWrite::put(b"key".to_vec(), b"old".to_vec())],
    );
    let old_hash = value_hash(domain, b"old").unwrap();
    source.values.remove(&old_hash);
    let snapshots = [snapshot];
    let mut journal = ExecutionJournalV1::new(
        &source,
        context(),
        &snapshots,
        JournalLimitsV1::default(),
    )
    .unwrap();
    match final_write.value() {
        Some(value) => journal.put(domain, final_write.key(), value).unwrap(),
        None => journal.delete(domain, final_write.key()).unwrap(),
    }

    assert!(matches!(
        journal.finalize(JournalIntentV1::Committed),
        Err(JournalError::State(StateError::MissingValue(found))) if found == old_hash
    ));
}

#[test]
fn finalization_preserves_stage2_checked_old_value_rules() {
    assert_corrupt_old_value_rejected(StateWrite::put(b"key".to_vec(), b"old".to_vec()));
    assert_corrupt_old_value_rejected(StateWrite::put(b"key".to_vec(), b"new".to_vec()));
    assert_corrupt_old_value_rejected(StateWrite::delete(b"key".to_vec()));
}

#[test]
fn later_domain_failure_returns_no_partial_bundle_and_mutates_no_source() {
    let mut source = MemorySource::default();
    let wasm = seed_snapshot(
        &mut source,
        CommitmentDomainId::Wasm,
        vec![StateWrite::put(b"w".to_vec(), b"old-w".to_vec())],
    );
    let fee = seed_snapshot(
        &mut source,
        CommitmentDomainId::FeeState,
        vec![StateWrite::put(b"f".to_vec(), b"old-f".to_vec())],
    );
    let corrupt_hash = value_hash(CommitmentDomainId::FeeState, b"old-f").unwrap();
    source.values.insert(corrupt_hash, b"tampered".to_vec());
    let before = source.clone();
    let snapshots = [wasm, fee];
    let mut journal = ExecutionJournalV1::new(
        &source,
        context(),
        &snapshots,
        JournalLimitsV1::default(),
    )
    .unwrap();
    journal.put(CommitmentDomainId::Wasm, b"w", b"new-w").unwrap();
    journal.put(CommitmentDomainId::FeeState, b"f", b"new-f").unwrap();

    assert!(matches!(
        journal.finalize(JournalIntentV1::Committed),
        Err(JournalError::State(StateError::ValueHashMismatch(found))) if found == corrupt_hash
    ));
    assert_eq!(source, before);
}

#[test]
fn committed_results_are_unpublished_and_reverted_results_are_empty() {
    let mut source = MemorySource::default();
    let domain = CommitmentDomainId::Wasm;
    let snapshot = seed_snapshot(
        &mut source,
        domain,
        vec![StateWrite::put(b"key".to_vec(), b"old".to_vec())],
    );
    let snapshots = [snapshot];

    let mut committed = ExecutionJournalV1::new(
        &source,
        context(),
        &snapshots,
        JournalLimitsV1::default(),
    )
    .unwrap();
    committed.put(domain, b"key", b"new").unwrap();
    let result = committed.finalize(JournalIntentV1::Committed).unwrap();
    assert_eq!(result.context, context());
    assert_eq!(result.roots.len(), 1);
    assert_eq!(result.transitions.len(), 1);
    assert_eq!(read_value(&source, snapshot, b"key").unwrap(), Some(b"old".to_vec()));

    let mut published = source.clone();
    for transition in &result.transitions {
        published.absorb(transition);
    }
    let new_snapshot = DomainSnapshot {
        domain,
        root: result.roots[0].new_root,
    };
    assert_eq!(read_value(&published, new_snapshot, b"key").unwrap(), Some(b"new".to_vec()));

    let mut reverted = ExecutionJournalV1::new(
        &source,
        context(),
        &snapshots,
        JournalLimitsV1::default(),
    )
    .unwrap();
    reverted.put(domain, b"key", b"discarded").unwrap();
    let reverted = reverted.finalize(JournalIntentV1::Reverted).unwrap();
    assert!(reverted.transitions.is_empty());
    assert_eq!(reverted.roots[0].old_root, snapshot.root);
    assert_eq!(reverted.roots[0].new_root, snapshot.root);
}

proptest! {
    #[test]
    fn nested_traces_match_an_independent_clone_map_reference(
        operations in prop::collection::vec((0u8..5, 0u8..4, any::<u8>()), 0..40)
    ) {
        let source = MemorySource::default();
        let domain = CommitmentDomainId::Wasm;
        let snapshots = [empty_snapshot(domain)];
        let limits = JournalLimitsV1::new(4, 64, 64, 1024).unwrap();
        let mut journal = ExecutionJournalV1::new(&source, context(), &snapshots, limits).unwrap();
        let mut model: Vec<BTreeMap<Vec<u8>, Option<Vec<u8>>>> = vec![BTreeMap::new()];

        for (kind, key_byte, value_byte) in operations {
            let key = vec![key_byte];
            match kind {
                0 => {
                    let value = vec![value_byte];
                    journal.put(domain, &key, &value).unwrap();
                    model.last_mut().unwrap().insert(key, Some(value));
                }
                1 => {
                    journal.delete(domain, &key).unwrap();
                    model.last_mut().unwrap().insert(key, None);
                }
                2 if model.len() < 4 => {
                    journal.begin_frame().unwrap();
                    model.push(BTreeMap::new());
                }
                3 if model.len() > 1 => {
                    journal.commit_frame().unwrap();
                    let child = model.pop().unwrap();
                    model.last_mut().unwrap().extend(child);
                }
                4 if model.len() > 1 => {
                    journal.revert_frame().unwrap();
                    model.pop();
                }
                _ => {}
            }

            for probe in 0u8..4 {
                let probe_key = vec![probe];
                let expected = model
                    .iter()
                    .rev()
                    .find_map(|frame| frame.get(&probe_key).cloned())
                    .flatten();
                prop_assert_eq!(journal.read(domain, &probe_key).unwrap(), expected);
            }
        }
    }
}
