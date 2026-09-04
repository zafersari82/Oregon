mod common;

use common::{AcceptTestSpends, RejectTestSpends, base, entry, outpoint, spend, state_with};
use oregon_mempool::{Mempool, MempoolConfig, MempoolError};
use oregon_primitives::{Block, BlockHeader, Hash256, OutPoint, Transaction};

type ObservableEntry = (Hash256, u64, usize, Vec<Hash256>);
type Observable = (Vec<Hash256>, usize, Vec<ObservableEntry>);

fn tx_outpoint(transaction: &Transaction, index: u32) -> OutPoint {
    OutPoint {
        txid: transaction.txid(),
        index,
    }
}

fn active_block(transactions: Vec<Transaction>, tag: u8) -> Block {
    Block {
        header: BlockHeader {
            version: 1,
            previous_block: Hash256::from_bytes([tag; 32]),
            transaction_root: Hash256::from_bytes([tag.wrapping_add(1); 32]),
            timestamp: u64::from(tag),
            difficulty_commitment: [tag.wrapping_add(2); 32],
            nonce: u64::from(tag),
        },
        transactions,
    }
}

fn observable(pool: &Mempool) -> Observable {
    let order = pool.deterministic_order().unwrap();
    let entries = order
        .iter()
        .map(|txid| {
            let entry = pool.entry(txid).unwrap();
            (
                *txid,
                entry.fee(),
                entry.encoded_bytes(),
                entry.parents().iter().copied().collect(),
            )
        })
        .collect();
    (order, pool.total_bytes(), entries)
}

#[test]
fn confirmed_parent_is_removed_while_child_is_promoted_to_chain_backed() {
    let root = outpoint(0x11, 0);
    let old_chain = state_with(vec![(root, entry(100, 1, false))]);
    let old_base = base(0x12, 20);
    let mut pool = Mempool::new(old_base, MempoolConfig::default()).unwrap();
    let parent = spend(vec![root], &[90], 1);
    let child = spend(vec![tx_outpoint(&parent, 0)], &[80], 2);

    pool.admit(parent.clone(), old_base, &old_chain, &AcceptTestSpends)
        .unwrap();
    pool.admit(child.clone(), old_base, &old_chain, &AcceptTestSpends)
        .unwrap();

    let parent_output = tx_outpoint(&parent, 0);
    let new_chain = state_with(vec![(parent_output, entry(90, 21, false))]);
    let new_base = base(0x13, 21);
    let block = active_block(vec![parent.clone()], 0x14);

    let report = pool
        .reconcile_active_block(&block, new_base, &new_chain, &AcceptTestSpends)
        .expect("confirmed parent promotion succeeds");

    assert_eq!(report.removed, vec![parent.txid()]);
    assert_eq!(report.retained, 1);
    assert_eq!(pool.base(), new_base);
    assert!(!pool.contains(&parent.txid()));
    assert!(pool.contains(&child.txid()));
    assert!(pool.entry(&child.txid()).unwrap().parents().is_empty());
}

#[test]
fn active_chain_conflict_removes_conflicting_root_and_its_descendants() {
    let shared = outpoint(0x21, 0);
    let old_chain = state_with(vec![(shared, entry(100, 1, false))]);
    let old_base = base(0x22, 20);
    let mut pool = Mempool::new(old_base, MempoolConfig::default()).unwrap();
    let a = spend(vec![shared], &[90], 1);
    let child = spend(vec![tx_outpoint(&a, 0)], &[80], 2);
    pool.admit(a.clone(), old_base, &old_chain, &AcceptTestSpends)
        .unwrap();
    pool.admit(child.clone(), old_base, &old_chain, &AcceptTestSpends)
        .unwrap();

    let b = spend(vec![shared], &[70], 3);
    let block = active_block(vec![b], 0x23);
    let new_chain = state_with(vec![]);
    let new_base = base(0x24, 21);

    let report = pool
        .reconcile_active_block(&block, new_base, &new_chain, &AcceptTestSpends)
        .expect("active conflict reconciliation succeeds");

    let mut expected = vec![a.txid(), child.txid()];
    expected.sort();
    assert_eq!(report.removed, expected);
    assert_eq!(report.retained, 0);
    assert_eq!(pool.base(), new_base);
    assert!(pool.is_empty());
}

