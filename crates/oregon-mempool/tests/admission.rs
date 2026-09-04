use oregon_mempool::{ChainBase, Mempool, MempoolConfig, MempoolError};
use oregon_primitives::Hash256;

fn base(tag: u8, height: u64) -> ChainBase {
    ChainBase {
        tip_id: Hash256::from_bytes([tag; 32]),
        tip_height: height,
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
