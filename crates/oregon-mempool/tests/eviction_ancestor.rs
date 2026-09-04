mod common;

use common::{AcceptTestSpends, base, entry, outpoint, spend, state_with};
use oregon_mempool::{Mempool, MempoolConfig, MempoolError};
use oregon_primitives::OutPoint;

#[test]
fn capacity_never_evicts_an_ancestor_required_by_candidate() {
    let root = outpoint(0x61, 0);
    let chain = state_with(vec![(root, entry(100, 1, false))]);
    let chain_base = base(0x62, 20);
    let config = MempoolConfig {
        max_entries: 1,
        ..MempoolConfig::default()
    };
    let mut pool = Mempool::new(chain_base, config).unwrap();
    let parent = spend(vec![root], &[99], 1);
    let child_input = OutPoint {
        txid: parent.txid(),
        index: 0,
    };
    let child = spend(vec![child_input], &[9], 2);

    pool.admit(parent.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();
    let before_order = pool.deterministic_order().unwrap();
    let before_bytes = pool.total_bytes();

    assert_eq!(
        pool.admit(child, chain_base, &chain, &AcceptTestSpends),
        Err(MempoolError::CapacityRejected)
    );
    assert_eq!(pool.deterministic_order().unwrap(), before_order);
    assert_eq!(pool.total_bytes(), before_bytes);
    assert!(pool.contains(&parent.txid()));
}
