mod common;

use common::{AcceptTestSpends, base, entry, outpoint, spend, state_with};
use oregon_mempool::{Mempool, MempoolConfig, MempoolError};
use oregon_primitives::{Hash256, OutPoint, Transaction};

fn tx_outpoint(transaction: &Transaction, index: u32) -> OutPoint {
    OutPoint {
        txid: transaction.txid(),
        index,
    }
}

#[test]
fn parent_then_child_succeeds_but_child_before_parent_is_missing_dependency() {
    let root = outpoint(0x11, 0);
    let chain = state_with(vec![(root, entry(100, 1, false))]);
    let chain_base = base(0x12, 20);
    let parent = spend(vec![root], &[90], 1);
    let child_input = tx_outpoint(&parent, 0);
    let child = spend(vec![child_input], &[80], 2);

    let mut child_first = Mempool::new(chain_base, MempoolConfig::default()).unwrap();
    assert_eq!(
        child_first.admit(child.clone(), chain_base, &chain, &AcceptTestSpends),
        Err(MempoolError::MissingDependency(child_input))
    );
    assert!(child_first.is_empty());

    let mut pool = Mempool::new(chain_base, MempoolConfig::default()).unwrap();
    pool.admit(parent.clone(), chain_base, &chain, &AcceptTestSpends)
        .expect("parent admission");
    pool.admit(child.clone(), chain_base, &chain, &AcceptTestSpends)
        .expect("child admission");

    assert_eq!(
        pool.deterministic_order().unwrap(),
        vec![parent.txid(), child.txid()]
    );
    assert_eq!(
        pool.entry(&child.txid())
            .expect("child entry")
            .parents()
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![parent.txid()]
    );
    assert!(
        pool.entry(&parent.txid())
            .expect("parent entry")
            .children()
            .contains(&child.txid())
    );
}

#[test]
fn existing_parent_with_invalid_output_index_is_rejected_atomically() {
    let root = outpoint(0x21, 0);
    let chain = state_with(vec![(root, entry(100, 1, false))]);
    let chain_base = base(0x22, 20);
    let parent = spend(vec![root], &[90], 1);
    let invalid_parent_output = tx_outpoint(&parent, 1);
    let child = spend(vec![invalid_parent_output], &[1], 2);
    let mut pool = Mempool::new(chain_base, MempoolConfig::default()).unwrap();

    pool.admit(parent.clone(), chain_base, &chain, &AcceptTestSpends)
        .expect("parent admission");
    let before_order = pool.deterministic_order().unwrap();
    let before_bytes = pool.total_bytes();

    assert_eq!(
        pool.admit(child, chain_base, &chain, &AcceptTestSpends),
        Err(MempoolError::InvalidParentOutput(invalid_parent_output))
    );
    assert_eq!(pool.deterministic_order().unwrap(), before_order);
    assert_eq!(pool.total_bytes(), before_bytes);
    assert_eq!(pool.len(), 1);
}

#[test]
fn ancestor_limit_allows_exact_limit_and_rejects_one_more() {
    let root = outpoint(0x31, 0);
    let chain = state_with(vec![(root, entry(100, 1, false))]);
    let chain_base = base(0x32, 20);
    let config = MempoolConfig {
        max_ancestors: 2,
        max_descendants: 25,
        ..MempoolConfig::default()
    };
    let mut pool = Mempool::new(chain_base, config).unwrap();

    let a = spend(vec![root], &[90], 1);
    let b = spend(vec![tx_outpoint(&a, 0)], &[80], 2);
    let c = spend(vec![tx_outpoint(&b, 0)], &[70], 3);
    let d = spend(vec![tx_outpoint(&c, 0)], &[60], 4);

    pool.admit(a.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();
    pool.admit(b.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();
    pool.admit(c.clone(), chain_base, &chain, &AcceptTestSpends)
        .expect("exactly two ancestors are allowed");
    let before_order = pool.deterministic_order().unwrap();
    let before_bytes = pool.total_bytes();

    assert_eq!(
        pool.admit(d, chain_base, &chain, &AcceptTestSpends),
        Err(MempoolError::TooManyAncestors)
    );
    assert_eq!(pool.deterministic_order().unwrap(), before_order);
    assert_eq!(pool.total_bytes(), before_bytes);
}

#[test]
fn descendant_limit_allows_exact_limit_and_rejects_one_more() {
    let root = outpoint(0x41, 0);
    let chain = state_with(vec![(root, entry(100, 1, false))]);
    let chain_base = base(0x42, 20);
    let config = MempoolConfig {
        max_ancestors: 25,
        max_descendants: 2,
        ..MempoolConfig::default()
    };
    let mut pool = Mempool::new(chain_base, config).unwrap();

    let a = spend(vec![root], &[90], 1);
    let b = spend(vec![tx_outpoint(&a, 0)], &[80], 2);
    let c = spend(vec![tx_outpoint(&b, 0)], &[70], 3);
    let d = spend(vec![tx_outpoint(&c, 0)], &[60], 4);

    pool.admit(a.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();
    pool.admit(b.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();
    pool.admit(c.clone(), chain_base, &chain, &AcceptTestSpends)
        .expect("root may have exactly two descendants");
    let before_order = pool.deterministic_order().unwrap();
    let before_bytes = pool.total_bytes();

    assert_eq!(
        pool.admit(d, chain_base, &chain, &AcceptTestSpends),
        Err(MempoolError::TooManyDescendants)
    );
    assert_eq!(pool.deterministic_order().unwrap(), before_order);
    assert_eq!(pool.total_bytes(), before_bytes);
}

#[test]
fn topology_is_insertion_order_independent_and_parents_precede_children() {
    let left_root = outpoint(0x51, 0);
    let right_root = outpoint(0x52, 0);
    let chain = state_with(vec![
        (left_root, entry(100, 1, false)),
        (right_root, entry(100, 1, false)),
    ]);
    let chain_base = base(0x53, 20);

    let left = spend(vec![left_root], &[90], 1);
    let right = spend(vec![right_root], &[90], 2);
    let child = spend(vec![tx_outpoint(&left, 0)], &[80], 3);

    let mut first = Mempool::new(chain_base, MempoolConfig::default()).unwrap();
    first
        .admit(left.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();
    first
        .admit(right.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();
    first
        .admit(child.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();

    let mut second = Mempool::new(chain_base, MempoolConfig::default()).unwrap();
    second
        .admit(right.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();
    second
        .admit(left.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();
    second
        .admit(child.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();

    let first_order = first.deterministic_order().unwrap();
    let second_order = second.deterministic_order().unwrap();
    assert_eq!(first_order, second_order);
    let parent_pos = first_order
        .iter()
        .position(|txid| *txid == left.txid())
        .unwrap();
    let child_pos = first_order
        .iter()
        .position(|txid| *txid == child.txid())
        .unwrap();
    assert!(parent_pos < child_pos);

    let mut independent = vec![left.txid(), right.txid()];
    independent.sort();
    let emitted_independent: Vec<Hash256> = first_order
        .into_iter()
        .filter(|txid| *txid == left.txid() || *txid == right.txid())
        .collect();
    assert_eq!(emitted_independent, independent);
}
