use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use oregon_chainstate::{ChainConfig, ChainState};
use oregon_consensus::{ConsensusParams, Target};
use oregon_primitives::{BlockHeader, Hash256};
use oregon_storage::OregonDb;

fn test_path() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "oregon-public-prune-{}-{n}",
        std::process::id()
    ))
}

fn test_config() -> ChainConfig {
    let target = Target::from_le_bytes([0xff; 32]).unwrap();
    let genesis_timestamp = 1_800_000_000;
    ChainConfig {
        anchor_header: BlockHeader {
            version: 1,
            previous_block: Hash256::from_bytes([0; 32]),
            transaction_root: Hash256::from_bytes([0x22; 32]),
            timestamp: genesis_timestamp,
            difficulty_commitment: target.to_le_bytes(),
            nonce: 7,
        },
        genesis_timestamp,
        params: ConsensusParams::new(target, target, [0x42; 32]).unwrap(),
    }
}

#[test]
fn chainstate_prune_runs_maintenance_without_changing_consensus_state() {
    let path = test_path();
    std::fs::create_dir_all(&path).unwrap();
    let config = test_config();
    let mut state = ChainState::open(&path, config.clone()).unwrap();
    let before_tip = state.tip().clone();
    let before_utxos = state.utxos().clone();

    let report = state.prune().unwrap();

    assert_eq!(report.deleted_bodies, 0);
    assert_eq!(report.deleted_undos, 0);
    assert_eq!(state.tip(), &before_tip);
    assert_eq!(state.utxos(), &before_utxos);
    drop(state);

    let db = OregonDb::open(&path).unwrap();
    assert_eq!(db.prune_cursor().unwrap(), Some(0));
    drop(db);

    let reopened = ChainState::open(&path, config).unwrap();
    assert_eq!(reopened.tip(), &before_tip);
    assert_eq!(reopened.utxos(), &before_utxos);
    drop(reopened);
    let _ = std::fs::remove_dir_all(path);
}
