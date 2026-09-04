use oregon_consensus::block_work;
use oregon_primitives::{BlockHeader, Hash256};
use oregon_storage::ValidationStatus;

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
fn heavier_valid_header_becomes_preferred_without_mutating_active_state() {
    let dir = TestDir::scoped("header-import", "preferred");
    let config = standard_chain_config();
    let anchor_id = config.anchor_header.block_id();
    let mut state = ChainState::open(dir.path(), config.clone()).unwrap();
    let active_before = state.tip().clone();
    let utxos_before = state.utxos().clone();
    let header = candidate_header(&config, anchor_id, 1, 101);
    let block_id = header.block_id();

    let out = state.accept_header(header.clone()).unwrap();

    assert_eq!(out.block_id, block_id);
    assert_eq!(out.height, 1);
    assert_eq!(out.status, HeaderImportStatus::Preferred);
    assert_eq!(out.preferred_tip, *state.preferred_header_tip());
    assert_eq!(state.tip(), &active_before);
    assert_eq!(state.utxos(), &utxos_before);
    assert_eq!(state.preferred_header_tip().block_id, block_id);
    assert_eq!(state.preferred_header_tip().height, 1);
    assert_eq!(
        state.preferred_header_tip().cumulative_work,
        block_work(config.params.initial_target)
    );

    let index = state.storage().get_index(block_id).unwrap().unwrap();
    assert_eq!(index.header, header);
    assert_eq!(index.parent, anchor_id);
    assert_eq!(index.height, 1);
    assert_eq!(index.validation, ValidationStatus::HeaderValidated);
    assert!(!index.body_retained);
    assert!(state.storage().get_block(block_id).unwrap().is_none());
    assert_eq!(
        state.storage().preferred_header_tip().unwrap(),
        Some((block_id, 1))
    );
}

#[test]
fn duplicate_header_returns_known_without_changing_chainstate() {
    let dir = TestDir::scoped("header-import", "known");
    let config = standard_chain_config();
    let anchor_id = config.anchor_header.block_id();
    let mut state = ChainState::open(dir.path(), config.clone()).unwrap();
    let header = candidate_header(&config, anchor_id, 1, 202);
    let block_id = header.block_id();
    assert_eq!(
        state.accept_header(header.clone()).unwrap().status,
        HeaderImportStatus::Preferred
    );
    let active_before = state.tip().clone();
    let preferred_before = state.preferred_header_tip().clone();
    let utxos_before = state.utxos().clone();

    let out = state.accept_header(header).unwrap();

    assert_eq!(out.block_id, block_id);
    assert_eq!(out.height, 1);
    assert_eq!(out.status, HeaderImportStatus::Known);
    assert_eq!(out.preferred_tip, preferred_before);
    assert_eq!(state.preferred_header_tip(), &preferred_before);
    assert_eq!(state.tip(), &active_before);
    assert_eq!(state.utxos(), &utxos_before);
}
