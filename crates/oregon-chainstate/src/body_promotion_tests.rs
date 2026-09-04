use oregon_consensus::params::KEY_COMMIT_V1;
use oregon_primitives::{
    Amount, Block, BlockHeader, FOUNDER_ALLOCATION_BASE_UNITS, Hash256, OutPoint, Transaction,
    TxInput, TxOutput, transaction_root, write_varint,
};
use oregon_storage::ValidationStatus;

use crate::test_support::{AcceptAllSpends, TestDir, standard_chain_config};
use crate::{AcceptOutcome, ChainState, HeaderImportStatus};

fn height_one_founder_block(config: &crate::ChainConfig) -> Block {
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
            nonce: 801,
        },
        transactions,
    }
}

#[test]
fn retained_body_promotes_known_header_to_fully_validated_active_block() {
    let dir = TestDir::scoped("body-promotion", "height-one");
    let config = standard_chain_config();
    let block = height_one_founder_block(&config);
    let block_id = block.header.block_id();
    let founder_outpoint = OutPoint {
        txid: block.transactions[0].txid(),
        index: 0,
    };
    let mut state = ChainState::open(dir.path(), config).unwrap();

    let header_out = state.accept_header(block.header.clone()).unwrap();
    assert_eq!(header_out.status, HeaderImportStatus::Preferred);
    assert_eq!(header_out.block_id, block_id);
    assert_eq!(state.tip().height, 0);
    let before = state.storage().get_index(block_id).unwrap().unwrap();
    assert_eq!(before.validation, ValidationStatus::HeaderValidated);
    assert!(!before.body_retained);
    assert_eq!(state.storage().get_block(block_id).unwrap(), None);

    assert_eq!(
        state.accept_block(block.clone(), &AcceptAllSpends).unwrap(),
        AcceptOutcome::Extended
    );
    assert_eq!(state.tip().block_id, block_id);
    assert_eq!(state.tip().height, 1);
    assert_eq!(state.preferred_header_tip().block_id, block_id);
    let after = state.storage().get_index(block_id).unwrap().unwrap();
    assert_eq!(after.validation, ValidationStatus::FullyValidated);
    assert!(after.body_retained);
    assert_eq!(state.storage().get_block(block_id).unwrap(), Some(block));
    assert!(state.storage().get_undo(block_id).unwrap().is_some());
    assert_eq!(state.storage().active_id_at_height(1).unwrap(), Some(block_id));
    assert!(state.utxos().get(&founder_outpoint).is_some());
}
