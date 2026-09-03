# Oregon v1 M4 Persistent Chainstate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add crash-safe RocksDB-backed persistent blockchain and UTXO chainstate with deterministic restart, cumulative-work chain selection, atomic reorgs, and a frozen 8,064-block pruning/reorg window.

**Architecture:** Add `oregon-storage` as the only RocksDB-owning crate and `oregon-chainstate` as the chain-selection/state-orchestration crate. Existing M2 header/RandomX validation remains authoritative for PoW provenance, and existing M3 `UtxoState` remains authoritative for transaction/UTXO transitions; M4 stages all state changes in memory and publishes them only after a synchronous atomic RocksDB write succeeds.

**Tech Stack:** Rust 1.85.0 / edition 2024 workspace, `rocksdb = 0.24.0` with `default-features = false` and features `lz4,bindgen-runtime`, existing `thiserror`, `num-bigint`, Oregon primitives/consensus/pow/utxo crates, GitHub Actions with `cargo +1.85.0 ... --locked`.

**Spec:** `docs/superpowers/specs/2026-09-03-oregon-m4-persistent-chainstate-design.md`

## Global Constraints

- Work only on `oregon-v1-m4-persistent-chainstate` or explicitly named throwaway mutation branches; do not merge or modify `main`.
- Accepted base is M3 checkpoint commit `8f3ee9043b9cf3beb7b8e4653c0f2ab183233b71`.
- Rust toolchain remains exactly `1.85.0`; CI remains locked and warnings are denied.
- Pin `rocksdb` to exact crate version `=0.24.0`; do not move to 0.25.x because its MSRV is newer than this workspace.
- Use `rocksdb = { version = "=0.24.0", default-features = false, features = ["lz4", "bindgen-runtime"] }`; do not enable `multi-threaded-cf`.
- Named RocksDB column families are `blocks`, `block_index`, `utxo`, `undo`, and `chain_meta`; RocksDB `default` CF remains reserved and unused for Oregon records.
- Initial storage schema is exactly `1.0`.
- Active block/reorg acceptance uses WAL enabled and synchronous durability; no accepted result or in-memory active-state publication occurs before the durable batch reports success.
- Any acceptance write/sync error faults the current chainstate session; no further chain mutation is allowed before close/reopen recovery.
- Frozen rollback window is exactly `8_064` blocks: depth `8_064` allowed, depth `8_065` fail-closed to durable `ReindexRequired`.
- Pruning runs after durable acceptance as a separate idempotent maintenance write and may use `sync=false`.
- M2 RandomX key-block identity always comes from validated branch ancestry through `PowKeyBlockSource`; no candidate API accepts a raw key-block ID or arbitrary cumulative work.
- M3 `SpendVerifier` remains mandatory for every normal spend; no production permissive verifier is added.
- Storage encodings are explicit, versioned, bounded and deterministic. No authoritative `serde`/`bincode` persistence format.
- Block bodies are stored exactly as `Block::encode()` bytes.
- `block_index` is never pruned. Body/undo pruning may remove only data proven unnecessary under the frozen window.
- TDD is mandatory: write RED test, observe intended failure, implement minimal GREEN, run focused tests, then run fresh workspace CI before the task is accepted.
- Security mutations live only on throwaway branches and never enter the M4 development branch.

---

## File Structure

### Existing files modified

- `Cargo.toml` — add `oregon-storage` and `oregon-chainstate` workspace members.
- `Cargo.lock` — lock RocksDB 0.24.0 and native dependencies.
- `.github/workflows/oregon-rust.yml` — include M4 branch and install Clang/libclang for rust-rocksdb bindgen.
- `crates/oregon-consensus/src/work.rs` — canonical `ChainWork` storage bytes.
- `crates/oregon-utxo/src/error.rs` — duplicate persisted outpoint error.
- `crates/oregon-utxo/src/state.rs` — checked persisted-state reconstruction and read-only iteration.
- `crates/oregon-utxo/src/lib.rs` — export persistence bridge.

### New `oregon-storage` files

- `crates/oregon-storage/Cargo.toml` — RocksDB/Oregon dependencies and disabled-by-default `test-hooks` feature.
- `crates/oregon-storage/src/lib.rs` — public storage exports.
- `crates/oregon-storage/src/error.rs` — storage errors.
- `crates/oregon-storage/src/codec.rs` — deterministic bounded codecs and 36-byte outpoint keys.
- `crates/oregon-storage/src/records.rs` — block-index and metadata record types/codecs.
- `crates/oregon-storage/src/schema.rs` — schema 1.0 and restart-resumable migration engine.
- `crates/oregon-storage/src/batch.rs` — typed batch plan and durability mode.
- `crates/oregon-storage/src/db.rs` — RocksDB ownership, typed reads and writes.
- `crates/oregon-storage/src/tests.rs` — storage tests.

### New `oregon-chainstate` files

