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
- Pin `rocksdb` to exact crate version `=0.24.0`; 0.24.0 declares Rust 1.85.0 support. Do not move to 0.25.x because its MSRV is newer than this workspace.
- Use `rocksdb = { version = "=0.24.0", default-features = false, features = ["lz4", "bindgen-runtime"] }`; do not enable `multi-threaded-cf` because M4 uses one chainstate writer and does not mutate column families concurrently.
- Named RocksDB column families are `blocks`, `block_index`, `utxo`, `undo`, and `chain_meta`; the mandatory RocksDB `default` CF is reserved and unused for Oregon records.
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
- `Cargo.lock` — lock RocksDB 0.24.0 and its transitive native dependencies.
- `.github/workflows/oregon-rust.yml` — include M4 branch in push CI and install the native Clang/libclang prerequisites needed by rust-rocksdb bindgen.
- `crates/oregon-consensus/src/work.rs` — canonical `ChainWork` byte round-trip for storage only; chain-selection APIs still derive work from validated `PrePowHeaderFacts`.
- `crates/oregon-utxo/src/error.rs` — explicit duplicate persisted outpoint error.
- `crates/oregon-utxo/src/state.rs` — checked persisted-state reconstruction and read-only iteration boundary.
- `crates/oregon-utxo/src/lib.rs` — export the persistence bridge without adding a validation bypass.

### New `oregon-storage` files

- `crates/oregon-storage/Cargo.toml` — exact RocksDB dependency and Oregon domain dependencies.
- `crates/oregon-storage/src/lib.rs` — public storage API exports.
- `crates/oregon-storage/src/error.rs` — `StorageError` and corruption/schema/durability distinctions.
- `crates/oregon-storage/src/codec.rs` — deterministic primitive codecs, bounded cursor, outpoint key codec.
- `crates/oregon-storage/src/records.rs` — versioned `BlockIndexRecord`, schema/health/tip metadata codecs.
- `crates/oregon-storage/src/schema.rs` — schema 1.0 open checks, migration marker and restart-resumable minor-migration runner.
- `crates/oregon-storage/src/batch.rs` — RocksDB-independent `StorageBatch` operation builder and durability mode.
- `crates/oregon-storage/src/db.rs` — RocksDB open/CF ownership, typed reads/iterators, durable and maintenance batch execution.
- `crates/oregon-storage/src/tests.rs` — storage round-trip, corruption, determinism, durability-mode, migration and reopen tests.

### New `oregon-chainstate` files

- `crates/oregon-chainstate/Cargo.toml` — Oregon domain dependencies plus `thiserror`.
- `crates/oregon-chainstate/src/lib.rs` — exported chainstate types and API.
- `crates/oregon-chainstate/src/error.rs` — `ChainStateError`, `StorageFaulted`, `DeepReorg`, `ReindexRequired`, missing body/undo errors.
- `crates/oregon-chainstate/src/config.rs` — trusted height-0 anchor/header, genesis timestamp and consensus params binding.
- `crates/oregon-chainstate/src/branch.rs` — branch ancestry lookup, MTP window collection and branch-aware `PowKeyBlockSource`.
- `crates/oregon-chainstate/src/state.rs` — open/bootstrap, block admission, active extension and storage-fault session state.
- `crates/oregon-chainstate/src/reorg.rs` — common-fork discovery, 8,064 boundary, staged disconnect/connect and atomic reorg publication.
- `crates/oregon-chainstate/src/prune.rs` — exact safe pruning predicate and idempotent maintenance plan.
- `crates/oregon-chainstate/src/tests.rs` — bootstrap/reopen, direct extension, side-chain, reorg, pruning, crash/fault and corruption integration tests.

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
- Produces error: `UtxoError::DuplicatePersistedOutpoint(OutPoint)`

- [ ] **Step 1: Enable the M4 development branch in normal Rust CI before pushing a RED commit**

Change the push branch list to include `oregon-v1-m4-persistent-chainstate` while preserving every existing branch and pinned checkout SHA.

- [ ] **Step 2: Write the failing `ChainWork` canonical encoding tests**

Add tests to `work.rs` that require exactly one representation for zero and reject empty/non-minimal input:

```rust
#[test]
fn chainwork_canonical_storage_bytes_round_trip() {
    let zero = ChainWork::zero();
    assert_eq!(zero.to_canonical_be_bytes(), vec![0]);
    assert_eq!(
        ChainWork::from_canonical_be_bytes(&[0]).unwrap(),
        zero
    );

    let work = block_work(Target::from_biguint(&BigUint::from(1u8)).unwrap());
    let encoded = work.to_canonical_be_bytes();
    assert_eq!(ChainWork::from_canonical_be_bytes(&encoded).unwrap(), work);
    assert_eq!(ChainWork::from_canonical_be_bytes(&[]), None);
    assert_eq!(ChainWork::from_canonical_be_bytes(&[0, 1]), None);
}
```

- [ ] **Step 3: Write the failing persisted UTXO reconstruction tests**

