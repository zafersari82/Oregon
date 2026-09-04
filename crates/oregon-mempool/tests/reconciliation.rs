mod common;

use common::{AcceptTestSpends, base, entry, outpoint, spend, state_with};
use oregon_mempool::{Mempool, MempoolConfig};
use oregon_primitives::{Block, BlockHeader, Hash256, OutPoint, Transaction};

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
