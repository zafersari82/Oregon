mod common;

use common::{AcceptTestSpends, RejectTestSpends, base, entry, outpoint, spend, state_with};
use oregon_consensus::NormalTransactionError;
use oregon_mempool::{ChainBase, Mempool, MempoolConfig, MempoolError};
use oregon_primitives::{Amount, Hash256, Transaction, TxOutput};
use oregon_utxo::UtxoError;

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
    base: ChainBase,
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
        base: pool.base(),
        len: pool.len(),
        total_bytes: pool.total_bytes(),
        order,
        entries,
    }
}

#[test]
fn default_limits_are_exact() {
    let config = MempoolConfig::default();
    assert_eq!(config.max_entries, 50_000);
    assert_eq!(config.max_total_bytes, 64 * 1024 * 1024);
    assert_eq!(config.max_ancestors, 25);
    assert_eq!(config.max_descendants, 25);
}

#[test]
fn zero_entry_capacity_is_invalid() {
    let config = MempoolConfig {
        max_entries: 0,
        ..MempoolConfig::default()
    };
    assert!(matches!(
        Mempool::new(base(1, 10), config),
        Err(MempoolError::InvalidConfig)
    ));
}

#[test]
fn zero_byte_capacity_is_invalid() {
    let config = MempoolConfig {
        max_total_bytes: 0,
        ..MempoolConfig::default()
    };
    assert!(matches!(
        Mempool::new(base(2, 10), config),
        Err(MempoolError::InvalidConfig)
    ));
}

#[test]
fn zero_dependency_limits_are_valid_policy() {
    let config = MempoolConfig {
        max_ancestors: 0,
        max_descendants: 0,
        ..MempoolConfig::default()
    };
    let pool = Mempool::new(base(3, 10), config).expect("zero dependency limits are allowed");
    assert!(pool.is_empty());
    assert_eq!(pool.total_bytes(), 0);
}

#[test]
fn valid_chain_backed_transaction_records_fee_and_size() {
    let previous = outpoint(0x11, 0);
    let chain = state_with(vec![(previous, entry(100, 1, false))]);
    let tx = spend(vec![previous], &[60, 30], 1);
    let chain_base = base(0x22, 20);
    let mut pool = Mempool::new(chain_base, MempoolConfig::default()).unwrap();

    let outcome = pool
        .admit(tx.clone(), chain_base, &chain, &AcceptTestSpends)
        .expect("valid chain-backed admission");

    assert_eq!(outcome.txid, tx.txid());
    assert_eq!(outcome.fee, 10);
    assert_eq!(outcome.encoded_bytes, tx.encode().len());
    assert!(outcome.evicted.is_empty());
    assert_eq!(pool.len(), 1);
    assert_eq!(pool.total_bytes(), tx.encode().len());
    assert!(pool.contains(&tx.txid()));
    let stored = pool.entry(&tx.txid()).expect("stored entry");
    assert_eq!(stored.transaction(), &tx);
    assert_eq!(stored.fee(), 10);
    assert_eq!(stored.encoded_bytes(), tx.encode().len());
    assert!(stored.parents().is_empty());
    assert!(stored.children().is_empty());
}

#[test]
fn zero_fee_transaction_is_valid_when_capacity_allows() {
    let previous = outpoint(0x12, 0);
    let chain = state_with(vec![(previous, entry(100, 1, false))]);
    let tx = spend(vec![previous], &[100], 2);
    let chain_base = base(0x13, 20);
    let mut pool = Mempool::new(chain_base, MempoolConfig::default()).unwrap();

    let outcome = pool
        .admit(tx.clone(), chain_base, &chain, &AcceptTestSpends)
        .expect("zero-fee transaction is valid policy");

    assert_eq!(outcome.fee, 0);
    assert_eq!(outcome.encoded_bytes, tx.encode().len());
    assert_eq!(pool.entry(&tx.txid()).unwrap().fee(), 0);
    assert!(pool.contains(&tx.txid()));
}