- `crates/oregon-chainstate/Cargo.toml` — consensus/pow/primitives/storage/utxo dependencies.
- `crates/oregon-chainstate/src/lib.rs` — public chainstate exports.
- `crates/oregon-chainstate/src/error.rs` — chainstate/session errors.
- `crates/oregon-chainstate/src/config.rs` — trusted height-0 anchor and consensus configuration.
- `crates/oregon-chainstate/src/branch.rs` — branch ancestry, MTP and `PowKeyBlockSource`.
- `crates/oregon-chainstate/src/state.rs` — bootstrap/reopen/block admission/direct extension.
- `crates/oregon-chainstate/src/reorg.rs` — fork/reorg staging and atomic publication.
- `crates/oregon-chainstate/src/prune.rs` — pruning predicate and maintenance batch.
- `crates/oregon-chainstate/src/tests.rs` — chainstate integration tests.

## Test Fixture Contracts

The following helpers are test-only and must never be exported in production APIs.

`TestDir` is implemented independently in each crate test module without adding a tempfile dependency:

```rust
struct TestDir(std::path::PathBuf);

impl TestDir {
    fn new(label: &str) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "oregon-{label}-{}-{n}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
```

Chainstate tests use only this test verifier:

```rust
struct AcceptTestSpends;

impl SpendVerifier for AcceptTestSpends {
    fn verify_spend(
        &self,
        _transaction: &Transaction,
        _input_index: usize,
        _prevout: &UtxoEntry,
    ) -> Result<(), UtxoError> {
        Ok(())
    }
}
```

`AcceptTestSpends` stays inside `#[cfg(test)]`; M4 must not add an equivalent production verifier.

The standard test consensus configuration uses maximum target so every correctly keyed RandomX hash meets target:

```rust
fn test_params() -> ConsensusParams {
    let max = Target::from_le_bytes([0xff; 32]).unwrap();
    ConsensusParams::new(max, max, [0x42; 32]).unwrap()
}
```

The test anchor is height 0 only and carries no caller-supplied work:

```rust
fn test_anchor(genesis_timestamp: u64) -> BlockHeader {
    BlockHeader {
        version: 1,
        previous_block: Hash256::from_bytes([0u8; 32]),
        transaction_root: Hash256::from_bytes([0x11; 32]),
        timestamp: genesis_timestamp,
        difficulty_commitment: [0xff; 32],
        nonce: 0,
    }
}
```

Coinbase fixtures copy the already-frozen consensus form: one null outpoint input, `sequence=u32::MAX`, first witness item is canonical `write_varint(height)`, height 1 output 0 is exactly `FOUNDER_ALLOCATION_BASE_UNITS` to `[KEY_COMMIT_V1 || founder_key_commitment]`; later heights claim no more than `block_subsidy(height) + fees`.

---

### Task 1: Persistence-Safe M2/M3 Value Bridges and M4 CI Gate

**Files:**
- Modify: `.github/workflows/oregon-rust.yml`
- Modify: `crates/oregon-consensus/src/work.rs`
- Modify: `crates/oregon-utxo/src/error.rs`
- Modify: `crates/oregon-utxo/src/state.rs`
- Modify: `crates/oregon-utxo/src/lib.rs`

**Interfaces:**
- Produces: `ChainWork::to_canonical_be_bytes() -> Vec<u8>`
- Produces: `ChainWork::from_canonical_be_bytes(bytes: &[u8]) -> Option<ChainWork>`
- Produces: `UtxoState::from_persisted_entries<I>(entries: I) -> Result<UtxoState, UtxoError>` where `I: IntoIterator<Item = (OutPoint, UtxoEntry)>`
- Produces: `UtxoState::entries(&self) -> impl Iterator<Item = (&OutPoint, &UtxoEntry)>`
- Produces: `UtxoError::DuplicatePersistedOutpoint(OutPoint)`

- [ ] **Step 1: Add M4 branch to push CI before the RED commit**

Preserve all existing branches and pinned checkout SHA; append `oregon-v1-m4-persistent-chainstate`.

- [ ] **Step 2: Write failing `ChainWork` canonical tests**

```rust
#[test]
fn chainwork_canonical_storage_bytes_round_trip() {
    let zero = ChainWork::zero();
    assert_eq!(zero.to_canonical_be_bytes(), vec![0]);
    assert_eq!(ChainWork::from_canonical_be_bytes(&[0]).unwrap(), zero);

    let work = block_work(Target::from_biguint(&BigUint::from(1u8)).unwrap());
    let encoded = work.to_canonical_be_bytes();
    assert_eq!(ChainWork::from_canonical_be_bytes(&encoded).unwrap(), work);
    assert_eq!(ChainWork::from_canonical_be_bytes(&[]), None);
    assert_eq!(ChainWork::from_canonical_be_bytes(&[0, 1]), None);
}
```

- [ ] **Step 3: Write failing UTXO restoration tests**

```rust
#[test]
fn persisted_utxo_reconstruction_rejects_duplicate_outpoints() {
    let point = outpoint(0x44, 0);
    let result = UtxoState::from_persisted_entries([
        (point, entry(100)),
        (point, entry(100)),
    ]);
    assert_eq!(result, Err(UtxoError::DuplicatePersistedOutpoint(point)));
}

#[test]
fn restored_state_still_requires_spend_verifier() {
    let point = outpoint(0x45, 0);
    let mut state = UtxoState::from_persisted_entries([(point, entry(100))]).unwrap();
    let tx = spend(vec![point], &[90]);
    assert_eq!(
        state.apply_normal_transaction(&tx, 2, &RejectAll),
        Err(UtxoError::SpendAuthorizationFailed)
    );
    assert!(state.get(&point).is_some());
}
```