Add tests requiring duplicate rejection and normal verifier enforcement after restoration:

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

- [ ] **Step 4: Push the RED commit and record the intended CI failure**

Expected failure: missing `ChainWork` canonical methods and missing `UtxoState::from_persisted_entries`/`DuplicatePersistedOutpoint`; the failure must not be a workflow syntax or unrelated dependency error.

- [ ] **Step 5: Implement minimal canonical `ChainWork` bytes**

Use `BigUint::to_bytes_be()` and represent zero as exactly `[0]`. `from_canonical_be_bytes` returns `None` for empty input or a leading zero when length is greater than one, then constructs the internal `BigUint` from the canonical bytes.

- [ ] **Step 6: Implement checked UTXO restoration and read-only entry iteration**

Build a fresh `HashMap`, reject any second insert of the same `OutPoint`, and return `Self { entries }`. `entries()` returns `self.entries.iter()` only; it exposes no mutable map reference and does not alter `connect_block`, `disconnect_block`, maturity, fee or verifier rules.

- [ ] **Step 7: Run focused tests and workspace verification**

Run:

```bash
cargo +1.85.0 test --locked -p oregon-consensus chainwork_canonical_storage_bytes_round_trip
cargo +1.85.0 test --locked -p oregon-utxo persisted_utxo_reconstruction_rejects_duplicate_outpoints
cargo +1.85.0 test --locked -p oregon-utxo restored_state_still_requires_spend_verifier
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

Expected: all PASS.

- [ ] **Step 8: Commit GREEN and require fresh GitHub CI success before Task 2**

Suggested commit: `feat: add persistence-safe chainwork and utxo bridges`.

---

### Task 2: RocksDB Storage Crate, Column Families, and Schema 1.0 Open

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
- Produces constants: `CF_BLOCKS`, `CF_BLOCK_INDEX`, `CF_UTXO`, `CF_UNDO`, `CF_CHAIN_META`
- Produces: `SchemaVersion { major: u16, minor: u16 }`, current `SCHEMA_VERSION = 1.0`
- Produces: `OregonDb::open(path: impl AsRef<Path>) -> Result<OregonDb, StorageError>`
- Produces: `OregonDb::schema_version(&self) -> Result<SchemaVersion, StorageError>`
- Produces: `StorageError::{RocksDb, CorruptData, UnsupportedSchema, DurabilityFailure}`

- [ ] **Step 1: Add crate scaffolding and exact dependency lock without implementing `OregonDb::open`**

`crates/oregon-storage/Cargo.toml` must contain:

```toml
[package]
name = "oregon-storage"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true

[dependencies]
oregon-consensus = { path = "../oregon-consensus" }
oregon-primitives = { path = "../oregon-primitives" }
oregon-utxo = { path = "../oregon-utxo" }
rocksdb = { version = "=0.24.0", default-features = false, features = ["lz4", "bindgen-runtime"] }
thiserror = "2"
```

Add the crate to the workspace and regenerate `Cargo.lock` with Rust 1.85.0 before pushing the RED commit so `--locked` failure cannot mask the intended test failure.

- [ ] **Step 2: Add native build prerequisites to CI**

Before `Test`, install `clang` and `libclang-dev` on `ubuntu-latest`; do not change checkout permissions or unpin the checkout action.

- [ ] **Step 3: Write failing open/schema/CF tests**

Use a test helper that creates a unique directory under `std::env::temp_dir()` and removes it on drop. Require `OregonDb::open` to create/open all named CFs, reserve the default CF, and return schema `1.0` on reopen.

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

- [ ] **Step 4: Push RED and verify failure is missing storage behavior**

Expected failure: `OregonDb::open`/schema/CF API missing or test assertion failure, not Cargo lock or native toolchain setup.

- [ ] **Step 5: Implement minimal RocksDB open**

Use `DB::open_cf_descriptors` with `create_if_missing(true)` and `create_missing_column_families(true)`. Keep the mandatory default CF but never use it for Oregon keys. Store schema version under `chain_meta` key `b"schema/version"` as exactly four bytes: `major.to_be_bytes() || minor.to_be_bytes()`.

- [ ] **Step 6: Make first schema write synchronous**

When a brand-new DB has no schema key, write schema `1.0` with WAL enabled and `WriteOptions::set_sync(true)`. Existing schema `major != 1` returns `UnsupportedSchema` without rewriting it; existing minor greater than the current supported minor also returns `UnsupportedSchema`.

- [ ] **Step 7: Verify focused and workspace tests**

Run:

```bash
cargo +1.85.0 test --locked -p oregon-storage new_database_opens_with_schema_1_0_and_required_column_families
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

- [ ] **Step 8: Commit GREEN and require fresh CI**

Suggested commit: `feat: add rocksdb storage foundation`.

---

### Task 3: Deterministic Storage Record Codecs

