use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use oregon_chainstate::{ChainConfig, ChainState};
use oregon_consensus::{ConsensusParams, Target, params::KEY_COMMIT_V1};
use oregon_mempool::MempoolConfig;
use oregon_peer::PeerConfig;
use oregon_primitives::{
    Amount, Block, BlockHeader, FOUNDER_ALLOCATION_BASE_UNITS, Hash256, OutPoint, Transaction,
    TxInput, TxOutput, transaction_root, write_varint,
};
use oregon_protocol::{
    FeatureSet, Hello, PROTOCOL_VERSION_CURRENT, PROTOCOL_VERSION_MIN, network_magic,
};
use oregon_storage::{OregonDb, StorageBatch};
use oregon_utxo::{SpendVerifier, UtxoEntry, UtxoError};

pub struct TestDir(PathBuf);

impl TestDir {
    pub fn scoped(scope: &str, label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "oregon-node-{scope}-{label}-{}-{n}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone, Copy)]
pub struct AcceptAllSpends;

impl SpendVerifier for AcceptAllSpends {
    fn verify_spend(
        &self,
        _transaction: &Transaction,
        _input_index: usize,
        _prevout: &UtxoEntry,
    ) -> Result<(), UtxoError> {
        Ok(())
    }
}

pub fn chain_config() -> ChainConfig {
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

pub fn founder_block(config: &ChainConfig) -> Block {
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
            timestamp: config.genesis_timestamp + 300,
            difficulty_commitment: config.params.initial_target.to_le_bytes(),
            nonce: 801,
        },
        transactions,
    }
}

pub fn unknown_parent_block(config: &ChainConfig) -> Block {
    let mut block = founder_block(config);
    block.header.previous_block = Hash256::from_bytes([0x99; 32]);
    block
}

pub fn state_with_spendable_utxo(dir: &TestDir, config: &ChainConfig) -> (ChainState, OutPoint) {
    let state = ChainState::open(dir.path(), config.clone()).unwrap();
    drop(state);

    let outpoint = OutPoint {
        txid: Hash256::from_bytes([0x91; 32]),
        index: 0,
    };
    let entry = UtxoEntry {
        output: TxOutput {
            value: Amount::from_base_units(100_000).unwrap(),
            locking_program: vec![0x51],
        },
        creation_height: 0,
        is_coinbase: false,
    };
    let db = OregonDb::open(dir.path()).unwrap();
    let mut batch = StorageBatch::new();
    batch.put_utxo(outpoint, entry);
    db.commit_durable(batch).unwrap();
    drop(db);

    let state = ChainState::open(dir.path(), config.clone()).unwrap();
    assert!(state.utxos().get(&outpoint).is_some());
    (state, outpoint)
}

pub fn spend(outpoint: OutPoint, output_value: u64, marker: u8) -> Transaction {
    Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_txid: outpoint.txid,
            previous_output_index: outpoint.index,
            sequence: 0,
            witness: vec![],
        }],
        outputs: vec![TxOutput {
            value: Amount::from_base_units(output_value).unwrap(),
            locking_program: vec![marker],
        }],
        lock_time: 0,
    }
}

pub fn node_config() -> oregon_node::NodeConfig {
    oregon_node::NodeConfig {
        listen_addr: "127.0.0.1:0".parse().unwrap(),
        bootstrap_peers: Vec::new(),
        peer: PeerConfig::default(),
        mempool: MempoolConfig::default(),
    }
}

pub fn hello(chain_id: Hash256, nonce: u8) -> Hello {
    Hello {
        min_protocol_version: PROTOCOL_VERSION_MIN,
        max_protocol_version: PROTOCOL_VERSION_CURRENT,
        chain_id,
        instance_nonce: [nonce; 16],
        offered_features: FeatureSet::KNOWN,
        required_features: FeatureSet::HEADERS_SYNC,
        best_height: 0,
        best_block_id: chain_id,
    }
}

pub fn magic(chain_id: Hash256) -> [u8; 4] {
    network_magic(chain_id)
}