- [ ] **Step 4: Push RED and record intended CI failure**

Expected: missing canonical methods and persisted UTXO API/error. Reject a RED caused by workflow syntax or unrelated tooling.

- [ ] **Step 5: Implement minimal GREEN**

`to_canonical_be_bytes`: `BigUint::to_bytes_be()`, mapping empty zero to `[0]`. `from_canonical_be_bytes`: reject empty input and any length > 1 with byte 0 equal to zero, then construct the internal `BigUint`.

`from_persisted_entries`: insert into a fresh `HashMap`, return `DuplicatePersistedOutpoint` on second insert. `entries`: return `self.entries.iter()` only.

- [ ] **Step 6: Verify**

```bash
cargo +1.85.0 test --locked -p oregon-consensus chainwork_canonical_storage_bytes_round_trip
cargo +1.85.0 test --locked -p oregon-utxo persisted_utxo_reconstruction_rejects_duplicate_outpoints
cargo +1.85.0 test --locked -p oregon-utxo restored_state_still_requires_spend_verifier
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

- [ ] **Step 7: Commit GREEN and require fresh CI**

Commit: `feat: add persistence-safe chainwork and utxo bridges`.

---

### Task 2: RocksDB Storage Crate, Column Families, and Schema 1.0

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `.github/workflows/oregon-rust.yml`
- Create: `crates/oregon-storage/Cargo.toml`
- Create: `crates/oregon-storage/src/lib.rs`
- Create: `crates/oregon-storage/src/error.rs`
- Create: `crates/oregon-storage/src/schema.rs`
- Create: `crates/oregon-storage/src/db.rs`
- Create: `crates/oregon-storage/src/tests.rs`

**Interfaces:**
- `CF_BLOCKS`, `CF_BLOCK_INDEX`, `CF_UTXO`, `CF_UNDO`, `CF_CHAIN_META`
- `SchemaVersion { major: u16, minor: u16 }`, `SCHEMA_VERSION = SchemaVersion { major: 1, minor: 0 }`
- `OregonDb::open(path: impl AsRef<Path>) -> Result<OregonDb, StorageError>`
- `OregonDb::schema_version(&self) -> Result<SchemaVersion, StorageError>`
- Test-only `OregonDb::has_column_family(&self, name: &str) -> bool`
- `StorageError::{RocksDb, CorruptData, UnsupportedSchema, DurabilityFailure}`

- [ ] **Step 1: Add exact crate manifest and lock**

```toml
[package]
name = "oregon-storage"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true

[features]
default = []
test-hooks = []

[dependencies]
oregon-consensus = { path = "../oregon-consensus" }
oregon-primitives = { path = "../oregon-primitives" }
oregon-utxo = { path = "../oregon-utxo" }
rocksdb = { version = "=0.24.0", default-features = false, features = ["lz4", "bindgen-runtime"] }
thiserror = "2"
```

Add workspace member and regenerate `Cargo.lock` under Rust 1.85.0 before RED push so `--locked` does not mask the behavior test.

- [ ] **Step 2: Add CI native prerequisites**

Add an Ubuntu step before Test:

```yaml
- name: Install RocksDB build prerequisites
  run: sudo apt-get update && sudo apt-get install -y clang libclang-dev
```

Do not change permissions or checkout pin.

- [ ] **Step 3: Write RED open/schema test**

```rust
#[test]
fn new_database_opens_with_schema_1_0_and_required_column_families() {
    let dir = TestDir::new("schema-open");
    let db = OregonDb::open(dir.path()).unwrap();
    assert_eq!(db.schema_version().unwrap(), SchemaVersion { major: 1, minor: 0 });
    for name in [CF_BLOCKS, CF_BLOCK_INDEX, CF_UTXO, CF_UNDO, CF_CHAIN_META] {
        assert!(db.has_column_family(name));
    }
    drop(db);
    let reopened = OregonDb::open(dir.path()).unwrap();
    assert_eq!(reopened.schema_version().unwrap(), SchemaVersion { major: 1, minor: 0 });
}
```

- [ ] **Step 4: Push RED**

Expected: missing storage open/schema APIs, not lock/toolchain failure.

- [ ] **Step 5: Implement open/schema**

Use `DB::open_cf_descriptors` with `create_if_missing(true)` and `create_missing_column_families(true)`. Schema key is `b"schema/version"`; value is exactly `major.to_be_bytes() || minor.to_be_bytes()`. Brand-new schema write uses WAL and `WriteOptions::set_sync(true)`. Existing major other than 1 or minor greater than 0 returns `UnsupportedSchema` without rewriting.

- [ ] **Step 6: Verify and commit**

```bash
cargo +1.85.0 test --locked -p oregon-storage new_database_opens_with_schema_1_0_and_required_column_families
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

Commit: `feat: add rocksdb storage foundation`.

---

### Task 3: Deterministic Storage Record Codecs

**Files:**
- Create: `crates/oregon-storage/src/codec.rs`
- Create: `crates/oregon-storage/src/records.rs`
- Modify: `crates/oregon-storage/src/lib.rs`
- Modify: `crates/oregon-storage/src/tests.rs`