**Files:**
- Create: `crates/oregon-storage/src/codec.rs`
- Create: `crates/oregon-storage/src/records.rs`
- Modify: `crates/oregon-storage/src/lib.rs`
- Modify: `crates/oregon-storage/src/tests.rs`

**Interfaces:**
- Produces: `encode_outpoint_key(&OutPoint) -> [u8; 36]`
- Produces: `decode_outpoint_key(&[u8]) -> Result<OutPoint, StorageError>`
- Produces: `encode_utxo_entry(&UtxoEntry) -> Vec<u8>` / `decode_utxo_entry(&[u8]) -> Result<UtxoEntry, StorageError>`
- Produces: `encode_block_undo(&BlockUndo) -> Vec<u8>` / `decode_block_undo(&[u8]) -> Result<BlockUndo, StorageError>`
- Produces: `ValidationStatus::{HeaderValidated, FullyValidated, Invalid}`
- Produces: `BlockIndexRecord { header, parent, height, cumulative_work, validation, body_retained }`
- Produces: deterministic `encode_block_index` / `decode_block_index`
- Produces: `NodeHealth::{Healthy, ReindexRequired}` and versioned metadata codecs.

- [ ] **Step 1: Write RED round-trip and corruption tests**

Require:

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
    let second = encode_block_undo(&undo);
    assert_eq!(first, second);
    assert_eq!(decode_block_undo(&first).unwrap(), undo);
    let mut tainted = first;
    tainted.push(0);
    assert!(matches!(decode_block_undo(&tainted), Err(StorageError::CorruptData(_))));
}
```

Also test UTXO locking-program length `65_536` accepted, `65_537` rejected on decode, duplicate outpoints in undo rejected, unknown record version rejected, truncated every record family rejected, and non-minimal `ChainWork` bytes rejected through the Task 1 constructor.

- [ ] **Step 2: Push RED and observe codec symbols/behavior missing**

- [ ] **Step 3: Implement a bounded storage cursor**

`StorageCursor` owns `input` and `offset`, exposes `read_u8/u16/u32/u64`, `read_exact`, `read_len(max)` using the existing canonical Oregon varint convention, and `finish()` that rejects trailing bytes. Reuse `oregon_primitives::write_varint` for lengths; do not add serde/bincode.

- [ ] **Step 4: Implement UTXO and undo codecs**

Record version byte is `1`. UTXO value layout is:

```text
version:u8 | value:u64 LE | creation_height:u64 LE | is_coinbase:u8 |
locking_program_len:canonical_varint | locking_program:bytes
```

Undo value layout is:

```text
version:u8 |
spent_count:canonical_varint |
  repeated(outpoint:36 | utxo_entry_len:canonical_varint | utxo_entry) |
created_count:canonical_varint |
  repeated(outpoint:36)
```

Decoder verifies sorted strict outpoint order in both collections and rejects duplicates/non-canonical order as corruption.

- [ ] **Step 5: Implement block-index codec including the full canonical header**

Persist canonical 114-byte `BlockHeader::encode()` bytes so MTP, parent linkage and RandomX key ancestry remain available after body pruning. Layout:

```text
version:u8 | header:114 | parent:32 | height:u64 LE |
chainwork_len:canonical_varint | canonical_chainwork_be |
validation:u8 | body_retained:u8
```

Decoder requires `parent == header.previous_block`, except the trusted height-0 anchor record where the stored parent is still exactly the header's `previous_block`; chainstate decides anchor trust, storage only enforces byte consistency.

- [ ] **Step 6: Implement health/tip/config metadata records**

Use distinct fixed `chain_meta` keys for anchor ID, genesis timestamp, active tip ID, active height, health and prune cursor. Active height mapping keys are `b"active/" || height.to_be_bytes()` so lexicographic iteration follows height.

- [ ] **Step 7: Run focused codec tests and full verification**

```bash
cargo +1.85.0 test --locked -p oregon-storage codec
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

- [ ] **Step 8: Commit GREEN and require fresh CI**

Suggested commit: `feat: add deterministic chainstate storage codecs`.

---

### Task 4: Typed RocksDB Reads, Atomic Batches, Durability Modes, and Migration Runner

**Files:**
- Create: `crates/oregon-storage/src/batch.rs`
- Modify: `crates/oregon-storage/src/db.rs`
- Modify: `crates/oregon-storage/src/schema.rs`
- Modify: `crates/oregon-storage/src/lib.rs`
- Modify: `crates/oregon-storage/src/tests.rs`

**Interfaces:**
- Produces: `StorageBatch::new()` with typed `put/delete` methods for blocks, indices, UTXOs, undo, active-height mapping, tip, health, prune cursor.
- Produces: `OregonDb::commit_durable(batch) -> Result<(), StorageError>`; always requests sync.
- Produces: `OregonDb::commit_maintenance(batch) -> Result<(), StorageError>`; requests non-sync maintenance durability.
- Produces typed getters/iterators: `get_block`, `get_index`, `get_utxo`, `iter_utxos`, `get_undo`, `active_id_at_height`, `active_tip`, `health`, `iter_body_retained_indices`.
- Produces private migration state machine with durable marker key `b"schema/migration"`.
- Produces test-only feature `test-hooks` that can inject a failure immediately before a durable batch is handed to RocksDB and can record the requested durability mode; feature is disabled by default.

