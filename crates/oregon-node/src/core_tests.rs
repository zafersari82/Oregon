use std::thread::ThreadId;

use oregon_primitives::{BlockHeader, Hash256};

use crate::core::{
    CoreSendError, HEADER_VALIDATION_SLICE, MAX_CORE_COMMAND_BYTES, MAX_CORE_COMMANDS,
    spawn_probe_worker, test_core_channel,
};

fn header(nonce: u64) -> BlockHeader {
    BlockHeader {
        version: 1,
        previous_block: Hash256::from_bytes([0u8; 32]),
        transaction_root: Hash256::from_bytes([1u8; 32]),
        timestamp: 1_700_000_000,
        difficulty_commitment: [0xff; 32],
        nonce,
    }
}

#[test]
fn core_queue_has_exact_sixty_four_command_capacity() {
    assert_eq!(MAX_CORE_COMMANDS, 64);
    let (handle, receiver) = test_core_channel();

    for _ in 0..MAX_CORE_COMMANDS {
        handle.try_send_test_bytes(1).unwrap();
    }
    assert_eq!(receiver.len(), MAX_CORE_COMMANDS);
    assert_eq!(handle.try_send_test_bytes(1), Err(CoreSendError::QueueFull));
}

#[tokio::test]
async fn core_byte_budget_is_exact_and_permit_releases_when_envelope_drops() {
    assert_eq!(MAX_CORE_COMMAND_BYTES, 8 * 1024 * 1024);
    let (handle, mut receiver) = test_core_channel();
    assert_eq!(handle.available_bytes(), MAX_CORE_COMMAND_BYTES);

    handle.try_send_test_bytes(MAX_CORE_COMMAND_BYTES).unwrap();
    assert_eq!(handle.available_bytes(), 0);
    assert_eq!(
        handle.try_send_test_bytes(1),
        Err(CoreSendError::ByteBudgetExhausted)
    );

    let envelope = receiver.recv().await.unwrap();
    assert_eq!(handle.available_bytes(), 0);
    drop(envelope);
    assert_eq!(handle.available_bytes(), MAX_CORE_COMMAND_BYTES);
}

#[test]
fn failed_queue_send_releases_acquired_byte_permit() {
    let (handle, _receiver) = test_core_channel();
    for _ in 0..MAX_CORE_COMMANDS {
        handle.try_send_test_bytes(1).unwrap();
    }
    assert_eq!(
        handle.available_bytes(),
        MAX_CORE_COMMAND_BYTES - MAX_CORE_COMMANDS
    );
    assert_eq!(handle.try_send_test_bytes(1), Err(CoreSendError::QueueFull));
    assert_eq!(
        handle.available_bytes(),
        MAX_CORE_COMMAND_BYTES - MAX_CORE_COMMANDS
    );
}

#[test]
fn header_validation_slice_accepts_sixteen_and_rejects_seventeen() {
    assert_eq!(HEADER_VALIDATION_SLICE, 16);
    let (handle, receiver) = test_core_channel();
    handle
        .try_send_headers((0..HEADER_VALIDATION_SLICE as u64).map(header).collect())
        .unwrap();
    assert_eq!(receiver.len(), 1);

    assert_eq!(
        handle.try_send_headers((0..=HEADER_VALIDATION_SLICE as u64).map(header).collect()),
        Err(CoreSendError::HeaderBatchTooLarge)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blocking_core_worker_executes_off_the_tokio_reactor_thread() {
    let reactor_thread: ThreadId = std::thread::current().id();
    let handle = spawn_probe_worker();
    let worker_thread = handle.probe_thread_id().await.unwrap();
    assert_ne!(worker_thread, reactor_thread);
}