**Interfaces:**
- `encode_outpoint_key(&OutPoint) -> [u8; 36]`
- `decode_outpoint_key(&[u8]) -> Result<OutPoint, StorageError>`
- `encode_utxo_entry` / `decode_utxo_entry`
- `encode_block_undo` / `decode_block_undo`
- `ValidationStatus::{HeaderValidated, FullyValidated, Invalid}`
- `BlockIndexRecord { header: BlockHeader, parent: Hash256, height: u64, cumulative_work: ChainWork, validation: ValidationStatus, body_retained: bool }`
- `encode_block_index` / `decode_block_index`
- `NodeHealth::{Healthy, ReindexRequired}` plus exact metadata codecs.

- [ ] **Step 1: Add storage codec test helper**

```rust
fn sample_sorted_undo() -> BlockUndo {
    let first = OutPoint { txid: Hash256::from_bytes([0x11; 32]), index: 0 };
    let second = OutPoint { txid: Hash256::from_bytes([0x22; 32]), index: 1 };
    BlockUndo {
        spent: vec![(first, sample_utxo(100))],
        created: vec![second],
    }
}
```

`sample_utxo(value)` creates `UtxoEntry { output: TxOutput { value: Amount::from_base_units(value).unwrap(), locking_program: vec![0x51] }, creation_height: 7, is_coinbase: false }`.

- [ ] **Step 2: Write RED codec tests**

```rust
#[test]
fn outpoint_key_is_exactly_36_bytes_and_little_endian_indexed() {
    let point = OutPoint { txid: Hash256::from_bytes([0x11; 32]), index: 0x0102_0304 };
    let key = encode_outpoint_key(&point);
    assert_eq!(&key[..32], &[0x11; 32]);
    assert_eq!(&key[32..], &[0x04, 0x03, 0x02, 0x01]);
    assert_eq!(decode_outpoint_key(&key).unwrap(), point);
}

#[test]
fn block_undo_encoding_is_deterministic_and_rejects_trailing_bytes() {
    let undo = sample_sorted_undo();
    let first = encode_block_undo(&undo);
    assert_eq!(encode_block_undo(&undo), first);
    assert_eq!(decode_block_undo(&first).unwrap(), undo);
    let mut tainted = first;
    tainted.push(0);
    assert!(matches!(decode_block_undo(&tainted), Err(StorageError::CorruptData(_))));
}
```

Also require UTXO locking-program length 65,536 accepted and 65,537 rejected, duplicate/non-strictly-sorted undo outpoints rejected, unknown record version rejected, truncation rejected, and non-minimal chainwork bytes rejected.

- [ ] **Step 3: Push RED**

- [ ] **Step 4: Implement bounded cursor and layouts**

`StorageCursor` exposes exact integer reads, canonical varint `read_len(max)`, `read_exact` and `finish`. Reuse `oregon_primitives::write_varint` for length output.

UTXO layout:

```text
version:u8 | value:u64 LE | creation_height:u64 LE | is_coinbase:u8 |
locking_program_len:canonical_varint | locking_program
```

Undo layout:

```text
version:u8 | spent_count:varint |
repeated(outpoint:36 | utxo_len:varint | utxo) |
created_count:varint | repeated(outpoint:36)
```

Version is 1. Undo decoder requires strict ascending canonical outpoint-key bytes in both collections.

- [ ] **Step 5: Implement block-index layout**

```text
version:u8 | header:114 | parent:32 | height:u64 LE |
chainwork_len:varint | canonical_chainwork_be |
validation:u8 | body_retained:u8
```

Require decoded `parent == header.previous_block`. Store full header forever so body pruning cannot remove MTP/key-ancestry data.

- [ ] **Step 6: Implement metadata keys**

Use fixed keys:

```text
schema/version
schema/migration
config/anchor_id
config/genesis_timestamp
active/tip_id
active/tip_height
health/state
prune/cursor
active/<height:8-byte BE>
```

- [ ] **Step 7: Verify and commit**

