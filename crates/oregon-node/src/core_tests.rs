use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::ThreadId;

use oregon_chainstate::{ChainConfig, ChainState};
use oregon_consensus::{ConsensusParams, Target};
use oregon_mempool::MempoolConfig;
use oregon_network::TcpTransport;
use oregon_primitives::{BlockHeader, Hash256, Transaction};
use oregon_sync::ChainSyncView;
use oregon_utxo::{SpendVerifier, UtxoEntry, UtxoError};

use crate::core::{
    HEADER_VALIDATION_SLICE, MAX_CORE_COMMAND_BYTES, MAX_CORE_COMMANDS, spawn_core,
    spawn_probe_worker, test_core_channel,
};
use crate::{NodeQueueError, OregonNode};

struct NeverVerify;

impl SpendVerifier for NeverVerify {
    fn verify_spend(
        &self,
        _transaction: &Transaction,
        _input_index: usize,
        _prevout: &UtxoEntry,
    ) -> Result<(), UtxoError> {
        unreachable!("constructor contract does not execute spend verification")
    }
}

struct TestDir(PathBuf);

impl TestDir {
    fn scoped(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "oregon-node-core-{label}-{}-{n}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn chain_config() -> ChainConfig {
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

fn valid_headers(config: &ChainConfig, length: u64) -> Vec<BlockHeader> {
    let mut headers = Vec::with_capacity(length as usize);
    let mut previous = config.anchor_header.block_id();
    for height in 1..=length {
        let header = BlockHeader {
            version: 1,
            previous_block: previous,
            transaction_root: Hash256::from_bytes([height as u8; 32]),
            timestamp: config.genesis_timestamp + 300 * height,
            difficulty_commitment: config.params.initial_target.to_le_bytes(),
            nonce: 1_000 + height,
        };
        previous = header.block_id();
        headers.push(header);
    }
    headers
}

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
fn production_core_owner_contract_is_declared_without_dead_code_suppression() {
    let _constructor = spawn_core::<NeverVerify>;
    let source = include_str!("core.rs");
    assert!(source.contains("ChainState"));
    assert!(source.contains("Mempool"));
    assert!(source.contains("submit_block"));
    assert!(source.contains("submit_transaction"));
    assert!(source.contains("spawn_blocking"));
    assert!(!source.contains("allow(dead_code)"));
}

#[test]
fn core_queue_has_exact_sixty_four_command_capacity() {
    assert_eq!(MAX_CORE_COMMANDS, 64);
    let (handle, receiver) = test_core_channel();

    for _ in 0..MAX_CORE_COMMANDS {
        handle.try_send_test_bytes(1).unwrap();
    }
    assert_eq!(receiver.len(), MAX_CORE_COMMANDS);
    assert_eq!(
        handle.try_send_test_bytes(1),
        Err(NodeQueueError::QueueFull)
    );
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
        Err(NodeQueueError::ByteBudgetExhausted)
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
    assert_eq!(
        handle.try_send_test_bytes(1),
        Err(NodeQueueError::QueueFull)
    );
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
        Err(NodeQueueError::HeaderBatchTooLarge)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn node_splits_remote_header_batch_into_core_slices() {
    let dir = TestDir::scoped("header-slicing");
    let config = chain_config();
    let state = ChainState::open(dir.path(), config.clone()).unwrap();
    let node = OregonNode::new(state, MempoolConfig::default(), NeverVerify, TcpTransport)
        .await
        .unwrap();
    let headers = valid_headers(&config, HEADER_VALIDATION_SLICE as u64 + 1);
    let expected_tip = headers.last().unwrap().block_id();

    let results = node.submit_headers(headers).await.unwrap();
    assert_eq!(results.len(), HEADER_VALIDATION_SLICE + 1);
    assert!(results.iter().all(Result::is_ok));

    let preferred = node.sync_view().preferred_header_tip().await.unwrap();
    assert_eq!(preferred.height, (HEADER_VALIDATION_SLICE + 1) as u64);
    assert_eq!(preferred.block_id, expected_tip);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn blocking_core_worker_executes_off_the_tokio_reactor_thread() {
    let reactor_thread: ThreadId = std::thread::current().id();
    let handle = spawn_probe_worker();
    let worker_thread = handle.probe_thread_id().await.unwrap();
    assert_ne!(worker_thread, reactor_thread);
}
