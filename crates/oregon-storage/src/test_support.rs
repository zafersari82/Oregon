use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use oregon_primitives::{Amount, Hash256, OutPoint, TxOutput};
use oregon_utxo::{BlockUndo, UtxoEntry};
use rocksdb::{ColumnFamilyDescriptor, DB, Options};

use crate::db::{CF_BLOCK_INDEX, CF_BLOCKS, CF_CHAIN_META, CF_UNDO, CF_UTXO};

pub(crate) struct TestDir(PathBuf);

impl TestDir {
    pub(crate) fn new(label: &str) -> Self {
        Self::scoped("", label)
    }

    pub(crate) fn scoped(scope: &str, label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let prefix = if scope.is_empty() {
            "oregon".to_owned()
        } else {
            format!("oregon-{scope}")
        };
        let path = std::env::temp_dir().join(format!(
            "{prefix}-{label}-{}-{n}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn open_raw_existing(path: &Path) -> DB {
    let options = Options::default();
    let descriptors = [CF_BLOCKS, CF_BLOCK_INDEX, CF_UTXO, CF_UNDO, CF_CHAIN_META]
        .into_iter()
        .map(|name| ColumnFamilyDescriptor::new(name, Options::default()));
    DB::open_cf_descriptors(&options, path, descriptors).unwrap()
}

pub(crate) fn sample_utxo(value: u64) -> UtxoEntry {
    UtxoEntry {
        output: TxOutput {
            value: Amount::from_base_units(value).unwrap(),
            locking_program: vec![0x51],
        },
        creation_height: 7,
        is_coinbase: false,
    }
}

pub(crate) fn sample_sorted_undo() -> BlockUndo {
    BlockUndo {
        spent: vec![(
            OutPoint {
                txid: Hash256::from_bytes([0x11; 32]),
                index: 0,
            },
            sample_utxo(100),
        )],
        created: vec![OutPoint {
            txid: Hash256::from_bytes([0x22; 32]),
            index: 1,
        }],
    }
}
