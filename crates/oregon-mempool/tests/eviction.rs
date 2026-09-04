mod common;

use common::{AcceptTestSpends, base, entry, outpoint, spend, state_with};
use oregon_mempool::{Mempool, MempoolConfig, MempoolError};
use oregon_primitives::{Hash256, OutPoint, Transaction};

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntrySnapshot {
    txid: Hash256,
    fee: u64,
    encoded_bytes: usize,
    parents: Vec<Hash256>,
    children: Vec<Hash256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PoolSnapshot {
    len: usize,
    total_bytes: usize,
    order: Vec<Hash256>,
    entries: Vec<EntrySnapshot>,
}

fn snapshot(pool: &Mempool) -> PoolSnapshot {
    let order = pool.deterministic_order().expect("valid pool topology");
    let entries = order
        .iter()
        .map(|txid| {
            let entry = pool.entry(txid).expect("ordered txid exists");
            EntrySnapshot {
                txid: *txid,
                fee: entry.fee(),
                encoded_bytes: entry.encoded_bytes(),
                parents: entry.parents().iter().copied().collect(),
                children: entry.children().iter().copied().collect(),
            }
        })
        .collect();
    PoolSnapshot {
        len: pool.len(),
        total_bytes: pool.total_bytes(),
        order,
        entries,
    }
}

fn tx_outpoint(transaction: &Transaction, index: u32) -> OutPoint {
    OutPoint {
        txid: transaction.txid(),
        index,
    }
}

#[test]
fn entry_limit_accepts_equality_and_evicts_only_when_exceeded() {
    let a_root = outpoint(0x11, 0);
    let b_root = outpoint(0x12, 0);
    let c_root = outpoint(0x13, 0);
    let chain = state_with(vec![
        (a_root, entry(100, 1, false)),
        (b_root, entry(100, 1, false)),
        (c_root, entry(100, 1, false)),
    ]);
    let chain_base = base(0x14, 20);
    let config = MempoolConfig {
        max_entries: 2,
        ..MempoolConfig::default()
    };
    let mut pool = Mempool::new(chain_base, config).unwrap();
    let a = spend(vec![a_root], &[90], 1);
    let b = spend(vec![b_root], &[80], 2);
    let c = spend(vec![c_root], &[10], 3);

    assert!(
        pool.admit(a.clone(), chain_base, &chain, &AcceptTestSpends)
            .unwrap()
            .evicted
            .is_empty()
    );
    assert!(
        pool.admit(b.clone(), chain_base, &chain, &AcceptTestSpends)
            .unwrap()
            .evicted
            .is_empty()
    );
    assert_eq!(pool.len(), 2);

    let outcome = pool
        .admit(c.clone(), chain_base, &chain, &AcceptTestSpends)
        .expect("better candidate evicts lower-priority entry");
    assert_eq!(outcome.evicted, vec![a.txid()]);
    assert_eq!(pool.len(), 2);
    assert!(!pool.contains(&a.txid()));
    assert!(pool.contains(&b.txid()));
    assert!(pool.contains(&c.txid()));
}

#[test]
fn byte_limit_accepts_equality_and_evicts_only_when_exceeded() {
    let a_root = outpoint(0x21, 0);
    let b_root = outpoint(0x22, 0);
    let c_root = outpoint(0x23, 0);
    let chain = state_with(vec![
        (a_root, entry(100, 1, false)),
        (b_root, entry(100, 1, false)),
        (c_root, entry(100, 1, false)),
    ]);
    let chain_base = base(0x24, 20);
    let a = spend(vec![a_root], &[90], 1);
    let b = spend(vec![b_root], &[80], 2);
    let c = spend(vec![c_root], &[10], 3);
    let exact_bytes = a.encode().len() + b.encode().len();
    let config = MempoolConfig {
        max_entries: 10,
        max_total_bytes: exact_bytes,
        ..MempoolConfig::default()
    };
    let mut pool = Mempool::new(chain_base, config.clone()).unwrap();

    pool.admit(a.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();
    let second = pool
        .admit(b.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();
    assert!(second.evicted.is_empty());
    assert_eq!(pool.total_bytes(), config.max_total_bytes);

    let before_overflow_candidate = snapshot(&pool);
    match pool.admit(c.clone(), chain_base, &chain, &AcceptTestSpends) {
        Ok(outcome) => {
            assert_eq!(outcome.evicted, vec![a.txid()]);
            assert!(pool.contains(&b.txid()));
            assert!(pool.contains(&c.txid()));
        }
        Err(MempoolError::CapacityRejected) => {
            assert_eq!(snapshot(&pool), before_overflow_candidate);
        }
        Err(error) => panic!("unexpected capacity result: {error:?}"),
    }
    assert!(pool.total_bytes() <= config.max_total_bytes);
}

#[test]
fn evicting_low_priority_parent_removes_entire_descendant_subtree() {
    let parent_root = outpoint(0x31, 0);
    let candidate_root = outpoint(0x32, 0);
    let chain = state_with(vec![
        (parent_root, entry(100, 1, false)),
        (candidate_root, entry(100, 1, false)),
    ]);
    let chain_base = base(0x33, 20);
    let config = MempoolConfig {
        max_entries: 2,
        ..MempoolConfig::default()
    };
    let mut pool = Mempool::new(chain_base, config).unwrap();
    let parent = spend(vec![parent_root], &[99], 1);
    let child = spend(vec![tx_outpoint(&parent, 0)], &[9], 2);
    let candidate = spend(vec![candidate_root], &[5], 3);

    pool.admit(parent.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();
    pool.admit(child.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();

    let outcome = pool
        .admit(candidate.clone(), chain_base, &chain, &AcceptTestSpends)
        .expect("parent subtree is evicted atomically");
    let mut expected = vec![parent.txid(), child.txid()];
    expected.sort();
    assert_eq!(outcome.evicted, expected);
    assert_eq!(pool.len(), 1);
    assert_eq!(pool.deterministic_order().unwrap(), vec![candidate.txid()]);
    assert!(!pool.contains(&parent.txid()));
    assert!(!pool.contains(&child.txid()));
}

#[test]
fn candidate_self_eviction_rejects_without_any_public_mutation() {
    let existing_root = outpoint(0x41, 0);
    let candidate_root = outpoint(0x42, 0);
    let chain = state_with(vec![
        (existing_root, entry(100, 1, false)),
        (candidate_root, entry(100, 1, false)),
    ]);
    let chain_base = base(0x43, 20);
    let config = MempoolConfig {
        max_entries: 1,
        ..MempoolConfig::default()
    };
    let mut pool = Mempool::new(chain_base, config).unwrap();
    let existing = spend(vec![existing_root], &[10], 1);
    let worse_candidate = spend(vec![candidate_root], &[99], 2);

    pool.admit(existing, chain_base, &chain, &AcceptTestSpends)
        .unwrap();
    let before = snapshot(&pool);

    assert_eq!(
        pool.admit(worse_candidate, chain_base, &chain, &AcceptTestSpends,),
        Err(MempoolError::CapacityRejected)
    );
    assert_eq!(snapshot(&pool), before);
}

#[test]
fn eviction_is_independent_of_independent_insertion_order() {
    let a_root = outpoint(0x51, 0);
    let b_root = outpoint(0x52, 0);
    let c_root = outpoint(0x53, 0);
    let chain = state_with(vec![
        (a_root, entry(100, 1, false)),
        (b_root, entry(100, 1, false)),
        (c_root, entry(100, 1, false)),
    ]);
    let chain_base = base(0x54, 20);
    let config = MempoolConfig {
        max_entries: 2,
        ..MempoolConfig::default()
    };
    let a = spend(vec![a_root], &[90], 1);
    let b = spend(vec![b_root], &[80], 2);
    let c = spend(vec![c_root], &[10], 3);

    let mut first = Mempool::new(chain_base, config.clone()).unwrap();
    first
        .admit(a.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();
    first
        .admit(b.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();
    let first_outcome = first
        .admit(c.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();

    let mut second = Mempool::new(chain_base, config).unwrap();
    second
        .admit(b.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();
    second
        .admit(a.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();
    let second_outcome = second
        .admit(c, chain_base, &chain, &AcceptTestSpends)
        .unwrap();

    assert_eq!(first_outcome.evicted, second_outcome.evicted);
    assert_eq!(first_outcome.evicted, vec![a.txid()]);
    assert_eq!(
        first.deterministic_order().unwrap(),
        second.deterministic_order().unwrap()
    );
    assert_eq!(first.total_bytes(), second.total_bytes());
}
