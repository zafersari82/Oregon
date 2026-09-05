use oregon_chainstate::{AcceptOutcome, ChainStateError};
use oregon_mempool::{AdmissionOutcome, MempoolError};
use oregon_primitives::Hash256;
use oregon_protocol::{InventoryItem, InventoryKind};

use crate::relay::validated_inventory;

#[test]
fn rejected_transaction_never_authorizes_inventory_relay() {
    let txid = Hash256::from_bytes([0x11; 32]);
    let result: Result<AdmissionOutcome, MempoolError> = Err(MempoolError::AlreadyKnown(txid));
    assert_eq!(
        validated_inventory(InventoryKind::Transaction, txid, &result),
        None
    );
}

#[test]
fn accepted_transaction_authorizes_inventory_relay() {
    let txid = Hash256::from_bytes([0x22; 32]);
    let result: Result<AdmissionOutcome, MempoolError> = Ok(AdmissionOutcome {
        txid,
        fee: 1,
        encoded_bytes: 42,
        evicted: Vec::new(),
    });
    assert_eq!(
        validated_inventory(InventoryKind::Transaction, txid, &result),
        Some(InventoryItem {
            kind: InventoryKind::Transaction,
            hash: txid,
        })
    );
}

#[test]
fn invalid_block_never_authorizes_inventory_relay() {
    let block_id = Hash256::from_bytes([0x33; 32]);
    let result: Result<AcceptOutcome, ChainStateError> = Err(ChainStateError::UnknownParent(
        Hash256::from_bytes([0x44; 32]),
    ));
    assert_eq!(
        validated_inventory(InventoryKind::Block, block_id, &result),
        None
    );
}

#[test]
fn accepted_sidechain_block_authorizes_inventory_relay() {
    let block_id = Hash256::from_bytes([0x55; 32]);
    let result: Result<AcceptOutcome, ChainStateError> = Ok(AcceptOutcome::StoredSideChain);
    assert_eq!(
        validated_inventory(InventoryKind::Block, block_id, &result),
        Some(InventoryItem {
            kind: InventoryKind::Block,
            hash: block_id,
        })
    );
}
