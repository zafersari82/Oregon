use oregon_primitives::{BlockHeader, Hash256};

use crate::test_support::{TestDir, standard_chain_config};
use crate::{ChainState, HeaderImportStatus};

fn candidate_header(
    config: &crate::ChainConfig,
    parent: Hash256,
    height: u64,
    nonce: u64,
) -> BlockHeader {
    let mut root = [0u8; 32];
    root[..8].copy_from_slice(&height.to_le_bytes());
    root[8..16].copy_from_slice(&nonce.to_le_bytes());
    BlockHeader {
        version: 1,
        previous_block: parent,
        transaction_root: Hash256::from_bytes(root),
        timestamp: config.genesis_timestamp + height * 300,
        difficulty_commitment: config.params.initial_target.to_le_bytes(),
        nonce,
    }
}

#[test]
fn sync_view_reads_preferred_and_active_ancestry_without_mutating_state() {
    let dir = TestDir::scoped("sync-view", "ancestry");
    let config = standard_chain_config();
    let anchor_id = config.anchor_header.block_id();
    let mut state = ChainState::open(dir.path(), config.clone()).unwrap();
    let active_before = state.tip().clone();
    let utxos_before = state.utxos().clone();

    let first = candidate_header(&config, anchor_id, 1, 901);
    let first_id = first.block_id();
    assert_eq!(
        state.accept_header(first.clone()).unwrap().status,
        HeaderImportStatus::Preferred
    );
    let second = candidate_header(&config, first_id, 2, 902);
    let second_id = second.block_id();
    assert_eq!(
        state.accept_header(second.clone()).unwrap().status,
        HeaderImportStatus::Preferred
    );

    assert_eq!(state.chain_id(), anchor_id);
    assert_eq!(
        state.preferred_header_id_at_height(0).unwrap(),
        Some(anchor_id)
    );
    assert_eq!(
        state.preferred_header_id_at_height(1).unwrap(),
        Some(first_id)
    );
    assert_eq!(
        state.preferred_header_id_at_height(2).unwrap(),
        Some(second_id)
    );
    assert_eq!(state.preferred_header_id_at_height(3).unwrap(), None);
    assert_eq!(state.preferred_header_at_height(1).unwrap(), Some(first));
    assert_eq!(state.preferred_header_at_height(2).unwrap(), Some(second));
    assert_eq!(state.preferred_header_at_height(3).unwrap(), None);
    assert_eq!(state.active_id_at_height(0).unwrap(), Some(anchor_id));
    assert_eq!(state.active_id_at_height(1).unwrap(), None);
    assert!(!state.body_retained(first_id).unwrap());
    assert!(
        !state
            .body_retained(Hash256::from_bytes([0xee; 32]))
            .unwrap()
    );

    assert_eq!(state.tip(), &active_before);
    assert_eq!(state.utxos(), &utxos_before);
}