```bash
cargo +1.85.0 test --locked -p oregon-storage codec
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

Commit: `feat: add deterministic chainstate storage codecs`.

---

### Task 4: Typed Reads, Atomic Batches, Durability Modes, and Migration Runner

**Files:**
- Create: `crates/oregon-storage/src/batch.rs`
- Modify: `crates/oregon-storage/src/db.rs`
- Modify: `crates/oregon-storage/src/schema.rs`
- Modify: `crates/oregon-storage/src/lib.rs`
- Modify: `crates/oregon-storage/src/tests.rs`

**Interfaces:**
- `StorageBatch::new()`; typed methods `put_block`, `delete_block`, `put_index`, `put_utxo`, `delete_utxo`, `put_undo`, `delete_undo`, `set_active_height`, `delete_active_height`, `set_tip`, `set_health`, `set_prune_cursor`.
- `DurabilityMode::{Sync, NoSync}`.
- `OregonDb::commit_durable(batch)` always maps to Sync.
- `OregonDb::commit_maintenance(batch)` always maps to NoSync.
- Typed getters: `get_block`, `get_index`, `get_utxo`, `iter_utxos`, `get_undo`, `active_id_at_height`, `active_tip`, `health`, `iter_body_retained_indices`.
- `test-hooks` exposes only `open_with_test_hooks`, `fail_next_durable_write`, `fail_next_maintenance_write`, and `last_mode` under `cfg(any(test, feature = "test-hooks"))`.

- [ ] **Step 1: Write RED atomic typed round-trip test**

Persist sample block/index/UTXO/undo/active mapping/tip in one durable batch, close/reopen and require every value to decode identically.

- [ ] **Step 2: Write RED durability/failure tests**

```rust
#[test]
fn durable_commit_requests_sync_and_maintenance_does_not() {
    let dir = TestDir::new("durability-mode");
    let db = OregonDb::open_with_test_hooks(dir.path()).unwrap();
    db.commit_durable(StorageBatch::new()).unwrap();
    assert_eq!(db.test_hooks().last_mode(), Some(DurabilityMode::Sync));
    db.commit_maintenance(StorageBatch::new()).unwrap();
    assert_eq!(db.test_hooks().last_mode(), Some(DurabilityMode::NoSync));
}
```

Injected durable failure occurs before `db.write_opt`, writes nothing, and returns `DurabilityFailure`.

- [ ] **Step 3: Push RED**

- [ ] **Step 4: Implement `StorageBatch`**

Store typed operations in a private enum. Encode every operation before constructing/executing the RocksDB `WriteBatch`; codec error returns before RocksDB mutation.

- [ ] **Step 5: Implement durability mapping**

`commit_durable`: WAL remains enabled, `WriteOptions::set_sync(true)`, `db.write_opt`. `commit_maintenance`: WAL remains enabled, `set_sync(false)`. Never call `disable_wal(true)`.

- [ ] **Step 6: Implement typed reads with identity checks**

`get_block(block_id)` decodes with `Block::decode(..., &DecodeLimits::default())` and requires `block.header.block_id() == block_id`; mismatch is `CorruptData`. `get_index(block_id)` requires decoded header ID equals the key block ID and parent consistency.

- [ ] **Step 7: Implement/test migration runner**

Production current schema remains 1.0. Private migration engine is tested with synthetic target 1.1: write sync migration marker first, interrupt after first idempotent step, rerun resumes/repeats deterministically, then clears marker. Unknown major 2.0 returns `UnsupportedSchema` and leaves DB unchanged.

- [ ] **Step 8: Verify and commit**

```bash
cargo +1.85.0 test --locked -p oregon-storage
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

Commit: `feat: add atomic durable rocksdb batches`.

---

### Task 5: Chainstate Bootstrap, Reopen Validation, and UTXO Reconstruction

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/oregon-chainstate/Cargo.toml`
- Create: `crates/oregon-chainstate/src/lib.rs`
- Create: `crates/oregon-chainstate/src/error.rs`
- Create: `crates/oregon-chainstate/src/config.rs`
- Create: `crates/oregon-chainstate/src/state.rs`
- Create: `crates/oregon-chainstate/src/tests.rs`

**Interfaces:**
- `ChainConfig { anchor_header: BlockHeader, genesis_timestamp: u64, params: ConsensusParams }`
- `Tip { block_id: Hash256, height: u64, cumulative_work: ChainWork }`
- `SessionHealth::{Healthy, StorageFaulted, ReindexRequired}`
- `ChainState::open(path, config) -> Result<ChainState, ChainStateError>`
- `ChainState::tip() -> &Tip`, `ChainState::utxos() -> &UtxoState`, `ChainState::session_health() -> SessionHealth`.
- Height-0 anchor cumulative work is always `ChainWork::zero()`; no config/caller field supplies work.

- [ ] **Step 1: Add exact crate manifest**

```toml
[package]
name = "oregon-chainstate"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true

[dependencies]
oregon-consensus = { path = "../oregon-consensus" }
oregon-pow = { path = "../oregon-pow" }
oregon-primitives = { path = "../oregon-primitives" }
oregon-storage = { path = "../oregon-storage" }
oregon-utxo = { path = "../oregon-utxo" }
thiserror = "2"

[dev-dependencies]
oregon-storage = { path = "../oregon-storage", features = ["test-hooks"] }
```

- [ ] **Step 2: Write RED bootstrap/reopen tests**

Open new DB with `ChainConfig { anchor_header: test_anchor(g), genesis_timestamp: g, params: test_params() }`. Require tip height 0, anchor ID, zero work and empty UTXO. Close/reopen must match. Different anchor ID or genesis timestamp must fail closed.

- [ ] **Step 3: Push RED**

- [ ] **Step 4: Implement bootstrap durable batch**

Write anchor index height 0, zero work, `FullyValidated`, `body_retained=false`, active map 0, tip 0, anchor ID, genesis timestamp, `Healthy`, prune cursor 0 in one sync batch.

- [ ] **Step 5: Implement reopen validation**

For active heights 0..=tip:

1. active mapping exists;
2. index exists and decoded header ID equals mapped block ID;
3. index height equals mapping height;
4. for height > 0, index parent equals previous active ID;
5. parse header target and recompute `expected_work = previous.cumulative_work + block_work(target)`; require exact equality to stored cumulative work;
6. height 0 cumulative work is exactly zero.

For current active retained range greater than height 0, require body and undo records. Decode every UTXO and reconstruct with `UtxoState::from_persisted_entries`.

- [ ] **Step 6: Add corruption RED/GREEN tests**

Tamper cumulative work only, delete active mapping, delete retained undo, corrupt UTXO bytes; every case must fail closed. Body missing behind valid prune horizon is allowed.

- [ ] **Step 7: Verify and commit**

```bash
cargo +1.85.0 test --locked -p oregon-chainstate bootstrap
cargo +1.85.0 test --locked -p oregon-chainstate reopen
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