#[test]
fn ordinary_tip_update_retains_valid_entries_and_filters_missing_inputs() {
    let kept_root = outpoint(0x31, 0);
    let lost_root = outpoint(0x32, 0);
    let old_chain = state_with(vec![
        (kept_root, entry(100, 1, false)),
        (lost_root, entry(100, 1, false)),
    ]);
    let old_base = base(0x33, 20);
    let mut pool = Mempool::new(old_base, MempoolConfig::default()).unwrap();
    let kept = spend(vec![kept_root], &[90], 1);
    let lost = spend(vec![lost_root], &[80], 2);
    pool.admit(kept.clone(), old_base, &old_chain, &AcceptTestSpends)
        .unwrap();
    pool.admit(lost.clone(), old_base, &old_chain, &AcceptTestSpends)
        .unwrap();

    let new_chain = state_with(vec![(kept_root, entry(100, 1, false))]);
    let new_base = base(0x34, 21);
    let unrelated = spend(vec![outpoint(0x99, 0)], &[1], 9);
    let block = active_block(vec![unrelated], 0x35);

    let report = pool
        .reconcile_active_block(&block, new_base, &new_chain, &AcceptTestSpends)
        .expect("ordinary tip reconciliation succeeds");

    assert_eq!(report.removed, vec![lost.txid()]);
    assert_eq!(report.retained, 1);
    assert_eq!(pool.base(), new_base);
    assert!(pool.contains(&kept.txid()));
    assert!(!pool.contains(&lost.txid()));
}

#[test]
fn reorg_retains_transactions_valid_against_new_chain_snapshot() {
    let root = outpoint(0x41, 0);
    let chain = state_with(vec![(root, entry(100, 1, false))]);
    let old_base = base(0x42, 20);
    let mut pool = Mempool::new(old_base, MempoolConfig::default()).unwrap();
    let tx = spend(vec![root], &[90], 1);
    pool.admit(tx.clone(), old_base, &chain, &AcceptTestSpends)
        .unwrap();

    let new_base = base(0x43, 19);
    let report = pool
        .reconcile_reorg(new_base, &chain, &AcceptTestSpends)
        .expect("compatible reorg retains tx");

    assert!(report.removed.is_empty());
    assert_eq!(report.retained, 1);
    assert_eq!(pool.base(), new_base);
    assert!(pool.contains(&tx.txid()));
}

#[test]
fn reorg_removes_promoted_child_when_confirmed_parent_output_disappears() {
    let root = outpoint(0x51, 0);
    let initial_chain = state_with(vec![(root, entry(100, 1, false))]);
    let initial_base = base(0x52, 20);
    let mut pool = Mempool::new(initial_base, MempoolConfig::default()).unwrap();
    let parent = spend(vec![root], &[90], 1);
    let child = spend(vec![tx_outpoint(&parent, 0)], &[80], 2);
    pool.admit(
        parent.clone(),
        initial_base,
        &initial_chain,
        &AcceptTestSpends,
    )
    .unwrap();
    pool.admit(
        child.clone(),
        initial_base,
        &initial_chain,
        &AcceptTestSpends,
    )
    .unwrap();

    let promoted_chain = state_with(vec![(tx_outpoint(&parent, 0), entry(90, 21, false))]);
    let promoted_base = base(0x53, 21);
    pool.reconcile_active_block(
        &active_block(vec![parent], 0x54),
        promoted_base,
        &promoted_chain,
        &AcceptTestSpends,
    )
    .unwrap();
    assert!(pool.contains(&child.txid()));
    assert!(pool.entry(&child.txid()).unwrap().parents().is_empty());

    let reorg_chain = state_with(vec![(root, entry(100, 1, false))]);
    let reorg_base = base(0x55, 20);
    let report = pool
        .reconcile_reorg(reorg_base, &reorg_chain, &AcceptTestSpends)
        .expect("disappeared parent output filters child");

    assert_eq!(report.removed, vec![child.txid()]);
    assert_eq!(report.retained, 0);
    assert!(pool.is_empty());
}