- [ ] **Step 1: Write RED typed round-trip/batch tests**

Require one durable batch to atomically persist a sample block, index, UTXO, undo, active mapping and tip and recover all of them after close/reopen.

- [ ] **Step 2: Write RED durability-mode/failure-injection tests**

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

#[test]
fn injected_precommit_failure_writes_nothing() {
    let dir = TestDir::new("durable-failure");
    let db = OregonDb::open_with_test_hooks(dir.path()).unwrap();
    let mut batch = StorageBatch::new();
    batch.set_health(NodeHealth::ReindexRequired);
    db.test_hooks().fail_next_durable_write();
    assert!(matches!(db.commit_durable(batch), Err(StorageError::DurabilityFailure(_))));
    assert_eq!(db.health().unwrap(), NodeHealth::Healthy);
}
```

The test hook fails before RocksDB execution; it exists to prove publish ordering. Device-level ambiguous I/O errors remain handled by the chainstate `StorageFaulted` reopen rule.

- [ ] **Step 3: Push RED and observe missing batch/durability APIs**

- [ ] **Step 4: Implement `StorageBatch` without exposing RocksDB handles**

Store typed operations in a private enum and translate them into one RocksDB `WriteBatch` at commit time. Encoding happens before execution so codec failure cannot produce a partially constructed durable state.

- [ ] **Step 5: Implement exact durability mapping**

`commit_durable` creates `WriteOptions`, leaves WAL enabled, calls `set_sync(true)`, then calls `db.write_opt`. `commit_maintenance` calls `set_sync(false)`. Never call `disable_wal(true)` on either path.

- [ ] **Step 6: Implement typed reads and iterators**

Every value passes its deterministic decoder before returning. Missing required values return `Ok(None)` at the storage layer; chainstate decides whether absence is legal or a `MissingUndo`/`MissingBlockBody` failure.

- [ ] **Step 7: Write and implement RED/GREEN migration runner tests**

The production current schema remains `1.0`. Test the generic minor migration engine with a synthetic target `1.1`: durable marker is written first, a test hook interrupts after step 1, rerun sees the marker, repeats/resumes idempotently, completes, then clears the marker. Unknown major `2.0` open returns `UnsupportedSchema` and leaves bytes unchanged.

- [ ] **Step 8: Verify all storage tests and workspace gates**

```bash
cargo +1.85.0 test --locked -p oregon-storage
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

- [ ] **Step 9: Commit GREEN and require fresh CI**

Suggested commit: `feat: add atomic durable rocksdb batches`.

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
- Produces: `ChainConfig { anchor_header: BlockHeader, genesis_timestamp: u64, params: ConsensusParams }`
- Produces: `ChainState::open(path: impl AsRef<Path>, config: ChainConfig) -> Result<ChainState, ChainStateError>`
- Produces: `ChainState::tip() -> Tip { block_id, height, cumulative_work }`
- Produces: session health `SessionHealth::{Healthy, StorageFaulted, ReindexRequired}`.
- Anchor rule: height 0, cumulative work exactly zero, empty UTXO set, no caller-provided work.

- [ ] **Step 1: Add crate scaffold and write RED bootstrap/reopen tests**

```rust
#[test]
fn first_open_durably_initializes_height_zero_anchor_and_empty_utxo() {
    let dir = TestDir::new("bootstrap");
    let config = test_config();
    let anchor_id = config.anchor_header.block_id();
    let state = ChainState::open(dir.path(), config.clone()).unwrap();
    assert_eq!(state.tip().block_id, anchor_id);
    assert_eq!(state.tip().height, 0);
    drop(state);
    let reopened = ChainState::open(dir.path(), config).unwrap();
    assert_eq!(reopened.tip().block_id, anchor_id);
    assert_eq!(reopened.utxos().entries().count(), 0);
}
```

Also require reopen with a different anchor ID or genesis timestamp to fail closed.

- [ ] **Step 2: Push RED and observe missing chainstate API**

- [ ] **Step 3: Implement bootstrap as one synchronous durable batch**

Write anchor index with `height=0`, `cumulative_work=ChainWork::zero()`, `ValidationStatus::FullyValidated`, body-retained false, active mapping `0 -> anchor_id`, active tip 0, anchor ID, genesis timestamp, `Healthy` health and prune cursor 0. Do not accept caller-provided cumulative work.

- [ ] **Step 4: Implement reopen validation and UTXO reconstruction**

On existing DB:

1. require stored anchor/genesis values match config;
2. fail immediately if durable health is `ReindexRequired`;
3. validate active mappings from height 0 through tip, parent linkage and index heights;
4. require retained-window active bodies/undo for every active height greater than zero that lies in the current rollback range;
5. decode every persisted UTXO and call `UtxoState::from_persisted_entries`;
6. publish `ChainState` only after all checks succeed.