Commit: `feat: add persistent chainstate bootstrap and reopen`.

---

### Task 6: Branch-Aware Header/RandomX Validation and Side-Chain Admission

**Files:**
- Create: `crates/oregon-chainstate/src/branch.rs`
- Modify: `crates/oregon-chainstate/src/state.rs`
- Modify: `crates/oregon-chainstate/src/lib.rs`
- Modify: `crates/oregon-chainstate/src/tests.rs`

**Interfaces:**
- Private `BranchView<'a> { db: &'a OregonDb, tip: Hash256 }`.
- `BranchView::ancestor_id_at_height(height) -> Result<Option<Hash256>, ChainStateError>`.
- `BranchView::mtp_window() -> Result<Vec<u64>, ChainStateError>` returns candidate-parent branch's latest 1..=11 timestamps.
- `BranchView` implements `PowKeyBlockSource` from validated ancestry only.
- `AcceptOutcome::{Extended, StoredSideChain, Reorganized}`.
- `ChainState::accept_block<V: SpendVerifier>(&mut self, block: Block, verifier: &V) -> Result<AcceptOutcome, ChainStateError>`; no height/work/key-ID parameters.

- [ ] **Step 1: Write RED branch ancestry/MTP tests**

Require exact candidate-branch ancestor lookup and max 11 timestamps.

- [ ] **Step 2: Write RED key-provenance tests**

Build a candidate whose scheduled RandomX key-block belongs to candidate ancestry. Require that ancestor ID to be used. A wrong-key engine remains rejected by M2 semantics; no alternate public API may inject a key ID.

- [ ] **Step 3: Push RED**

- [ ] **Step 4: Implement checked parent/index loading**

Before trusting a parent index, require header ID equals index key, parent linkage to its own parent index, `height == parent.height + 1`, and `cumulative_work == parent.cumulative_work + block_work(header_target)` for non-anchor records. Reject `Invalid` parents.

- [ ] **Step 5: Derive candidate facts from ancestry only**

Compute height via checked add, collect MTP, call `validate_header_pre_pow`, compute scheduled key height with `oregon_pow::key_block_height(facts.height())`, obtain ancestor ID from `BranchView`, derive key, construct `LightEngine`, call `validate_header_pow`, then cumulative work = parent work + `facts.work()`.

- [ ] **Step 6: Persist header-only side blocks durably**

If not direct active-tip extension and cumulative work does not exceed active tip, sync-write body plus `HeaderValidated` index with `body_retained=true`; no UTXO/active mapping mutation.

Known block ID is idempotently returned without rewriting work. Descendant of `Invalid` parent is rejected.

- [ ] **Step 7: Verify and commit**

```bash
cargo +1.85.0 test --locked -p oregon-chainstate branch
cargo +1.85.0 test --locked -p oregon-chainstate side_chain
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

Commit: `feat: add branch-aware pow chain indexing`.

---

### Task 7: Atomic Direct Active Extension and Storage-Fault Semantics

**Files:**
- Modify: `crates/oregon-chainstate/src/state.rs`
- Modify: `crates/oregon-chainstate/src/error.rs`
- Modify: `crates/oregon-chainstate/src/tests.rs`

**Interfaces:**
- Private delta type is exactly `BTreeMap<[u8; 36], (OutPoint, Option<UtxoEntry>)>`; key is `oregon_storage::encode_outpoint_key(&outpoint)`.
- `Some(entry)` means final insert/update, `None` means final delete.
- Session mutation guard returns `ChainStateError::StorageFaulted` or `ReindexRequired` before any validation/write when session is non-Healthy.

- [ ] **Step 1: Write RED direct-extension reopen test**

Use valid height-1 founder block, accept with `AcceptTestSpends`, close/reopen, require same tip and founder UTXO entries.

- [ ] **Step 2: Write RED durable failure test**

Inject failure before durable acceptance. Require old tip/UTXO remain in memory, session becomes `StorageFaulted`, and second mutation is rejected. Reopen validates actual durable DB before returning Healthy.

- [ ] **Step 3: Push RED**

- [ ] **Step 4: Implement staged extension**

Clone UTXO; call M3 `connect_block`; build delta from `BlockUndo.spent` -> `None` and each `BlockUndo.created` -> `Some(staged.get(outpoint).unwrap().clone())` keyed by exact 36-byte key. Build one sync batch with body, `FullyValidated` index, undo, UTXO delta, active mapping and tip.

- [ ] **Step 5: Publish only after durable success**

Success replaces in-memory UTXO/tip. Any storage error keeps old memory, sets `StorageFaulted`, returns storage/durability error and blocks later mutations.

- [ ] **Step 6: Verify and commit**

```bash
cargo +1.85.0 test --locked -p oregon-chainstate direct_extension
cargo +1.85.0 test --locked -p oregon-chainstate storage_fault
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

Commit: `feat: persist active chain extensions atomically`.

---

### Task 8: Fully Staged Reorgs and Exact 8,064/8,065 Boundary

