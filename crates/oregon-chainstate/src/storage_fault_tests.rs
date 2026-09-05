use oregon_consensus::params::KEY_COMMIT_V1;
use oregon_primitives::{
    Amount, Block, BlockHeader, FOUNDER_ALLOCATION_BASE_UNITS, Hash256, Transaction, TxInput,
    TxOutput, transaction_root, write_varint,
};

use crate::test_support::{AcceptAllSpends, TestDir, standard_chain_config};
use crate::{AcceptOutcome, ChainConfig, ChainState, ChainStateError, SessionHealth};

fn height_one_founder_block(config: &ChainConfig) -> Block {
    let mut height_bytes = Vec::new();
    write_varint(1, &mut height_bytes);
    let mut founder_program = vec![KEY_COMMIT_V1];
    founder_program.extend_from_slice(&config.params.founder_key_commitment);
    let coinbase = Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_txid: Hash256::from_bytes([0; 32]),
            previous_output_index: u32::MAX,
            sequence: u32::MAX,
            witness: vec![height_bytes],
        }],
        outputs: vec![TxOutput {
            value: Amount::from_base_units(FOUNDER_ALLOCATION_BASE_UNITS).unwrap(),
            locking_program: founder_program,
        }],
        lock_time: 0,
    };
    let transactions = vec![coinbase];
    Block {
        header: BlockHeader {
            version: 1,
            previous_block: config.anchor_header.block_id(),
            transaction_root: transaction_root(&transactions).unwrap(),
            timestamp: config.genesis_timestamp + 1,
            difficulty_commitment: config.params.initial_target.to_le_bytes(),
            nonce: 101,
        },
        transactions,
    }
}

fn height_one_header(config: &ChainConfig, nonce: u64) -> BlockHeader {
    let mut root = [0u8; 32];
    root[..8].copy_from_slice(&1u64.to_le_bytes());
    root[8..16].copy_from_slice(&nonce.to_le_bytes());
    BlockHeader {
        version: 1,
        previous_block: config.anchor_header.block_id(),
        transaction_root: Hash256::from_bytes(root),
        timestamp: config.genesis_timestamp + 300,
        difficulty_commitment: config.params.initial_target.to_le_bytes(),
        nonce,
    }
}

fn reorg_coinbase(config: &ChainConfig, height: u64, miner_tag: u8) -> Transaction {
    let mut height_bytes = Vec::new();
    write_varint(height, &mut height_bytes);
    let mut outputs = Vec::new();
    if height == 1 {
        let mut founder_program = vec![KEY_COMMIT_V1];
        founder_program.extend_from_slice(&config.params.founder_key_commitment);
        outputs.push(TxOutput {
            value: Amount::from_base_units(FOUNDER_ALLOCATION_BASE_UNITS).unwrap(),
            locking_program: founder_program,
        });
    }
    outputs.push(TxOutput {
        value: Amount::from_base_units(1).unwrap(),
        locking_program: vec![miner_tag],
    });
    Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_txid: Hash256::from_bytes([0; 32]),
            previous_output_index: u32::MAX,
            sequence: u32::MAX,
            witness: vec![height_bytes],
        }],
        outputs,
        lock_time: 0,
    }
}

fn reorg_block(
    config: &ChainConfig,
    parent: Hash256,
    height: u64,
    nonce_domain: u64,
    miner_tag: u8,
) -> Block {
    let transactions = vec![reorg_coinbase(config, height, miner_tag)];
    Block {
        header: BlockHeader {
            version: 1,
            previous_block: parent,
            transaction_root: transaction_root(&transactions).unwrap(),
            timestamp: config.genesis_timestamp + height * 300,
            difficulty_commitment: config.params.initial_target.to_le_bytes(),
            nonce: nonce_domain + height,
        },
        transactions,
    }
}

#[test]
fn durable_failure_faults_session_without_publishing_or_persisting_candidate() {
    let dir = TestDir::scoped("storage-fault", "direct-extension");
    let config = standard_chain_config();
    let block = height_one_founder_block(&config);
    let block_id = block.header.block_id();

    let mut state = ChainState::open(dir.path(), config.clone()).unwrap();
    let before_tip = state.tip().clone();
    let before_utxos = state.utxos().clone();
    state.test_fail_next_durable_write();

    assert!(matches!(
        state.accept_block(block.clone(), &AcceptAllSpends),
        Err(ChainStateError::Storage(_))
    ));
    assert_eq!(state.tip(), &before_tip);
    assert_eq!(state.utxos(), &before_utxos);
    assert_eq!(state.session_health(), SessionHealth::StorageFaulted);

    let mut invalid_second = block;
    invalid_second.header.previous_block = Hash256::from_bytes([0xee; 32]);
    assert!(matches!(
        state.accept_block(invalid_second, &AcceptAllSpends),
        Err(ChainStateError::StorageFaulted)
    ));
    assert_eq!(state.tip(), &before_tip);
    assert_eq!(state.utxos(), &before_utxos);
    drop(state);

    let reopened = ChainState::open(dir.path(), config).unwrap();
    assert_eq!(reopened.tip(), &before_tip);
    assert_eq!(reopened.utxos(), &before_utxos);
    assert_eq!(reopened.session_health(), SessionHealth::Healthy);
    drop(reopened);

    let db = oregon_storage::OregonDb::open(dir.path()).unwrap();
    assert_eq!(db.get_index(block_id).unwrap(), None);
    assert_eq!(db.get_block(block_id).unwrap(), None);
    assert_eq!(db.active_tip().unwrap(), Some((before_tip.block_id, 0)));
}