- [ ] **Step 5: Add corruption RED/GREEN tests**

Corrupt active tip index, delete an active mapping, or place malformed UTXO bytes through a storage test fixture; every case must fail closed and never silently initialize a replacement chain.

- [ ] **Step 6: Verify chainstate bootstrap and workspace gates**

```bash
cargo +1.85.0 test --locked -p oregon-chainstate bootstrap
cargo +1.85.0 test --locked -p oregon-chainstate reopen
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

- [ ] **Step 7: Commit GREEN and require fresh CI**

Suggested commit: `feat: add persistent chainstate bootstrap and reopen`.

---

### Task 6: Branch-Aware Header/RandomX Validation and Header-Only Side-Chain Admission

**Files:**
- Create: `crates/oregon-chainstate/src/branch.rs`
- Modify: `crates/oregon-chainstate/src/state.rs`
- Modify: `crates/oregon-chainstate/src/lib.rs`
- Modify: `crates/oregon-chainstate/src/tests.rs`

**Interfaces:**
- Produces private `BranchView` over validated `BlockIndexRecord` ancestry.
- `BranchView` implements `PowKeyBlockSource::validated_block_id_at_height` by walking the candidate parent ancestry; it never accepts a caller-provided key ID.
- Produces: `AcceptOutcome::{Extended, StoredSideChain, Reorganized}`.
- Produces: `ChainState::accept_block<V: SpendVerifier>(&mut self, block: Block, verifier: &V) -> Result<AcceptOutcome, ChainStateError>`.
- Candidate block APIs accept no `height`, no cumulative-work input and no RandomX key-block ID.

- [ ] **Step 1: Write RED branch ancestry and MTP tests**

Require `BranchView` to recover the exact ancestor ID at a requested height and collect at most the previous 11 timestamps from the candidate's own branch.

- [ ] **Step 2: Write RED API/security tests**

Construct a side chain whose required RandomX key-block is on that side branch. The accepted path must use the side ancestor ID. A wrong-key `LightEngine` path must still fail through M2 `PowEngineKeyMismatch`; no alternate API may inject a key-block ID.

- [ ] **Step 3: Push RED and observe missing branch-aware validation**

- [ ] **Step 4: Implement candidate facts entirely from stored validated ancestry**

For candidate parent index:

1. reject missing or `Invalid` parent;
2. compute `height = parent.height.checked_add(1)`;
3. decode/use parent header from the index;
4. collect candidate-branch MTP window of 1..=11 ancestor timestamps;
5. call `validate_header_pre_pow`;
6. obtain required RandomX key height using the same `oregon_pow::key_block_height(facts.height())` schedule used by M2;
7. ask `BranchView` for that validated ancestor ID;
8. derive the expected Oregon RandomX key, construct `LightEngine`, and call `validate_header_pow`;
9. compute cumulative work only as `parent.cumulative_work + facts.work()`.

- [ ] **Step 5: Implement durable header-only side-block storage**

If candidate does not directly extend the active tip and does not yet exceed active cumulative work, persist block body and `BlockIndexRecord { validation: HeaderValidated, body_retained: true }` in one synchronous durable batch and return `StoredSideChain`. No UTXO or active mapping changes occur.

- [ ] **Step 6: Add duplicate and invalid-parent tests**

Known block ID is idempotently rejected/returned without rewriting chainwork. Descendant of an index marked `Invalid` is rejected before PoW work is accumulated.

- [ ] **Step 7: Verify and commit GREEN**

```bash
cargo +1.85.0 test --locked -p oregon-chainstate branch
cargo +1.85.0 test --locked -p oregon-chainstate side_chain
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

Suggested commit: `feat: add branch-aware pow chain indexing`.

---

### Task 7: Atomic Direct Active-Chain Extension and Storage-Fault Session Semantics

**Files:**
- Modify: `crates/oregon-chainstate/src/state.rs`
- Modify: `crates/oregon-chainstate/src/error.rs`
- Modify: `crates/oregon-chainstate/src/tests.rs`

**Interfaces:**
- Direct extension path stages `UtxoState::connect_block` on a clone.
- Produces private `UtxoDelta` keyed deterministically by `OutPoint` using `BTreeMap<OutPoint, Option<UtxoEntry>>` with a comparator-compatible key wrapper if needed because `OutPoint` lacks `Ord`.
- Produces: storage-fault guard that rejects every later mutation with `ChainStateError::StorageFaulted`.

- [ ] **Step 1: Write RED successful extension/reopen test**

Use a valid height-1 founder coinbase block under test consensus parameters. After `accept_block` returns `Extended`, close/reopen and require the same active tip and persisted coinbase UTXOs.

- [ ] **Step 2: Write RED failed durable-write publication test**

With `oregon-storage/test-hooks`, inject failure immediately before the durable acceptance write. Require:

