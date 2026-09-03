use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use oregon_consensus::{ChainWork, ConsensusParams, Target};
use oregon_primitives::{BlockHeader, Hash256};

use crate::{ChainConfig, ChainState, SessionHealth};

struct TestDir(PathBuf);

impl TestDir {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "oregon-chainstate-{label}-{}-{n}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn test_anchor(genesis_timestamp: u64, nonce: u64) -> BlockHeader {
    BlockHeader {
        version: 1,
        previous_block: Hash256::from_bytes([0u8; 32]),
        transaction_root: Hash256::from_bytes([0x22; 32]),
        timestamp: genesis_timestamp,
        difficulty_commitment: [0xff; 32],
        nonce,
    }
}

fn test_config(genesis_timestamp: u64, nonce: u64) -> ChainConfig {
    let target = Target::from_le_bytes([0xff; 32]).unwrap();
    ChainConfig {
        anchor_header: test_anchor(genesis_timestamp, nonce),
        genesis_timestamp,
        params: ConsensusParams::new(target.clone(), target, [0x42; 32]).unwrap(),
    }
}

#[test]
fn bootstrap_new_database_persists_zero_work_anchor_and_reopens_identically() {
    let dir = TestDir::new("bootstrap");
    let config = test_config(1_800_000_000, 7);
    let anchor_id = config.anchor_header.block_id();

    let state = ChainState::open(dir.path(), config.clone()).unwrap();
    assert_eq!(state.tip().block_id, anchor_id);
    assert_eq!(state.tip().height, 0);
    assert_eq!(state.tip().cumulative_work, ChainWork::zero());
    assert!(state.utxos().entries().next().is_none());
    assert_eq!(state.session_health(), SessionHealth::Healthy);
    drop(state);

    let reopened = ChainState::open(dir.path(), config).unwrap();
    assert_eq!(reopened.tip().block_id, anchor_id);
    assert_eq!(reopened.tip().height, 0);
    assert_eq!(reopened.tip().cumulative_work, ChainWork::zero());
    assert!(reopened.utxos().entries().next().is_none());
    assert_eq!(reopened.session_health(), SessionHealth::Healthy);
}

#[test]
fn reopen_fails_closed_if_anchor_or_genesis_timestamp_changes() {
    let dir = TestDir::new("config-binding");
    let original = test_config(1_800_000_000, 7);
    drop(ChainState::open(dir.path(), original.clone()).unwrap());

    assert!(ChainState::open(dir.path(), test_config(1_800_000_000, 8)).is_err());
    assert!(ChainState::open(dir.path(), test_config(1_800_000_001, 7)).is_err());

    let reopened = ChainState::open(dir.path(), original).unwrap();
    assert_eq!(reopened.tip().height, 0);
    assert_eq!(reopened.tip().cumulative_work, ChainWork::zero());
}
