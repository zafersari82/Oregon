use oregon_primitives::{BlockHeader, Hash256};
use oregon_sync::{ChainSyncView, SyncTip, SyncViewError};

use crate::core::{SyncProbeState, spawn_sync_probe_worker, test_core_channel};
use crate::sync_adapter::NodeSyncView;

fn header(tag: u8) -> BlockHeader {
    BlockHeader {
        version: 1,
        previous_block: Hash256::from_bytes([tag; 32]),
        transaction_root: Hash256::from_bytes([tag.wrapping_add(1); 32]),
        timestamp: 1_900_000_000 + u64::from(tag),
        difficulty_commitment: [0xff; 32],
        nonce: u64::from(tag),
    }
}

#[tokio::test]
async fn sync_adapter_reads_authoritative_values_through_bounded_core_commands() {
    let active = SyncTip {
        block_id: Hash256::from_bytes([0x11; 32]),
        height: 7,
    };
    let preferred = SyncTip {
        block_id: Hash256::from_bytes([0x22; 32]),
        height: 9,
    };
    let active_at_five = Hash256::from_bytes([0x33; 32]);
    let preferred_at_five = Hash256::from_bytes([0x44; 32]);
    let preferred_header = header(0x55);
    let retained = Hash256::from_bytes([0x66; 32]);
    let view = NodeSyncView::new(spawn_sync_probe_worker(SyncProbeState {
        active,
        preferred,
        active_at_height: (5, active_at_five),
        preferred_at_height: (5, preferred_at_five),
        preferred_header: (5, preferred_header.clone()),
        retained,
    }));

    assert_eq!(view.active_tip().await.unwrap(), active);
    assert_eq!(view.preferred_header_tip().await.unwrap(), preferred);
    assert_eq!(
        view.active_id_at_height(5).await.unwrap(),
        Some(active_at_five)
    );
    assert_eq!(
        view.preferred_header_id_at_height(5).await.unwrap(),
        Some(preferred_at_five)
    );
    assert_eq!(
        view.preferred_header_at_height(5).await.unwrap(),
        Some(preferred_header)
    );
    assert!(view.body_retained(retained).await.unwrap());
    assert_eq!(view.active_id_at_height(6).await.unwrap(), None);
    assert!(!view.body_retained(Hash256::from_bytes([0x77; 32])).await.unwrap());
}

#[tokio::test]
async fn closed_core_maps_every_sync_read_to_coarse_unavailable() {
    let (handle, receiver) = test_core_channel();
    drop(receiver);
    let view = NodeSyncView::new(handle);
    let id = Hash256::from_bytes([0x88; 32]);

    assert_eq!(view.active_tip().await, Err(SyncViewError::Unavailable));
    assert_eq!(
        view.preferred_header_tip().await,
        Err(SyncViewError::Unavailable)
    );
    assert_eq!(
        view.active_id_at_height(1).await,
        Err(SyncViewError::Unavailable)
    );
    assert_eq!(
        view.preferred_header_id_at_height(1).await,
        Err(SyncViewError::Unavailable)
    );
    assert_eq!(
        view.preferred_header_at_height(1).await,
        Err(SyncViewError::Unavailable)
    );
    assert_eq!(
        view.body_retained(id).await,
        Err(SyncViewError::Unavailable)
    );
}

#[test]
fn production_sync_adapter_keeps_chainstate_errors_below_node_boundary() {
    let adapter = include_str!("sync_adapter.rs");
    let core = include_str!("core.rs");

    assert!(adapter.contains("impl ChainSyncView for NodeSyncView"));
    assert!(adapter.contains("SyncViewError::Unavailable"));
    assert!(!adapter.contains("ChainStateError"));
    assert!(!adapter.contains("StorageError"));

    for authoritative_read in [
        "state.tip()",
        "state.preferred_header_tip()",
        "state.active_id_at_height",
        "state.preferred_header_id_at_height",
        "state.preferred_header_at_height",
        "state.body_retained",
    ] {
        assert!(
            core.contains(authoritative_read),
            "missing core-owned authoritative read: {authoritative_read}"
        );
    }
}
