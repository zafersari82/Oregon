use oregon_chainstate::{ChainConfig, ChainState};
use oregon_consensus::{ConsensusParams, Target};
use oregon_primitives::{BlockHeader, Hash256};
use oregon_storage::OregonDb;

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
    let dir = tempfile::tempdir().unwrap();
    let config = test_config();
    let mut state = ChainState::open(dir.path(), config.clone()).unwrap();
    let before_tip = state.tip().clone();
    let before_utxos = state.utxos().clone();

    let report = state.prune().unwrap();

    assert_eq!(report.deleted_bodies, 0);
    assert_eq!(report.deleted_undos, 0);
    assert_eq!(state.tip(), &before_tip);
    assert_eq!(state.utxos(), &before_utxos);
    drop(state);

    let db = OregonDb::open(dir.path()).unwrap();
    assert_eq!(db.prune_cursor().unwrap(), Some(0));
    drop(db);

    let reopened = ChainState::open(dir.path(), config).unwrap();
    assert_eq!(reopened.tip(), &before_tip);
    assert_eq!(reopened.utxos(), &before_utxos);
}