**Files:**
- Create: `crates/oregon-chainstate/src/reorg.rs`
- Modify: `crates/oregon-chainstate/src/state.rs`
- Modify: `crates/oregon-chainstate/src/error.rs`
- Modify: `crates/oregon-chainstate/src/tests.rs`

**Interfaces:**
- `pub const REORG_WINDOW: u64 = 8_064`.
- Private `reorg_depth_allowed(depth: u64) -> bool { depth <= REORG_WINDOW }`.
- Same exact ordered delta type as Task 7: `BTreeMap<[u8; 36], (OutPoint, Option<UtxoEntry>)>`.

- [ ] **Step 1: Write RED exact boundary test**

```rust
#[test]
fn reorg_window_accepts_8064_and_rejects_8065() {
    assert!(reorg_depth_allowed(8_064));
    assert!(!reorg_depth_allowed(8_065));
}
```

Use synthetic index fixtures to prove 8,065 writes only durable `ReindexRequired` and leaves active tip/UTXO/mapping unchanged.

- [ ] **Step 2: Write RED invalid-final-candidate atomicity test**

Candidate final block is transaction-invalid. Require old active state unchanged and candidate path from first failing block through attempted tip marked `Invalid`.

- [ ] **Step 3: Write RED valid reorg/reopen test**

Strictly greater-work candidate within window disconnects old active, connects candidate, commits once, reopens at candidate tip.

- [ ] **Step 4: Push RED**

- [ ] **Step 5: Implement preflight**

Find common fork; gather old active IDs tip->fork with every undo; gather candidate IDs fork-child->tip with every body. Missing retained data returns `MissingUndo`/`MissingBlockBody` with no mutation.

- [ ] **Step 6: Implement deep-reorg fail-closed**

If depth > 8,064, sync-write only `NodeHealth::ReindexRequired`, set session health `ReindexRequired`, leave active state unchanged.

- [ ] **Step 7: Implement staged disconnect/connect**

Clone current UTXO. For old undo tip->fork: call M3 disconnect, record `undo.created -> None`, `undo.spent -> Some(entry)`. For candidate blocks forward: M3 connect, record new undo `spent -> None`, surviving `created -> Some(staged entry)`. Later map writes overwrite earlier entries for same 36-byte key, producing final delta relative to current durable UTXO without full-set comparison.

- [ ] **Step 8: Handle invalid candidate without partial publication**

On first M3 candidate failure, do not persist active state. Sync-write index status `Invalid` for failing block and its candidate descendants through attempted tip. Return consensus error.

- [ ] **Step 9: Commit valid reorg once**

Single sync batch contains final UTXO delta, regenerated undo for new active blocks, active-height deletes/puts, new tip, candidate index upgrades to `FullyValidated`, and received block/index if not already stored. Publish memory only after success; storage error faults session.

- [ ] **Step 10: Verify and commit**

```bash
cargo +1.85.0 test --locked -p oregon-chainstate reorg_window_accepts_8064_and_rejects_8065
cargo +1.85.0 test --locked -p oregon-chainstate invalid_final_candidate
cargo +1.85.0 test --locked -p oregon-chainstate valid_reorg
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

Commit: `feat: add atomic cumulative-work reorgs`.

---

### Task 9: Safe Idempotent Pruning

**Files:**
- Create: `crates/oregon-chainstate/src/prune.rs`
- Modify: `crates/oregon-chainstate/src/state.rs`
- Modify: `crates/oregon-chainstate/src/tests.rs`
- Modify: `crates/oregon-storage/src/batch.rs`

**Interfaces:**
- `retained_active_floor(height: u64) -> u64 { height.saturating_sub(REORG_WINDOW - 1) }`.
- `ChainState::prune() -> Result<PruneReport, ChainStateError>`.
- `PruneReport { deleted_bodies: u64, deleted_undos: u64 }`.

- [ ] **Step 1: Write RED active-floor boundary tests**

At tip H, active body+undo at `H-8063` retained; height `H-8064` eligible. Early heights saturate to 0.

- [ ] **Step 2: Write RED side-body predicate tests**

A body is retained when height >= active floor and common-fork disconnect depth <= 8,064. A side body is eligible when its common fork depth > 8,064. Any body below active floor is eligible.

- [ ] **Step 3: Write RED undo predicate tests**

Undo retained only for current active blocks inside active window. Side undo may be deleted because reconnect regenerates undo before that block can become active.

- [ ] **Step 4: Push RED**

- [ ] **Step 5: Implement separate maintenance plan**

Scan `body_retained` indices. Evaluate exact predicate, delete eligible block body and set `body_retained=false` in unpruned index. Delete undo not belonging to current active retained window. Set prune cursor to current active height. Never alter UTXO, active mapping, tip, chainwork or validation status.

- [ ] **Step 6: Commit with `commit_maintenance`**

Use NoSync. Repeated `prune()` converges to same retained set.

- [ ] **Step 7: Write interrupted pruning test**

Injected pre-maintenance failure changes no consensus state. Reopen and rerun reaches same retained set as uninterrupted prune.

- [ ] **Step 8: Verify and commit**

```bash
cargo +1.85.0 test --locked -p oregon-chainstate prune
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

Commit: `feat: add safe idempotent chain pruning`.

---

### Task 10: Restart/Corruption/Crash Acceptance Matrix

