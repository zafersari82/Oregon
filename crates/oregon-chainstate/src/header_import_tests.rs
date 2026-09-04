use oregon_consensus::{ConsensusError, ConsensusParams, Target, block_work};
use oregon_primitives::{BlockHeader, Hash256};
use oregon_storage::ValidationStatus;

use crate::test_support::{TestDir, standard_chain_config};
use crate::{ChainConfig, ChainState, ChainStateError, HeaderImportStatus};

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

fn target_one_chain_config() -> ChainConfig {
    let mut target_bytes = [0u8; 32];
    target_bytes[0] = 1;
    let target = Target::from_le_bytes(target_bytes).unwrap();
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

#[test]
fn lower_work_valid_header_is_stored_without_replacing_preferred_tip() {
    let dir = TestDir::scoped("header-import", "stored-lower-work");
    let config = standard_chain_config();
    let anchor_id = config.anchor_header.block_id();
    let mut state = ChainState::open(dir.path(), config.clone()).unwrap();

    let first = candidate_header(&config, anchor_id, 1, 301);
    let first_id = first.block_id();
    assert_eq!(
        state.accept_header(first).unwrap().status,
        HeaderImportStatus::Preferred
    );
    let second = candidate_header(&config, first_id, 2, 302);
    let second_id = second.block_id();
    assert_eq!(
        state.accept_header(second).unwrap().status,
        HeaderImportStatus::Preferred
    );
    let preferred_before = state.preferred_header_tip().clone();
    let active_before = state.tip().clone();

    let lower_work = candidate_header(&config, anchor_id, 1, 303);
    let lower_work_id = lower_work.block_id();
    let out = state.accept_header(lower_work.clone()).unwrap();

    assert_eq!(out.block_id, lower_work_id);
    assert_eq!(out.height, 1);
    assert_eq!(out.status, HeaderImportStatus::Stored);
    assert_eq!(out.preferred_tip, preferred_before);
    assert_eq!(state.preferred_header_tip(), &preferred_before);
    assert_eq!(state.tip(), &active_before);
    assert_eq!(
        state.storage().preferred_header_tip().unwrap(),
        Some((second_id, 2))
    );
    let index = state.storage().get_index(lower_work_id).unwrap().unwrap();
    assert_eq!(index.header, lower_work);
    assert_eq!(index.validation, ValidationStatus::HeaderValidated);
    assert!(!index.body_retained);
}

#[test]
fn unknown_parent_header_is_rejected_without_persistence_or_tip_mutation() {
    let dir = TestDir::scoped("header-import", "unknown-parent");
    let config = standard_chain_config();
    let missing_parent = Hash256::from_bytes([0xab; 32]);
    let header = candidate_header(&config, missing_parent, 1, 404);
    let header_id = header.block_id();
    let mut state = ChainState::open(dir.path(), config).unwrap();
    let active_before = state.tip().clone();
    let preferred_before = state.preferred_header_tip().clone();
    let utxos_before = state.utxos().clone();

    assert!(matches!(
        state.accept_header(header),
        Err(ChainStateError::UnknownParent(parent)) if parent == missing_parent
    ));
    assert_eq!(state.tip(), &active_before);
    assert_eq!(state.preferred_header_tip(), &preferred_before);
    assert_eq!(state.utxos(), &utxos_before);
    assert_eq!(state.storage().get_index(header_id).unwrap(), None);
    assert_eq!(
        state.storage().preferred_header_tip().unwrap(),
        Some((preferred_before.block_id, preferred_before.height))
    );
}

#[test]
fn preferred_header_chain_reopens_without_advancing_active_state() {
    let dir = TestDir::scoped("header-import", "reopen-preferred");
    let config = standard_chain_config();
    let anchor_id = config.anchor_header.block_id();
    let mut state = ChainState::open(dir.path(), config.clone()).unwrap();
    let active_before = state.tip().clone();
    let utxos_before = state.utxos().clone();

    let first = candidate_header(&config, anchor_id, 1, 501);
    let first_id = first.block_id();
    assert_eq!(
        state.accept_header(first).unwrap().status,
        HeaderImportStatus::Preferred
    );
    let second = candidate_header(&config, first_id, 2, 502);
    let second_id = second.block_id();
    assert_eq!(
        state.accept_header(second).unwrap().status,
        HeaderImportStatus::Preferred
    );

    let preferred_before = state.preferred_header_tip().clone();
    assert_eq!(preferred_before.block_id, second_id);
    assert_eq!(preferred_before.height, 2);
    assert_eq!(state.tip(), &active_before);
    assert_eq!(state.utxos(), &utxos_before);
    drop(state);

    let reopened = ChainState::open(dir.path(), config).unwrap();
    assert_eq!(reopened.preferred_header_tip(), &preferred_before);
    assert_eq!(reopened.tip(), &active_before);
    assert_eq!(reopened.utxos(), &utxos_before);
    assert_eq!(
        reopened.storage().preferred_header_tip().unwrap(),
        Some((second_id, 2))
    );
}

#[test]
fn contextual_invalid_header_is_rejected_without_persistence_or_tip_mutation() {
    let dir = TestDir::scoped("header-import", "invalid-mtp");
    let config = standard_chain_config();
    let anchor_id = config.anchor_header.block_id();
    let mut header = candidate_header(&config, anchor_id, 1, 601);
    header.timestamp = config.genesis_timestamp;
    let header_id = header.block_id();
    let mut state = ChainState::open(dir.path(), config).unwrap();
    let active_before = state.tip().clone();
    let preferred_before = state.preferred_header_tip().clone();
    let utxos_before = state.utxos().clone();

    assert!(matches!(
        state.accept_header(header),
        Err(ChainStateError::Consensus(
            ConsensusError::TimestampNotAfterMtp
        ))
    ));
    assert_eq!(state.tip(), &active_before);
    assert_eq!(state.preferred_header_tip(), &preferred_before);
    assert_eq!(state.utxos(), &utxos_before);
    assert_eq!(state.storage().get_index(header_id).unwrap(), None);
    assert_eq!(
        state.storage().preferred_header_tip().unwrap(),
        Some((preferred_before.block_id, preferred_before.height))
    );
}

#[test]
fn insufficient_pow_header_is_rejected_without_persistence_or_tip_mutation() {
    let dir = TestDir::scoped("header-import", "invalid-pow");
    let config = target_one_chain_config();
    let anchor_id = config.anchor_header.block_id();
    let header = candidate_header(&config, anchor_id, 1, 701);
    let header_id = header.block_id();
    let mut state = ChainState::open(dir.path(), config).unwrap();
    let active_before = state.tip().clone();
    let preferred_before = state.preferred_header_tip().clone();
    let utxos_before = state.utxos().clone();

    assert!(matches!(
        state.accept_header(header),
        Err(ChainStateError::Consensus(
            ConsensusError::InsufficientProofOfWork
        ))
    ));
    assert_eq!(state.tip(), &active_before);
    assert_eq!(state.preferred_header_tip(), &preferred_before);
    assert_eq!(state.utxos(), &utxos_before);
    assert_eq!(state.storage().get_index(header_id).unwrap(), None);
    assert_eq!(
        state.storage().preferred_header_tip().unwrap(),
        Some((preferred_before.block_id, preferred_before.height))
    );
}