#[test]
fn reorg_never_resurrects_transactions_absent_from_current_pool() {
    let root = outpoint(0x61, 0);
    let other_root = outpoint(0x62, 0);
    let chain = state_with(vec![
        (root, entry(100, 1, false)),
        (other_root, entry(100, 1, false)),
    ]);
    let old_base = base(0x63, 20);
    let mut pool = Mempool::new(old_base, MempoolConfig::default()).unwrap();
    let retained = spend(vec![root], &[90], 1);
    let disconnected_but_absent = spend(vec![other_root], &[80], 2);
    pool.admit(retained.clone(), old_base, &chain, &AcceptTestSpends)
        .unwrap();
    assert!(!pool.contains(&disconnected_but_absent.txid()));

    pool.reconcile_reorg(base(0x64, 19), &chain, &AcceptTestSpends)
        .unwrap();

    assert!(pool.contains(&retained.txid()));
    assert!(!pool.contains(&disconnected_but_absent.txid()));
}

#[test]
fn stale_admission_is_rejected_until_reorg_reconciliation_publishes_new_base() {
    let root = outpoint(0x71, 0);
    let fresh_root = outpoint(0x72, 0);
    let chain = state_with(vec![
        (root, entry(100, 1, false)),
        (fresh_root, entry(100, 1, false)),
    ]);
    let old_base = base(0x73, 20);
    let new_base = base(0x74, 19);
    let mut pool = Mempool::new(old_base, MempoolConfig::default()).unwrap();
    let existing = spend(vec![root], &[90], 1);
    let fresh = spend(vec![fresh_root], &[80], 2);
    pool.admit(existing, old_base, &chain, &AcceptTestSpends)
        .unwrap();

    assert_eq!(
        pool.admit(fresh.clone(), new_base, &chain, &AcceptTestSpends),
        Err(MempoolError::StaleChainContext)
    );
    pool.reconcile_reorg(new_base, &chain, &AcceptTestSpends)
        .unwrap();
    pool.admit(fresh.clone(), new_base, &chain, &AcceptTestSpends)
        .expect("exact reconciled base admits");
    assert!(pool.contains(&fresh.txid()));
}

#[test]
fn reorg_rebuild_is_insertion_order_independent() {
    let a_root = outpoint(0x81, 0);
    let b_root = outpoint(0x82, 0);
    let chain = state_with(vec![
        (a_root, entry(100, 1, false)),
        (b_root, entry(100, 1, false)),
    ]);
    let old_base = base(0x83, 20);
    let new_base = base(0x84, 19);
    let a = spend(vec![a_root], &[90], 1);
    let b = spend(vec![b_root], &[80], 2);

    let mut first = Mempool::new(old_base, MempoolConfig::default()).unwrap();
    first
        .admit(a.clone(), old_base, &chain, &AcceptTestSpends)
        .unwrap();
    first
        .admit(b.clone(), old_base, &chain, &AcceptTestSpends)
        .unwrap();

    let mut second = Mempool::new(old_base, MempoolConfig::default()).unwrap();
    second
        .admit(b, old_base, &chain, &AcceptTestSpends)
        .unwrap();
    second
        .admit(a, old_base, &chain, &AcceptTestSpends)
        .unwrap();

    let first_report = first
        .reconcile_reorg(new_base, &chain, &AcceptTestSpends)
        .unwrap();
    let second_report = second
        .reconcile_reorg(new_base, &chain, &AcceptTestSpends)
        .unwrap();
    assert_eq!(first_report, second_report);
    assert_eq!(observable(&first), observable(&second));
}

#[test]
fn reorg_height_overflow_is_atomic_and_rejecting_verifier_filters_tx() {
    let root = outpoint(0x91, 0);
    let chain = state_with(vec![(root, entry(100, 1, false))]);
    let old_base = base(0x92, 20);
    let mut pool = Mempool::new(old_base, MempoolConfig::default()).unwrap();
    let tx = spend(vec![root], &[90], 1);
    pool.admit(tx.clone(), old_base, &chain, &AcceptTestSpends)
        .unwrap();
    let before = observable(&pool);

    assert_eq!(
        pool.reconcile_reorg(base(0x93, u64::MAX), &chain, &AcceptTestSpends),
        Err(MempoolError::HeightOverflow)
    );
    assert_eq!(pool.base(), old_base);
    assert_eq!(observable(&pool), before);

    let report = pool
        .reconcile_reorg(base(0x94, 19), &chain, &RejectTestSpends)
        .expect("verifier rejection filters tx during rebuild");
    assert_eq!(report.removed, vec![tx.txid()]);
    assert!(pool.is_empty());
}