**Files:**
- Modify: `crates/oregon-storage/src/tests.rs`
- Modify: `crates/oregon-chainstate/src/tests.rs`

**Interfaces:**
- No new production API unless a failing acceptance test proves a missing invariant; such a fix starts a new RED -> GREEN cycle before continuing.

- [ ] **Step 1: Close/reopen state equivalence**

Connect multiple blocks including spends, reopen, compare tip and UTXOs after sorting by `encode_outpoint_key`.

- [ ] **Step 2: Accepted-before-prune crash boundary**

Durably accept, fail/skip pruning, close immediately, reopen accepted state with harmless extra old data.

- [ ] **Step 3: Retained-window corruption**

Delete/tamper active body/undo inside window -> fail closed. Missing body behind valid prune horizon -> healthy.

- [ ] **Step 4: Index/cumulative-work corruption**

Tamper block-index cumulative work without changing header -> reopen/link validation must detect mismatch. Header/key mismatch and parent/height mismatch also fail closed.

- [ ] **Step 5: Equal/lower-work candidates**

Remain side chain; do not alter UTXO/active mapping.

- [ ] **Step 6: Deterministic bytes**

Construct logically identical UTXO/undo data from opposite insertion orders; sort canonically and require identical persisted bytes.

- [ ] **Step 7: Migration crash matrix**

Synthetic supported minor migration interruption before marker, after marker, and after first idempotent step converges. Unknown major is unchanged and fails closed.

- [ ] **Step 8: Full pre-mutation gate**

```bash
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

Record exact clean commit SHA and workflow run ID.

- [ ] **Step 9: Commit tests**

Commit: `test: harden persistent chainstate recovery boundaries`.

---

### Task 11: Required M4 Security Mutations

**Files:**
- Throwaway branches only.

**Interfaces:**
- Mutation A killed by `reorg_window_accepts_8064_and_rejects_8065`.
- Mutation B killed by durability/fault tests.
- Mutation C killed by invalid-candidate atomicity tests.

- [ ] **Step 1: Mutation A**

Create `mutation-m4-reorg-off-by-one-2026-09-03`. Change allowed comparison to deliberately permit 8,065. Trigger CI on throwaway branch if needed. Expected exact boundary test failure.

- [ ] **Step 2: Mutation B**

Create `mutation-m4-durability-sync-2026-09-03`. Deliberately route active acceptance through `DurabilityMode::NoSync` or publish active memory before durable return. Expected durability-mode/direct-extension failure.

- [ ] **Step 3: Mutation C**

Create `mutation-m4-early-reorg-publication-2026-09-03`. Deliberately publish staged UTXO/tip before final candidate validation. Expected invalid-final-candidate test failure.

- [ ] **Step 4: Record evidence**

For each: branch, mutation commit, CI-trigger commit when used, run ID, failed job, exact intended failing test and symptom. Confirm mutation never enters M4 branch.

- [ ] **Step 5: Fresh post-mutation clean gate**

Return to clean M4 branch. Create a tree-equivalent empty commit only if needed to trigger fresh CI. Require Test, Format, Clippy success at reviewed clean source.

---

### Task 12: M3 -> M4 Security Review and Accepted Checkpoint

**Files:**
- Create: `docs/checkpoints/OREGON_V1_M4_PERSISTENT_CHAINSTATE.md`
- Production files change only if review finds a defect; every defect gets a new RED -> GREEN cycle and new clean-gate evidence.

**Interfaces:**
- Accepted recovery branch: `oregon-v1-checkpoint-m4-persistent-chainstate-accepted-2026-09-03`.

- [ ] **Step 1: Compare accepted M3 base to final M4 code head**

Base: `8f3ee9043b9cf3beb7b8e4653c0f2ab183233b71`. Enumerate changed files; confirm no unrelated refactor or secrets.

- [ ] **Step 2: Manual critical-boundary review**

Verify source proves: sync WAL active writes; memory publication after durable success only; StorageFaulted fail-stop; atomic batch coverage; validated ChainWork provenance; branch-derived RandomX key ID; mandatory M3 verifier; unchanged founder maturity and checked amount arithmetic; exact 8,064/8,065 boundary; safe active/side pruning; canonical/truncation-resistant codecs; duplicate-free restart UTXO reconstruction; restart-resumable migration; no consensus-visible `HashMap` ordering.

- [ ] **Step 3: Require no open Critical/Important finding**

Any finding stops checkpointing, gets regression RED, minimal GREEN, full CI and renewed review.

- [ ] **Step 4: Create checkpoint document**

Record frozen behavior, reviewed code commit, RocksDB 0.24.0 pin, schema 1.0, pre/post-mutation CI, all mutation evidence, review disposition and exclusions.

- [ ] **Step 5: Run CI on checkpoint commit**

Require Test, Format and Clippy success.

- [ ] **Step 6: Create recovery branch**

Create `oregon-v1-checkpoint-m4-persistent-chainstate-accepted-2026-09-03` at verified checkpoint commit. Do not merge `main`.

- [ ] **Step 7: Report acceptance with exact evidence**

Report reviewed code SHA, checkpoint SHA, clean CI run IDs, mutation runs, recovery branch and explicit remaining exclusions. Do not claim full node or mainnet readiness.