#[test]
fn changed_witness_changes_mempool_txid_and_canonical_bytes() {
    let previous = outpoint(0x14, 0);
    let chain = state_with(vec![(previous, entry(100, 1, false))]);
    let chain_base = base(0x15, 20);
    let plain = spend(vec![previous], &[90], 3);
    let mut witnessed = plain.clone();
    witnessed.inputs[0].witness = vec![vec![0xaa, 0xbb, 0xcc]];

    assert_ne!(plain.txid(), witnessed.txid());
    assert_ne!(plain.encode().len(), witnessed.encode().len());

    let mut plain_pool = Mempool::new(chain_base, MempoolConfig::default()).unwrap();
    let plain_outcome = plain_pool
        .admit(plain.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();
    let mut witnessed_pool = Mempool::new(chain_base, MempoolConfig::default()).unwrap();
    let witnessed_outcome = witnessed_pool
        .admit(witnessed.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();

    assert_eq!(plain_outcome.txid, plain.txid());
    assert_eq!(witnessed_outcome.txid, witnessed.txid());
    assert_eq!(plain_outcome.encoded_bytes, plain.encode().len());
    assert_eq!(witnessed_outcome.encoded_bytes, witnessed.encode().len());
    assert_ne!(plain_outcome.txid, witnessed_outcome.txid);
    assert_ne!(plain_outcome.encoded_bytes, witnessed_outcome.encoded_bytes);
}

#[test]
fn duplicate_txid_rejection_is_atomic() {
    let previous = outpoint(0x21, 0);
    let chain = state_with(vec![(previous, entry(100, 1, false))]);
    let tx = spend(vec![previous], &[90], 1);
    let chain_base = base(0x22, 20);
    let mut pool = Mempool::new(chain_base, MempoolConfig::default()).unwrap();
    pool.admit(tx.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();
    let before = snapshot(&pool);

    assert_eq!(
        pool.admit(tx.clone(), chain_base, &chain, &AcceptTestSpends),
        Err(MempoolError::AlreadyKnown(tx.txid()))
    );
    assert_eq!(snapshot(&pool), before);
}

#[test]
fn conflicting_spend_rejection_is_atomic() {
    let previous = outpoint(0x31, 0);
    let chain = state_with(vec![(previous, entry(100, 1, false))]);
    let first = spend(vec![previous], &[90], 1);
    let conflict = spend(vec![previous], &[80], 2);
    let chain_base = base(0x32, 20);
    let mut pool = Mempool::new(chain_base, MempoolConfig::default()).unwrap();
    pool.admit(first.clone(), chain_base, &chain, &AcceptTestSpends)
        .unwrap();
    let before = snapshot(&pool);

    assert_eq!(
        pool.admit(conflict, chain_base, &chain, &AcceptTestSpends),
        Err(MempoolError::Conflict {
            outpoint: previous,
            existing_txid: first.txid(),
        })
    );
    assert_eq!(snapshot(&pool), before);
}

#[test]
fn missing_dependency_rejection_is_atomic() {
    let missing = outpoint(0x41, 0);
    let chain = state_with(vec![]);
    let tx = spend(vec![missing], &[1], 1);
    let chain_base = base(0x42, 20);
    let mut pool = Mempool::new(chain_base, MempoolConfig::default()).unwrap();
    let before = snapshot(&pool);

    assert_eq!(
        pool.admit(tx, chain_base, &chain, &AcceptTestSpends),
        Err(MempoolError::MissingDependency(missing))
    );
    assert_eq!(snapshot(&pool), before);
}

#[test]
fn structural_rejection_is_atomic() {
    let chain = state_with(vec![]);
    let tx = Transaction {
        version: 1,
        inputs: vec![],
        outputs: vec![TxOutput {
            value: Amount::from_base_units(1).unwrap(),
            locking_program: vec![],
        }],
        lock_time: 0,
    };
    let chain_base = base(0x52, 20);
    let mut pool = Mempool::new(chain_base, MempoolConfig::default()).unwrap();
    let before = snapshot(&pool);

    assert_eq!(
        pool.admit(tx, chain_base, &chain, &AcceptTestSpends),
        Err(MempoolError::Structural(
            NormalTransactionError::EmptyInputs
        ))
    );
    assert_eq!(snapshot(&pool), before);
}

#[test]
fn rejecting_verifier_is_atomic() {
    let previous = outpoint(0x61, 0);
    let chain = state_with(vec![(previous, entry(100, 1, false))]);
    let tx = spend(vec![previous], &[90], 1);
    let chain_base = base(0x62, 20);
    let mut pool = Mempool::new(chain_base, MempoolConfig::default()).unwrap();
    let before = snapshot(&pool);

    assert_eq!(
        pool.admit(tx, chain_base, &chain, &RejectTestSpends),
        Err(MempoolError::Utxo(UtxoError::SpendAuthorizationFailed))
    );
    assert_eq!(snapshot(&pool), before);
}

#[test]
fn stale_chain_base_rejection_is_atomic() {
    let previous = outpoint(0x71, 0);
    let chain = state_with(vec![(previous, entry(100, 1, false))]);
    let tx = spend(vec![previous], &[90], 1);
    let pool_base = base(0x72, 20);
    let stale_base = base(0x73, 20);
    let mut pool = Mempool::new(pool_base, MempoolConfig::default()).unwrap();
    let before = snapshot(&pool);

    assert_eq!(
        pool.admit(tx, stale_base, &chain, &AcceptTestSpends),
        Err(MempoolError::StaleChainContext)
    );
    assert_eq!(snapshot(&pool), before);
}

#[test]
fn maximum_tip_height_rejection_is_atomic() {
    let previous = outpoint(0x81, 0);
    let chain = state_with(vec![(previous, entry(100, 1, false))]);
    let tx = spend(vec![previous], &[90], 1);
    let chain_base = base(0x82, u64::MAX);
    let mut pool = Mempool::new(chain_base, MempoolConfig::default()).unwrap();
    let before = snapshot(&pool);

    assert_eq!(
        pool.admit(tx, chain_base, &chain, &AcceptTestSpends),
        Err(MempoolError::HeightOverflow)
    );
    assert_eq!(snapshot(&pool), before);
}

#[test]
fn coinbase_maturity_uses_next_block_height() {
    let previous = outpoint(0x91, 0);
    let immature_chain = state_with(vec![(previous, entry(100, 10, true))]);
    let tx = spend(vec![previous], &[90], 1);
    let immature_base = base(0x92, 128);
    let mut immature_pool = Mempool::new(immature_base, MempoolConfig::default()).unwrap();
    let before = snapshot(&immature_pool);

    assert_eq!(
        immature_pool.admit(
            tx.clone(),
            immature_base,
            &immature_chain,
            &AcceptTestSpends,
        ),
        Err(MempoolError::Utxo(UtxoError::ImmatureCoinbase))
    );
    assert_eq!(snapshot(&immature_pool), before);

    let mature_chain = state_with(vec![(previous, entry(100, 10, true))]);
    let mature_base = base(0x93, 129);
    let mut mature_pool = Mempool::new(mature_base, MempoolConfig::default()).unwrap();
    let outcome = mature_pool
        .admit(tx.clone(), mature_base, &mature_chain, &AcceptTestSpends)
        .expect("height 130 is mature");
    assert_eq!(outcome.fee, 10);
    assert!(mature_pool.contains(&tx.txid()));
}