```rust
assert!(matches!(state.accept_block(block, &verifier), Err(ChainStateError::DurabilityFailure(_))));
assert_eq!(state.tip(), old_tip);
assert_eq!(state.session_health(), SessionHealth::StorageFaulted);
assert!(matches!(state.accept_block(next_block, &verifier), Err(ChainStateError::StorageFaulted)));
```

Close/reopen then let restart invariants determine the durable state before mutations resume.

- [ ] **Step 3: Push RED and observe missing atomic active extension**

- [ ] **Step 4: Implement staged direct extension**

Clone current UTXO state, call M3 `connect_block`, derive the UTXO write delta from returned `BlockUndo` (`spent` -> delete; each `created` -> read surviving entry from staged state and insert), then build one durable storage batch containing block, fully validated index, undo, UTXO delta, active mapping and tip.

- [ ] **Step 5: Publish memory only after `commit_durable` succeeds**

On success, replace in-memory UTXO/tip. On any storage error, retain old memory, set in-memory `StorageFaulted`, return durability/storage error and refuse later mutations until reopen.

- [ ] **Step 6: Verify exact atomicity tests and workspace gates**

```bash
cargo +1.85.0 test --locked -p oregon-chainstate direct_extension
cargo +1.85.0 test --locked -p oregon-chainstate storage_fault
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

- [ ] **Step 7: Commit GREEN and require fresh CI**

Suggested commit: `feat: persist active chain extensions atomically`.

---

### Task 8: Fully Staged Reorgs and Exact 8,064/8,065 Boundary

**Files:**
- Create: `crates/oregon-chainstate/src/reorg.rs`
- Modify: `crates/oregon-chainstate/src/state.rs`
- Modify: `crates/oregon-chainstate/src/error.rs`
- Modify: `crates/oregon-chainstate/src/tests.rs`

**Interfaces:**
- Produces constant: `REORG_WINDOW: u64 = 8_064`.
- Produces pure boundary function: `reorg_depth_allowed(depth: u64) -> bool { depth <= REORG_WINDOW }`.
- Produces common-fork/path preflight from index ancestry.
- Reorg disk delta uses an ordered overlay keyed by canonical outpoint bytes rather than whole-UTXO comparison.

- [ ] **Step 1: Write RED exact boundary test**

```rust
#[test]
fn reorg_window_accepts_8064_and_rejects_8065() {
    assert!(reorg_depth_allowed(8_064));
    assert!(!reorg_depth_allowed(8_065));
}
```

Also use synthetic index fixtures to prove depth 8,065 triggers durable `ReindexRequired` before any UTXO/active mapping mutation.

- [ ] **Step 2: Write RED atomic candidate validation test**

Build a side branch with several header-valid blocks where the final block is transaction-invalid. Require the active tip and UTXO state to remain byte-for-byte unchanged and mark the failing candidate suffix `Invalid` without publishing partial candidate state.

- [ ] **Step 3: Write RED valid reorg + reopen test**

Require a strictly greater-work candidate within the window to disconnect old active blocks with stored undo, connect candidate blocks through M3, commit once, return `Reorganized`, and reopen to the new tip/UTXO state.

- [ ] **Step 4: Push RED and observe reorg behavior missing**

- [ ] **Step 5: Implement fork/path preflight**

Before cloning UTXO state, locate common fork, compute disconnect depth, and gather:

- old active block IDs from tip down to fork, each with decodable undo;
- new candidate block IDs from fork child through candidate tip, each with retained decodable body;
- active mapping updates required if new tip height differs from old tip.

Any missing required record returns `MissingUndo` or `MissingBlockBody` with no mutation.

- [ ] **Step 6: Implement deep-reorg fail-closed**

If depth > 8,064, write only durable `NodeHealth::ReindexRequired` in a synchronous batch, set session health `ReindexRequired`, leave active tip/UTXO/mapping unchanged and reject further chain mutations.

- [ ] **Step 7: Implement staged disconnect/connect and ordered UTXO delta**

Clone current UTXO state. For every old undo, call M3 `disconnect_block`; record each `undo.created` as final-delete and each `(outpoint, entry)` in `undo.spent` as final-insert. Then connect candidate blocks forward through M3; for each new undo record `spent` as final-delete and newly surviving `created` entries as final-insert. Later operations overwrite earlier overlay entries for the same outpoint, yielding final delta relative to current durable state without scanning the whole UTXO set.

- [ ] **Step 8: Validate entire candidate before one durable publication**

If any candidate block fails M3 validation, do not write active-state changes. Durably mark the first failing block and candidate descendants through the attempted tip as `Invalid`, return the consensus error and preserve old active memory/disk.

- [ ] **Step 9: Commit valid reorg in one synchronous batch**

Batch contains final UTXO delta, new undo for every newly active block, active-height deletes/puts, new tip, candidate index upgrades to `FullyValidated`, and any newly received candidate block/index not already stored. Publish in-memory UTXO/tip only after durable success; storage error faults the session.

- [ ] **Step 10: Verify reorg tests and workspace gates**

```bash
cargo +1.85.0 test --locked -p oregon-chainstate reorg_window_accepts_8064_and_rejects_8065
cargo +1.85.0 test --locked -p oregon-chainstate invalid_final_candidate
cargo +1.85.0 test --locked -p oregon-chainstate valid_reorg
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