#[test]
fn header_durable_failure_faults_session_without_publishing_or_persisting_candidate() {
    let dir = TestDir::scoped("storage-fault", "header-import");
    let config = standard_chain_config();
    let header = height_one_header(&config, 202);
    let header_id = header.block_id();

    let mut state = ChainState::open(dir.path(), config.clone()).unwrap();
    let before_tip = state.tip().clone();
    let before_header_tip = state.preferred_header_tip().clone();
    let before_utxos = state.utxos().clone();
    state.test_fail_next_durable_write();

    assert!(matches!(
        state.accept_header(header.clone()),
        Err(ChainStateError::Storage(_))
    ));
    assert_eq!(state.tip(), &before_tip);
    assert_eq!(state.preferred_header_tip(), &before_header_tip);
    assert_eq!(state.utxos(), &before_utxos);
    assert_eq!(state.session_health(), SessionHealth::StorageFaulted);
    assert!(matches!(
        state.accept_header(header),
        Err(ChainStateError::StorageFaulted)
    ));
    drop(state);

    let reopened = ChainState::open(dir.path(), config).unwrap();
    assert_eq!(reopened.tip(), &before_tip);
    assert_eq!(reopened.preferred_header_tip(), &before_header_tip);
    assert_eq!(reopened.utxos(), &before_utxos);
    assert_eq!(reopened.session_health(), SessionHealth::Healthy);
    drop(reopened);

    let db = oregon_storage::OregonDb::open(dir.path()).unwrap();
    assert_eq!(db.get_index(header_id).unwrap(), None);
    assert_eq!(db.active_tip().unwrap(), Some((before_tip.block_id, 0)));
    assert_eq!(
        db.preferred_header_tip().unwrap(),
        Some((before_header_tip.block_id, 0))
    );
}

#[test]
fn reorg_durable_failure_publishes_neither_active_nor_preferred_state() {
    let dir = TestDir::scoped("storage-fault", "reorg");
    let config = standard_chain_config();
    let anchor = config.anchor_header.block_id();
    let mut state = ChainState::open(dir.path(), config.clone()).unwrap();

    let active1 = reorg_block(&config, anchor, 1, 10_000, 0xa1);
    let active1_id = active1.header.block_id();
    assert_eq!(
        state.accept_block(active1, &AcceptAllSpends).unwrap(),
        AcceptOutcome::Extended
    );
    let active2 = reorg_block(&config, active1_id, 2, 10_000, 0xa2);
    let active2_id = active2.header.block_id();
    assert_eq!(
        state.accept_block(active2, &AcceptAllSpends).unwrap(),
        AcceptOutcome::Extended
    );

    let side1 = reorg_block(&config, anchor, 1, 20_000, 0xb1);
    let side1_id = side1.header.block_id();
    assert_eq!(
        state.accept_block(side1, &AcceptAllSpends).unwrap(),
        AcceptOutcome::StoredSideChain
    );
    let side2 = reorg_block(&config, side1_id, 2, 20_000, 0xb2);
    let side2_id = side2.header.block_id();
    assert_eq!(
        state.accept_block(side2, &AcceptAllSpends).unwrap(),
        AcceptOutcome::StoredSideChain
    );
    let candidate3 = reorg_block(&config, side2_id, 3, 20_000, 0xb3);
    let candidate3_id = candidate3.header.block_id();

    let before_tip = state.tip().clone();
    let before_header_tip = state.preferred_header_tip().clone();
    let before_utxos = state.utxos().clone();
    assert_eq!(before_tip.block_id, active2_id);
    assert_eq!(before_header_tip.block_id, active2_id);
    state.test_fail_next_durable_write();

    assert!(matches!(
        state.accept_block(candidate3, &AcceptAllSpends),
        Err(ChainStateError::Storage(_))
    ));
    assert_eq!(state.tip(), &before_tip);
    assert_eq!(state.preferred_header_tip(), &before_header_tip);
    assert_eq!(state.utxos(), &before_utxos);
    assert_eq!(state.session_health(), SessionHealth::StorageFaulted);
    drop(state);

    let reopened = ChainState::open(dir.path(), config).unwrap();
    assert_eq!(reopened.tip(), &before_tip);
    assert_eq!(reopened.preferred_header_tip(), &before_header_tip);
    assert_eq!(reopened.utxos(), &before_utxos);
    assert_eq!(reopened.session_health(), SessionHealth::Healthy);
    drop(reopened);

    let db = oregon_storage::OregonDb::open(dir.path()).unwrap();
    assert_eq!(db.active_tip().unwrap(), Some((active2_id, 2)));
    assert_eq!(db.preferred_header_tip().unwrap(), Some((active2_id, 2)));
    assert_eq!(db.active_id_at_height(3).unwrap(), None);
    assert_eq!(db.get_index(candidate3_id).unwrap(), None);
    assert_eq!(db.get_block(candidate3_id).unwrap(), None);
}