- [ ] **Step 11: Commit GREEN and require fresh CI**

Suggested commit: `feat: add atomic cumulative-work reorgs`.

---

### Task 9: Safe Idempotent Pruning

**Files:**
- Create: `crates/oregon-chainstate/src/prune.rs`
- Modify: `crates/oregon-chainstate/src/state.rs`
- Modify: `crates/oregon-chainstate/src/tests.rs`
- Modify: `crates/oregon-storage/src/batch.rs`

**Interfaces:**
- Produces: `retained_active_floor(height: u64) -> u64 { height.saturating_sub(REORG_WINDOW - 1) }`.
- Produces: `ChainState::prune() -> Result<PruneReport, ChainStateError>`.
- Prune report records deleted body and undo counts only; it does not affect consensus state.

- [ ] **Step 1: Write RED pruning off-by-one tests**

For tip `H`, require active body+undo at `H-8063` to remain and data at `H-8064` to be eligible for deletion. Early chain heights use saturating arithmetic and prune nothing before enough history exists.

- [ ] **Step 2: Write RED side-branch retention tests**

For each body-retained side index:

- retain if its common fork with current active tip has disconnect depth <= 8,064 and its height is at/above the retained floor;
- allow body deletion if the common fork requires depth > 8,064 because any future winning reorg already requires `ReindexRequired`;
- allow body deletion for every block with height below retained floor.

- [ ] **Step 3: Write RED undo pruning rule tests**

Retain undo only for current active blocks within the rollback window. Side-branch undo may be removed because if a side branch later becomes active, M3 reconnect regenerates its undo before publication.

- [ ] **Step 4: Push RED and observe pruning behavior missing**

- [ ] **Step 5: Implement pruning as a separate maintenance batch**

Scan body-retained indices, evaluate the explicit safe predicate, delete eligible `blocks` values and set `body_retained=false` in the corresponding unpruned index records. Delete undo outside the current active retained window. Update prune cursor. Do not alter UTXO, active mapping, tip, chainwork or validation status.

- [ ] **Step 6: Commit pruning with `commit_maintenance` only**

Use `sync=false`; a crash can leave extra old data but cannot remove an accepted active state transaction. Rerunning `prune()` on already-pruned data returns success with zero or reduced deletion counts.

- [ ] **Step 7: Add interrupted-pruning idempotency test**

Use maintenance test hook to fail before one maintenance batch; reopen must show no consensus-state change. Rerun pruning must reach the same retained set as an uninterrupted run.

- [ ] **Step 8: Verify and commit GREEN**

```bash
cargo +1.85.0 test --locked -p oregon-chainstate prune
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

Suggested commit: `feat: add safe idempotent chain pruning`.

---

### Task 10: Restart/Corruption/Crash Acceptance Matrix

**Files:**
- Modify: `crates/oregon-storage/src/tests.rs`
- Modify: `crates/oregon-chainstate/src/tests.rs`

**Interfaces:**
- No new production API unless a test exposes a genuine missing invariant; any fix follows a fresh RED -> GREEN cycle.

- [ ] **Step 1: Add close/reopen state-equivalence test**

Connect multiple blocks including spends, close DB, reopen and compare tip plus sorted `(OutPoint, UtxoEntry)` entries exactly.

- [ ] **Step 2: Add accepted-before-prune crash-boundary test**

Durably accept a block, skip/fail pruning, close immediately, reopen and require accepted tip/UTXO plus harmless extra old data.

- [ ] **Step 3: Add retained-window corruption tests**

Delete/tamper an active undo or active body inside the window and require explicit startup failure. Delete a body behind the valid prune horizon and require startup to remain healthy.

- [ ] **Step 4: Add block/index identity corruption tests**

Stored block body must decode and its header ID must match the index key/header. Parent/height active-index mismatch is fail-closed.

- [ ] **Step 5: Add equal/lower cumulative-work tests**

Equal-work candidate remains side chain; lower-work candidate remains side chain. Neither changes active mapping or UTXO.

- [ ] **Step 6: Add deterministic-on-disk byte tests**

Construct logically equal UTXO/undo inputs from opposite insertion orders and require identical sorted persisted bytes. No test may rely on `HashMap` iteration order.

- [ ] **Step 7: Add schema/migration crash matrix**

Exercise synthetic supported minor migration interruption before marker, after marker, and after first idempotent step. All reruns converge. Unknown major is unchanged and fails closed.

- [ ] **Step 8: Run complete M4 verification locally and in fresh CI**

```bash
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

Record the exact clean commit SHA and GitHub Actions run ID as the pre-mutation clean gate.

- [ ] **Step 9: Commit acceptance-matrix additions**

Suggested commit: `test: harden persistent chainstate recovery boundaries`.

---

### Task 11: Required M4 Security Mutations

**Files:**
- Throwaway branches only; never merge mutation code into `oregon-v1-m4-persistent-chainstate`.

**Interfaces:**
- Mutation A must be killed by `reorg_window_accepts_8064_and_rejects_8065`.
- Mutation B must be killed by durability/fault publication tests.
- Mutation C must be killed by invalid-candidate atomicity tests.

- [ ] **Step 1: Mutation A — off-by-one deep reorg**

Create `mutation-m4-reorg-off-by-one-2026-09-03` from the pre-mutation clean M4 head. Change the boundary from `depth <= REORG_WINDOW` to an incorrect condition that permits 8,065 or rejects 8,064. If normal workflow branch filters do not include the mutation branch, modify the workflow only on the throwaway branch to trigger CI.

Expected failure: `reorg_window_accepts_8064_and_rejects_8065`.

- [ ] **Step 2: Mutation B — durability weakening**

Create `mutation-m4-durability-sync-2026-09-03`. Deliberately route active acceptance through non-sync durability or publish in-memory active state before `commit_durable` returns.

Expected failure: `durable_commit_requests_sync_and_maintenance_does_not` and/or the direct-extension storage-fault publication test.

- [ ] **Step 3: Mutation C — early reorg publication**

Create `mutation-m4-early-reorg-publication-2026-09-03`. Deliberately publish staged candidate UTXO/tip before the final candidate block validates or before the durable reorg batch succeeds.

Expected failure: invalid-final-candidate atomicity/storage-fault tests.

- [ ] **Step 4: Record exact mutation evidence**

For each mutation record branch, mutation commit, optional CI-trigger commit, workflow run ID, failed job ID, exact intended failing test and observed symptom. Confirm mutation code exists only on the throwaway branch.

- [ ] **Step 5: Return to clean M4 branch and run a fresh post-mutation gate**

If necessary create an empty tree-equivalent commit solely to force fresh post-mutation CI. Require workspace Test, Format and Clippy success at the exact reviewed clean source tree.

---

### Task 12: M3 -> M4 Security Review and Accepted Checkpoint

**Files:**
- Create: `docs/checkpoints/OREGON_V1_M4_PERSISTENT_CHAINSTATE.md`
- No production changes unless review finds a defect; every defect fix gets its own RED -> GREEN cycle and invalidates prior clean-gate evidence.

**Interfaces:**
- Produces accepted recovery branch: `oregon-v1-checkpoint-m4-persistent-chainstate-accepted-2026-09-03`.

- [ ] **Step 1: Compare exact accepted M3 base to final M4 code head**

Review range begins at `8f3ee9043b9cf3beb7b8e4653c0f2ab183233b71`. Enumerate changed production/test/workflow files and confirm no unrelated refactor or secret material.

- [ ] **Step 2: Review the critical security boundaries manually**

Verify from source:

- durable active writes use WAL and sync;
- no active memory publication precedes durable success;
- storage errors fault the session;
- WriteBatch includes all UTXO/undo/active mapping/tip changes for a reorg;
- ChainWork candidate provenance comes from `PrePowHeaderFacts`, not caller input;
- branch-aware RandomX key ID comes from validated ancestry;
- M3 spend verifier remains mandatory;
- no founder maturity exemption or amount arithmetic regression;
- reorg depth 8,064/8,065 condition is exact;
- pruning cannot delete required active undo/body one block early;
- side-body retention does not remove a branch still eligible for an allowed reorg;
- disk codecs reject truncation/trailing/non-canonical forms;
- startup rebuild rejects duplicate UTXOs and corrupt retained state;
- migration marker/resume behavior is idempotent;
- no consensus-visible storage encoding depends on `HashMap` iteration order.

- [ ] **Step 3: Require no open Critical or Important findings**

If a finding exists, stop checkpointing, add a focused RED regression test, implement minimal GREEN, rerun full CI, then repeat review of the changed range.

- [ ] **Step 4: Create the M4 checkpoint document**

Record frozen behavior, exact reviewed code commit, pre/post-mutation CI runs, all mutation evidence, RocksDB 0.24.0 pin, schema 1.0, pruning window, explicit exclusions and review disposition.

- [ ] **Step 5: Run fresh CI on the checkpoint commit itself**

Require Test, Format and Clippy all successful at the checkpoint-doc commit.

- [ ] **Step 6: Create accepted recovery branch**

Create `oregon-v1-checkpoint-m4-persistent-chainstate-accepted-2026-09-03` at the verified checkpoint commit. Do not merge to `main`.

- [ ] **Step 7: Report M4 acceptance only with exact evidence**

Report reviewed code SHA, checkpoint SHA, clean CI run IDs, mutation run IDs, accepted recovery branch and remaining milestone exclusions. Do not claim full node/mainnet readiness.
