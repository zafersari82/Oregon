# Oregon M6 Network Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Oregon M6 so multiple nodes establish real bounded TCP peer sessions, relay only authoritatively accepted transactions/blocks, and synchronize a behind node through headers-first fork-aware synchronization without moving any consensus, economic, UTXO, storage, RandomX, chain-selection, or mempool-policy ownership into networking.

**Architecture:** Preserve the existing core dependency graph and add five one-way higher-level crates: `oregon-protocol -> oregon-network -> oregon-peer -> oregon-sync -> oregon-node`. `oregon-node` owns one bounded blocking core worker containing `ChainState`, `Mempool`, and the caller-supplied `SpendVerifier`; async socket tasks never mutate core state directly. Headers are durably validated by chainstate first, sync consumes only coarse authoritative chain views/results, and full objects become relay eligible only after core acceptance.

**Tech Stack:** Rust 1.85.0 / edition 2024, Tokio 1.x, RocksDB through existing `oregon-storage`, BLAKE3 through existing dependencies, `thiserror` 2, `async-trait` 0.1 for transport/sync-view async traits, `getrandom` 0.3 for process instance nonces.

**Spec:** `docs/superpowers/specs/2026-09-04-oregon-m6-network-design.md`

## Global Constraints

- `%5` founder allocation and every accepted economic/consensus rule stay frozen.
- `oregon-consensus`, `oregon-utxo`, `oregon-storage`, `oregon-chainstate`, and `oregon-mempool` never depend on any M6 network crate.
- All five M6 crates declare `#![forbid(unsafe_code)]`.
- `chain_id = ChainConfig.anchor_header.block_id()`; network crates receive only the opaque resulting hash.
- `FRAME_VERSION = 1`, `PROTOCOL_VERSION_CURRENT = 1`, `PROTOCOL_VERSION_MIN = 1`.
- Protocol tags are fixed: `Hello=0x01`, `HelloAck=0x02`, `Ping=0x03`, `Pong=0x04`, `Inv=0x10`, `GetData=0x11`, `GetHeaders=0x20`, `Headers=0x21`, `Transaction=0x30`, `Block=0x31`.
- `MAX_FRAME_PAYLOAD = 2 MiB`, `MAX_HANDSHAKE_PAYLOAD = 4 KiB`, `MAX_INV_ITEMS = 4,096`, `MAX_GETDATA_ITEMS = 128`, `MAX_LOCATOR_HASHES = 64`, `MAX_HEADERS_PER_MESSAGE = 128`, `HEADER_VALIDATION_SLICE = 16`.
- `DEFAULT_MAX_PEERS = 64`, `DEFAULT_MAX_OUTBOUND = 16`, `DEFAULT_MAX_INBOUND = 48`, `HARD_MAX_PEERS = 128`.
- `MAX_QUEUE_FRAMES_PEER = 256`, `MAX_QUEUE_BYTES_PEER = 4 MiB`, `MAX_QUEUE_BYTES_GLOBAL = 64 MiB`, `CONTROL_RESERVED_FRAMES = 16`, `CONTROL_RESERVED_BYTES = 64 KiB`, `QUEUE_ENQUEUE_TIMEOUT = 2 s`.
- `MAX_CORE_COMMANDS = 64`, `MAX_CORE_COMMAND_BYTES = 8 MiB`.
- `HANDSHAKE_TIMEOUT = 10 s`, `MAX_PENDING_HANDSHAKES = 32`, `FRAME_NO_PROGRESS_TIMEOUT = 15 s`, `MAX_FRAME_READ_DURATION = 60 s`, `FRAME_WRITE_TIMEOUT = 15 s`, `PING_INTERVAL = 30 s`, `PONG_TIMEOUT = 15 s`, `IDLE_TIMEOUT = 120 s`, `RESPONSE_START_TIMEOUT = 20 s`.
- `MAX_IN_FLIGHT_BLOCKS_GLOBAL = 32`, `MAX_IN_FLIGHT_BLOCKS_PEER = 8`, `MAX_BUFFERED_BLOCKS = 32`, `MAX_BLOCK_ATTEMPTS = 3`, `EXPIRED_REQUEST_GRACE = 30 s`, `MAX_RECENTLY_EXPIRED_REQUESTS_PER_PEER = 128`.
- `MAX_KNOWN_INVENTORY_PER_PEER = 8,192`, `MAX_RECENT_RELAY_CACHE = 65,536`, `DISCONNECT_COOLDOWN = 10 min`, `MAX_COOLDOWN_ENTRIES = 1,024`.
- Remote `best_height`/`best_block_id` are hints only and never select a chain.
- A decoded remote transaction/block is not relay eligible until `Mempool::admit` / `ChainState::accept_block` accepts it.
- No test-only permissive `SpendVerifier` is exported into production APIs or binaries.
- Every coherent task finishes with `cargo test` for the changed crate(s), formatting, clippy for the changed crate(s), and a focused commit.

---

## File Structure Map

### Existing files modified

- `Cargo.toml` — add five M6 workspace members only.
- `Cargo.lock` — lock Tokio/network dependencies once crate manifests exist.
- `crates/oregon-primitives/src/transaction.rs` — remove attacker-count-driven vector preallocation.
- `crates/oregon-primitives/src/block.rs` — remove attacker-count-driven block transaction preallocation.
- `crates/oregon-storage/src/schema.rs` — real schema `1.1` and production minor-migration metadata.
- `crates/oregon-storage/src/records.rs` — durable preferred-header-tip keys.
- `crates/oregon-storage/src/batch.rs` — batch setter for preferred-header tip.
- `crates/oregon-storage/src/db.rs` — 1.0 -> 1.1 migration and preferred-header-tip read/write encoding.
- `crates/oregon-storage/src/migration_tests.rs` — real migration/recovery tests.
- `crates/oregon-chainstate/src/lib.rs` — export only network-independent header/sync view types.
- `crates/oregon-chainstate/src/state.rs` — keep active full tip and preferred validated header tip separately.
- `crates/oregon-chainstate/src/recovery.rs` — bootstrap/recover preferred header tip.
- `crates/oregon-chainstate/src/admission.rs` — reuse shared header validation and correctly promote known header-only blocks.
- `crates/oregon-chainstate/src/transition.rs` — keep preferred header metadata synchronized when full-block acceptance advances it.
- `crates/oregon-chainstate/src/tests.rs` — header-first/restart/body-promotion characterization.
- `.github/workflows/oregon-rust.yml` — M6 implementation branch and dependency/remote-allocation architecture scans.

### Existing files created inside core crates

- `crates/oregon-chainstate/src/header.rs` — single authoritative header validation/index construction path shared by header-only and full-block admission.
- `crates/oregon-chainstate/src/sync_view.rs` — network-independent preferred-header ancestry/query API.

### New M6 crates

- `crates/oregon-protocol/{Cargo.toml,src/lib.rs,src/constants.rs,src/error.rs,src/features.rs,src/frame.rs,src/message.rs,src/tests.rs}`
- `crates/oregon-network/{Cargo.toml,src/lib.rs,src/error.rs,src/io.rs,src/tcp.rs,src/transport.rs,src/tests.rs}`
- `crates/oregon-peer/{Cargo.toml,src/lib.rs,src/budget.rs,src/config.rs,src/cooldown.rs,src/error.rs,src/handshake.rs,src/request.rs,src/score.rs,src/service.rs,src/session.rs,src/tests.rs}`
- `crates/oregon-sync/{Cargo.toml,src/lib.rs,src/error.rs,src/locator.rs,src/scheduler.rs,src/state.rs,src/view.rs,src/tests.rs}`
- `crates/oregon-node/{Cargo.toml,src/lib.rs,src/core.rs,src/error.rs,src/node.rs,src/relay.rs,src/sync_adapter.rs,src/tests.rs,tests/loopback.rs,tests/support/mod.rs}`

---

### Task 1: Harden canonical primitive decoding before remote exposure

**Files:**
- Modify: `crates/oregon-primitives/src/transaction.rs`
- Modify: `crates/oregon-primitives/src/block.rs`
- Test: existing unit tests in both files

**Interfaces:**
- Consumes: existing `Transaction::decode(bytes, limits)` and `Block::decode(bytes, limits)` signatures.
- Produces: identical public signatures and canonical acceptance semantics; only allocation timing changes.

- [ ] **Step 1: Add failing regression tests for extreme declared counts with tiny payloads**

Add tests that encode a canonical large count and truncate immediately after it. The test must complete with an error instead of requiring count-sized allocation:

```rust
#[test]
fn huge_declared_input_count_with_tiny_payload_fails_without_preallocation_contract() {
    let bytes = [0x01, 0x00, 0xfd, 0xff, 0xff]; // v1, 65535 inputs, no input bytes
    let error = Transaction::decode(&bytes, &DecodeLimits::default()).unwrap_err();
    assert_eq!(error, PrimitiveError::UnexpectedEof);
}
```

For block decoding, build a valid 114-byte header followed by canonical count `0xfe 00 00 01 00` (65,536 txs) and no transaction bytes; expect `UnexpectedEof`.

- [ ] **Step 2: Run the focused tests before implementation**

Run:

```bash
cargo test -p oregon-primitives huge_declared_input_count_with_tiny_payload_fails_without_preallocation_contract
cargo test -p oregon-primitives huge_declared_transaction_count_with_tiny_payload_fails_without_preallocation_contract
```

Expected: tests may currently pass functionally, but source inspection still shows attacker-count-sized `Vec::with_capacity(...)`; the red condition for this task is the architecture scan added in Step 3.

- [ ] **Step 3: Add a source-level test guard that fails on remote-count preallocation**

Add a unit test using `include_str!` for the two source files and assert the forbidden patterns are absent:

```rust
#[test]
fn remote_counts_are_not_used_for_direct_vector_preallocation() {
    let tx = include_str!("transaction.rs");
    let block = include_str!("block.rs");
    for forbidden in [
        "Vec::with_capacity(input_count)",
        "Vec::with_capacity(output_count)",
        "Vec::with_capacity(witness_count)",
        "Vec::with_capacity(transaction_count)",
    ] {
        assert!(!tx.contains(forbidden) && !block.contains(forbidden), "{forbidden}");
    }
}
```

Run it and verify it fails on current code.

- [ ] **Step 4: Remove count-driven preallocation without changing parsing**

Replace untrusted-count `Vec::with_capacity(count)` with `Vec::new()` in transaction inputs, witness items, outputs, and block transactions. Do not change varint rules, limits, error types, field order, or canonical bytes.

- [ ] **Step 5: Verify primitive behavior and formatting**

Run:

```bash
cargo test -p oregon-primitives
cargo fmt --all -- --check
cargo clippy -p oregon-primitives --all-targets -- -D warnings
```

Expected: PASS; existing round-trip/property tests remain unchanged.

- [ ] **Step 6: Commit**

```bash
git add crates/oregon-primitives/src/transaction.rs crates/oregon-primitives/src/block.rs
git commit -m "security: harden remote primitive allocation"
```

---

### Task 2: Persist a preferred validated-header tip with a real schema 1.0 -> 1.1 migration

**Files:**
- Modify: `crates/oregon-storage/src/schema.rs`
- Modify: `crates/oregon-storage/src/records.rs`
- Modify: `crates/oregon-storage/src/batch.rs`
- Modify: `crates/oregon-storage/src/db.rs`
- Modify: `crates/oregon-storage/src/migration_tests.rs`

**Interfaces:**
- Produces: `StorageBatch::set_preferred_header_tip(block_id: Hash256, height: u64)`.
- Produces: `OregonDb::preferred_header_tip() -> Result<Option<(Hash256, u64)>, StorageError>`.
- Preserves: synchronous WAL durability for acceptance writes and fail-closed incompatible schema behavior.

- [ ] **Step 1: Write migration and metadata tests**

Add tests with these assertions:

```rust
assert_eq!(db.schema_version().unwrap(), SchemaVersion { major: 1, minor: 1 });
assert_eq!(db.preferred_header_tip().unwrap(), Some((active_id, active_height)));
```

Cover: fresh 1.1 DB; legacy 1.0 DB with active tip migrates preferred tip from active tip; a legacy empty DB migrates without inventing a tip; interrupted 1.0 -> 1.1 migration resumes idempotently; mismatched preferred-tip id/height pair is corrupt; major version mismatch remains `UnsupportedSchema`.

- [ ] **Step 2: Run migration tests and verify failure**

Run:

```bash
cargo test -p oregon-storage migration -- --nocapture
```

Expected: FAIL because schema 1.1 and preferred-header metadata do not exist.

- [ ] **Step 3: Promote migration marker codec to production use and bump the minor schema**

Set:

```rust
pub(crate) const SCHEMA_VERSION: SchemaVersion = SchemaVersion { major: 1, minor: 1 };
```

Remove `#[cfg(test)]` from the migration-marker codec/constants needed by the real migration. Keep the marker versioned and fixed-width.

- [ ] **Step 4: Add preferred header metadata operations**

In `records.rs` add:

```rust
pub(crate) const PREFERRED_HEADER_TIP_ID_KEY: &[u8] = b"headers/tip_id";
pub(crate) const PREFERRED_HEADER_TIP_HEIGHT_KEY: &[u8] = b"headers/tip_height";
```

In `StorageBatch` add one atomic logical operation:

```rust
pub fn set_preferred_header_tip(&mut self, block_id: Hash256, height: u64) {
    self.operations.push(StorageOp::SetPreferredHeaderTip(block_id, height));
}
```

Encode that operation as two writes in the same RocksDB batch.

- [ ] **Step 5: Implement exact 1.0 -> 1.1 production migration**

`OregonDb::open_internal` must accept exactly schema 1.0 as the only supported older version, write/resume a migration marker, copy a complete active-tip pair into the preferred-header pair when present, then atomically write schema 1.1 and delete the marker with `sync=true` and WAL enabled. Partial active-tip metadata remains corruption; arbitrary older/newer minors are not guessed.

- [ ] **Step 6: Add the read API**

Implement `preferred_header_tip()` with the same pair-consistency rule as `active_tip()`:

```rust
match (id, height) {
    (None, None) => Ok(None),
    (Some(id), Some(height)) => Ok(Some((decode_hash(&id, "preferred header tip id")?, decode_u64_le(&height, "preferred header tip height")?))),
    _ => Err(corrupt("preferred header tip metadata is partially present")),
}
```

- [ ] **Step 7: Verify storage**

Run:

```bash
cargo test -p oregon-storage --all-targets
cargo fmt --all -- --check
cargo clippy -p oregon-storage --all-targets -- -D warnings
```

Expected: PASS including existing durability and synthetic migration coverage adapted to the real 1.1 baseline.

- [ ] **Step 8: Commit**

```bash
git add crates/oregon-storage

git commit -m "feat: persist preferred validated header tip"
```

---

### Task 3: Create one authoritative chainstate header-validation/import path

**Files:**
- Create: `crates/oregon-chainstate/src/header.rs`
- Modify: `crates/oregon-chainstate/src/lib.rs`
- Modify: `crates/oregon-chainstate/src/state.rs`
- Modify: `crates/oregon-chainstate/src/recovery.rs`
- Modify: `crates/oregon-chainstate/src/admission.rs`
- Modify: `crates/oregon-chainstate/src/transition.rs`
- Test: `crates/oregon-chainstate/src/tests.rs`

**Interfaces:**
- Produces public domain types:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderTip {
    pub block_id: Hash256,
    pub height: u64,
    pub cumulative_work: ChainWork,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderImportStatus { Known, Stored, Preferred }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderImportOutcome {
    pub block_id: Hash256,
    pub height: u64,
    pub status: HeaderImportStatus,
    pub preferred_tip: HeaderTip,
}
```

- Produces `ChainState::accept_header(&mut self, header: BlockHeader) -> Result<HeaderImportOutcome, ChainStateError>`.
- Produces `ChainState::preferred_header_tip(&self) -> &HeaderTip`.
- Preserves `ChainState::accept_block<V: SpendVerifier>(...)` signature.

- [ ] **Step 1: Write header-import tests**

Required tests:

```rust
let before_active = state.tip().clone();
let outcome = state.accept_header(header.clone()).unwrap();
assert_eq!(outcome.status, HeaderImportStatus::Preferred);
assert_eq!(state.tip(), &before_active);
assert_eq!(state.preferred_header_tip().block_id, header.block_id());
```

Also test lower-work side header -> `Stored` without replacing preferred tip; duplicate header -> `Known`; unknown parent rejection; invalid PoW/header rejection; storage-failure atomicity; preferred header survives close/reopen.

- [ ] **Step 2: Run the focused chainstate tests and verify failure**

Run:

```bash
cargo test -p oregon-chainstate header_import -- --nocapture
```

Expected: FAIL because the new API/types do not exist.

- [ ] **Step 3: Extract shared header validation from block admission**

Move the exact existing sequence from `admission.rs` into a crate-private helper in `header.rs`:

```rust
pub(crate) fn validate_candidate_header(
    state: &ChainState,
    header: &BlockHeader,
) -> Result<BlockIndexRecord, ChainStateError>
```

The helper must perform, in the existing order: parent lookup/status check -> branch MTP -> checked height -> `validate_header_pre_pow` -> RandomX key-height/ancestor -> `LightEngine` with derived key -> `validate_header_pow` -> checked cumulative work -> return `BlockIndexRecord { validation: HeaderValidated, body_retained: false }`.

`accept_block_healthy` must call this same helper for previously unknown headers; no duplicated header-validation code remains.

- [ ] **Step 4: Add preferred header state and bootstrap/recovery**

Add `header_tip: HeaderTip` to `ChainState`. Bootstrap writes `set_preferred_header_tip(anchor_id, 0)` in the same durable batch as active-tip initialization. Reopen requires a preferred-tip pair, loads its index, verifies height/id/status/work/ancestry, and rejects a preferred header descending from an invalid record.

- [ ] **Step 5: Implement `accept_header` as durable-before-publication**

For a new valid header, build a storage batch with the index and, only when cumulative work exceeds the current preferred tip, the preferred-tip metadata. Commit durably first; update in-memory `header_tip` only after commit succeeds. Equality does not replace the current preferred tip in M6; deterministic first-accepted tie behavior remains local and does not alter active chain-selection rules.

- [ ] **Step 6: Keep full-block paths synchronized with preferred header state**

Whenever normal full-block admission validates a previously unknown header whose cumulative work exceeds `header_tip`, include preferred-header metadata in the same durable transition batch and publish the in-memory header tip after the durable commit. Do not update preferred state on failed full-block validation.

- [ ] **Step 7: Verify chainstate**

Run:

```bash
cargo test -p oregon-chainstate --all-targets
cargo fmt --all -- --check
cargo clippy -p oregon-chainstate --all-targets -- -D warnings
```

Expected: PASS; existing block/reorg/durability behavior remains green.

- [ ] **Step 8: Commit**

```bash
git add crates/oregon-chainstate

git commit -m "feat: add authoritative header import path"
```

---

### Task 4: Make header-only indexes promotable by full block bodies and expose network-independent sync queries

**Files:**
- Create: `crates/oregon-chainstate/src/sync_view.rs`
- Modify: `crates/oregon-chainstate/src/lib.rs`
- Modify: `crates/oregon-chainstate/src/admission.rs`
- Modify: `crates/oregon-chainstate/src/tests.rs`

**Interfaces:**
- Produces:

```rust
impl ChainState {
    pub fn preferred_header_id_at_height(&self, height: u64) -> Result<Option<Hash256>, ChainStateError>;
    pub fn preferred_header_at_height(&self, height: u64) -> Result<Option<BlockHeader>, ChainStateError>;
    pub fn active_id_at_height(&self, height: u64) -> Result<Option<Hash256>, ChainStateError>;
    pub fn body_retained(&self, block_id: Hash256) -> Result<bool, ChainStateError>;
    pub fn chain_id(&self) -> Hash256;
}
```

- `chain_id()` returns exactly `self.config.anchor_header.block_id()`.

- [ ] **Step 1: Write header-first body-promotion tests**

Test the critical previously-impossible path:

```rust
state.accept_header(block.header.clone()).unwrap();
assert_eq!(state.tip().height, 0);
assert_eq!(state.accept_block(block.clone(), &RejectTestSpends).unwrap(), AcceptOutcome::Extended);
assert_eq!(state.tip().height, 1);
```

Use a block form that does not call spend verification when it contains no normal spend, matching existing chainstate fixtures. Verify the stored index changes from `HeaderValidated/body_retained=false` to the existing correct full-block status and retained body/undo semantics.

Also cover header-only sidechain body storage before it wins work, then later ordered bodies allowing existing reorg logic to activate it.

- [ ] **Step 2: Run the tests and verify the current early-return bug**

Run:

```bash
cargo test -p oregon-chainstate header_only -- --nocapture
```

Expected: FAIL because current `accept_block_healthy` treats any known non-invalid index as already accepted and returns without storing/validating a missing body.

- [ ] **Step 3: Correct known-index handling in `accept_block_healthy`**

Use this decision table:

```text
Invalid index                         -> corruption/error as today
FullyValidated + retained body        -> existing idempotent outcome
HeaderValidated + retained body       -> existing side-chain body behavior
HeaderValidated + body_retained=false -> process this body using stored height/work/header
```

For the last case, verify the incoming header exactly equals the stored header, then use existing `extend_active` / side-chain store / `reorganize` paths with the stored authoritative `height` and `cumulative_work`; do not rerun or reimplement fork/work selection in networking.

- [ ] **Step 4: Implement sync query methods as thin chainstate views**

`preferred_header_id_at_height` walks `BranchView` from the preferred tip. `preferred_header_at_height` resolves that id through storage. `active_id_at_height` delegates to storage. `body_retained` reads the index and returns false for unknown ids. None of these methods accepts peer/protocol types.

- [ ] **Step 5: Verify queries, restart, and regressions**

Run:

```bash
cargo test -p oregon-chainstate --all-targets
cargo test -p oregon-storage --all-targets
cargo fmt --all -- --check
cargo clippy -p oregon-chainstate --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/oregon-chainstate

git commit -m "feat: support header-first body promotion"
```

---

### Task 5: Add the five M6 crates with exact dependency direction and no behavior yet

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Create: each new crate manifest and `src/lib.rs`

**Interfaces:**
- Produces five compiling crates with `#![forbid(unsafe_code)]` and no upward core dependency.

- [ ] **Step 1: Add workspace members**

Append exactly:

```toml
"crates/oregon-protocol",
"crates/oregon-network",
"crates/oregon-peer",
"crates/oregon-sync",
"crates/oregon-node",
```

- [ ] **Step 2: Create manifests with one-way dependencies**

Use these dependency sets:

```text
oregon-protocol: oregon-primitives, blake3, thiserror
oregon-network:  oregon-protocol, tokio, async-trait, thiserror
oregon-peer:     oregon-network, oregon-protocol, tokio, getrandom, thiserror
oregon-sync:     oregon-peer, oregon-protocol, oregon-primitives, async-trait, thiserror
oregon-node:     oregon-chainstate, oregon-mempool, oregon-network, oregon-peer, oregon-protocol, oregon-sync, oregon-primitives, oregon-utxo, tokio, async-trait, thiserror
```

Use `tokio = { version = "1", features = ["io-util", "macros", "net", "rt-multi-thread", "sync", "time"] }` where required; `oregon-network` may use the smaller `io-util,net,sync,time` subset.

- [ ] **Step 3: Add minimal unsafe-forbidden libraries**

Each `lib.rs` starts:

```rust
#![forbid(unsafe_code)]
```

No future-feature stubs for QUIC, discovery, DHT, compact blocks, or launch daemon are added.

- [ ] **Step 4: Verify workspace compilation and lockfile**

Run:

```bash
cargo check --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/oregon-protocol crates/oregon-network crates/oregon-peer crates/oregon-sync crates/oregon-node

git commit -m "build: add M6 network crate boundaries"
```

---

### Task 6: Implement exact Oregon protocol-v1 messages, features, frame codec, and golden vectors

**Files:**
- Create/Modify: `crates/oregon-protocol/src/constants.rs`
- Create/Modify: `crates/oregon-protocol/src/features.rs`
- Create/Modify: `crates/oregon-protocol/src/message.rs`
- Create/Modify: `crates/oregon-protocol/src/frame.rs`
- Create/Modify: `crates/oregon-protocol/src/error.rs`
- Modify: `crates/oregon-protocol/src/lib.rs`
- Test: `crates/oregon-protocol/src/tests.rs`

**Interfaces:**

```rust
pub struct FeatureSet(u64);
pub enum InventoryKind { Transaction, Block }
pub struct InventoryItem { pub kind: InventoryKind, pub hash: Hash256 }
pub struct Hello { /* exact spec fields */ }
pub struct HelloAck { pub selected_protocol_version: u16, pub enabled_features: FeatureSet, pub remote_nonce_echo: [u8; 16] }
pub enum Message { Hello(Hello), HelloAck(HelloAck), Ping(u64), Pong(u64), Inv(Vec<InventoryItem>), GetData(Vec<InventoryItem>), GetHeaders(GetHeaders), Headers(Vec<BlockHeader>), Transaction(Transaction), Block(Block) }
pub struct FrameHeader { pub network_magic: [u8; 4], pub frame_version: u8, pub message_type: u8, pub flags: u16, pub payload_length: u32, pub checksum: [u8; 4] }
pub fn network_magic(chain_id: Hash256) -> [u8; 4];
pub fn encode_message(message: &Message) -> Result<(u8, Vec<u8>), ProtocolError>;
pub fn decode_message(tag: u8, payload: &[u8]) -> Result<Message, ProtocolError>;
pub fn build_frame_header(magic: [u8; 4], tag: u8, payload: &[u8]) -> Result<FrameHeader, ProtocolError>;
pub fn verify_frame_payload(header: &FrameHeader, expected_magic: [u8; 4], payload: &[u8]) -> Result<(), ProtocolError>;
```

- [ ] **Step 1: Write exact-tag, exact-length, and feature-negotiation tests first**

Examples:

```rust
assert_eq!(MessageTag::Hello as u8, 0x01);
assert_eq!(encode_message(&Message::Hello(hello)).unwrap().1.len(), 108);
assert_eq!(encode_message(&Message::HelloAck(ack)).unwrap().1.len(), 26);
```

Test highest mutual protocol version, no overlap, unknown optional feature ignored, unknown/unsupported required feature rejected, and `HelloAck` mismatch rejection helper.

- [ ] **Step 2: Add frame golden-vector tests and run them red**

Build one fixed chain id/ping payload and assert the complete 16-byte header plus payload hex is stable. Add corruption tests for wrong magic, non-zero flags, wrong checksum, unknown tag, truncated exact-size handshake payload, list count 4,097/129/65, and `MAX_FRAME_PAYLOAD + 1` header rejection.

Run:

```bash
cargo test -p oregon-protocol --all-targets
```

Expected: FAIL before implementation.

- [ ] **Step 3: Implement constants/features and canonical payload codecs**

Use existing Oregon canonical varints through `oregon-primitives::write_varint` and `Decoder`. Do not introduce serde/bincode. `Transaction` and `Block` payloads call their existing `encode/decode` directly.

- [ ] **Step 4: Implement the 16-byte frame header/checksum**

Checksum input is exactly:

```text
BLAKE3("OREGON/FRAME/V1\0" || 12-byte-header-without-checksum || payload)
```

`network_magic` is exactly first four bytes of `BLAKE3("OREGON/NETMAGIC/V1\0" || chain_id)`.

- [ ] **Step 5: Verify protocol**

Run:

```bash
cargo test -p oregon-protocol --all-targets
cargo fmt --all -- --check
cargo clippy -p oregon-protocol --all-targets -- -D warnings
```

Expected: PASS with golden vectors.

- [ ] **Step 6: Commit**

```bash
git add crates/oregon-protocol

git commit -m "feat: define Oregon protocol v1 wire format"
```

---

### Task 7: Implement bounded framed TCP transport with progress and absolute deadlines

**Files:**
- Create/Modify: `crates/oregon-network/src/transport.rs`
- Create/Modify: `crates/oregon-network/src/tcp.rs`
- Create/Modify: `crates/oregon-network/src/io.rs`
- Create/Modify: `crates/oregon-network/src/error.rs`
- Modify: `crates/oregon-network/src/lib.rs`
- Test: `crates/oregon-network/src/tests.rs`

**Interfaces:**

```rust
#[async_trait::async_trait]
pub trait Transport: Send + Sync + 'static {
    type Connection: TransportConnection;
    type Listener: TransportListener<Connection = Self::Connection>;
    async fn bind(&self, addr: SocketAddr, magic: [u8; 4]) -> Result<Self::Listener, NetworkError>;
    async fn connect(&self, addr: SocketAddr, magic: [u8; 4]) -> Result<Self::Connection, NetworkError>;
}

#[async_trait::async_trait]
pub trait TransportConnection: Send + 'static {
    fn remote_addr(&self) -> SocketAddr;
    async fn read_message(&mut self) -> Result<Message, NetworkError>;
    async fn write_message(&mut self, message: &Message) -> Result<(), NetworkError>;
    async fn shutdown(&mut self) -> Result<(), NetworkError>;
}
```

`TcpTransport`, `TcpListenerHandle`, and `TcpConnection` implement these traits.

- [ ] **Step 1: Write `tokio::io::duplex` framed-I/O tests first**

Cover exact before-allocation size rejection by writing only a 16-byte header advertising `MAX_FRAME_PAYLOAD + 1` and proving the reader returns `NetworkError::OversizedFrame` without waiting for payload bytes. Add wrong checksum, truncated frame, write/read roundtrip, no-progress timeout, and absolute trickle-duration timeout using Tokio paused time where practical.

- [ ] **Step 2: Run tests red**

```bash
cargo test -p oregon-network --all-targets
```

Expected: FAIL because transport/framed IO does not exist.

- [ ] **Step 3: Implement a progress-aware exact reader**

Do not use one unbounded `read_exact` timeout. Loop reads so every successful byte receipt resets `FRAME_NO_PROGRESS_TIMEOUT`, while a separately measured absolute deadline enforces `MAX_FRAME_READ_DURATION`. Parse the fixed frame header first and reject oversized length before allocating `Vec<u8>` for payload.

- [ ] **Step 4: Implement bounded frame writes and TCP transport**

`write_message` encodes payload first, verifies it is within `MAX_FRAME_PAYLOAD`, builds the protocol frame header, and wraps the full write in `FRAME_WRITE_TIMEOUT`. TCP bind/connect set `TCP_NODELAY`; no consensus/policy state appears here.

- [ ] **Step 5: Verify network crate**

```bash
cargo test -p oregon-network --all-targets
cargo fmt --all -- --check
cargo clippy -p oregon-network --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/oregon-network

git commit -m "feat: add bounded TCP transport"
```

---

### Task 8: Implement peer configuration, bounded queues, handshake, self/duplicate protection

**Files:**
- Create/Modify: `crates/oregon-peer/src/config.rs`
- Create/Modify: `crates/oregon-peer/src/budget.rs`
- Create/Modify: `crates/oregon-peer/src/handshake.rs`
- Create/Modify: `crates/oregon-peer/src/session.rs`
- Create/Modify: `crates/oregon-peer/src/service.rs`
- Create/Modify: `crates/oregon-peer/src/error.rs`
- Modify: `crates/oregon-peer/src/lib.rs`
- Test: `crates/oregon-peer/src/tests.rs`

**Interfaces:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PeerId(u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction { Inbound, Outbound }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueClass { Control, RequiredData, Gossip }

pub struct PeerConfig {
    pub max_peers: usize,
    pub max_outbound: usize,
    pub max_inbound: usize,
}

pub struct EstablishedPeer {
    pub peer_id: PeerId,
    pub remote_addr: SocketAddr,
    pub direction: Direction,
    pub negotiated_version: u16,
    pub features: FeatureSet,
    pub remote_best_height: u64,
    pub remote_best_block_id: Hash256,
}

pub enum PeerEvent {
    Established(EstablishedPeer),
    Message { peer_id: PeerId, message: Message },
    Disconnected { peer_id: PeerId, reason: DisconnectReason },
}
```

- [ ] **Step 1: Write exact config/queue boundary tests**

Test invalid `inbound + outbound > total`, total > 128, 256th/257th frame behavior, 4 MiB exact byte boundary, 64 MiB global accounting, and 16-frame/64-KiB control reservation. Gossip drops when data capacity is full; required/control send waits at most two seconds then causes disconnect/no side buffer.

- [ ] **Step 2: Write handshake state tests**

Cover: exact legal state transitions; gossip before `Established` => violation; no version overlap; unsupported required feature; `HelloAck` mismatch; 10-second timeout; 32 pending accepted / 33rd rejected; self nonce; both perspectives of simultaneous A<->B duplicate arbitration.

- [ ] **Step 3: Run peer tests red**

```bash
cargo test -p oregon-peer --all-targets
```

Expected: FAIL before implementation.

- [ ] **Step 4: Implement byte-and-count budget queues**

Keep accounting private to the crate. Queue insertion reserves both frame count and bytes before enqueue; drop/release returns both budgets exactly once. Control reservation lives inside total per-peer/global caps.

- [ ] **Step 5: Implement process nonce and handshake state machine**

Generate startup nonce with:

```rust
let mut nonce = [0u8; 16];
getrandom::fill(&mut nonce).map_err(PeerError::Entropy)?;
```

Implement the exact nonce comparison rule from the spec. `best_height` remains stored peer metadata only.

- [ ] **Step 6: Implement `PeerService` over generic `Transport`**

The service owns accept/dial/session tasks and emits only established/message/disconnect events. It does not inspect transaction/block validity.

- [ ] **Step 7: Verify peer lifecycle**

```bash
cargo test -p oregon-peer --all-targets
cargo fmt --all -- --check
cargo clippy -p oregon-peer --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/oregon-peer

git commit -m "feat: add bounded peer lifecycle and handshake"
```

---

### Task 9: Add peer request matching, liveness, scoring, and bounded cooldown

**Files:**
- Create/Modify: `crates/oregon-peer/src/request.rs`
- Create/Modify: `crates/oregon-peer/src/score.rs`
- Create/Modify: `crates/oregon-peer/src/cooldown.rs`
- Modify: `crates/oregon-peer/src/service.rs`
- Modify: `crates/oregon-peer/src/lib.rs`
- Test: `crates/oregon-peer/src/tests.rs`

**Interfaces:**

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RequestKey {
    Headers,
    Object(InventoryItem),
}

pub enum PeerCommand {
    Send { peer_id: PeerId, message: Message, class: QueueClass },
    Expect { peer_id: PeerId, key: RequestKey },
    Disconnect { peer_id: PeerId },
    Feedback { peer_id: PeerId, feedback: PeerFeedback },
}

pub enum PeerFeedback { InvalidHeader, InvalidBlock, RequestAbuse }

pub enum PeerEvent {
    /* existing variants */
    MatchedResponse { peer_id: PeerId, key: RequestKey, message: Message },
    RequestTimedOut { peer_id: PeerId, key: RequestKey },
    Unsolicited { peer_id: PeerId, message: Message },
}
```

- [ ] **Step 1: Write request/late-response tests**

Cover one outstanding `Headers` request per peer; object response matching by kind/hash; response-start timeout at 20 seconds; timed-out key moved to grace set; late match within 30 seconds discarded without score; 128 grace entries exact / deterministic oldest expiry eviction; non-grace unsolicited object +10.

- [ ] **Step 2: Write scoring/cooldown/liveness tests**

Assert exact points and thresholds:

```rust
score.add(Misbehavior::InvalidBlock);
assert_eq!(score.points(), 50);
assert!(!score.sync_eligible());
score.add(Misbehavior::InvalidBlock);
assert!(score.disconnect_required());
```

Test malformed +25, handshake +25, invalid response +10, unsolicited +10, request abuse +10, sync timeout +5, oversized immediate disconnect. Test ping every 30s, pong timeout 15s, idle 120s. Cooldown keys normalize IPv4-mapped IPv6 and evict earliest expiry when 1,024 cap is exceeded.

- [ ] **Step 3: Run tests red then implement request registry**

```bash
cargo test -p oregon-peer request -- --nocapture
cargo test -p oregon-peer score -- --nocapture
```

Expected: FAIL before implementation.

- [ ] **Step 4: Implement matching before `PeerEvent` exposure**

A `Headers`, `Transaction`, or `Block` message reaches upper layers only as `MatchedResponse` when registered. Grace responses are dropped locally. Unmatched full objects become `Unsolicited` and score according to the spec.

- [ ] **Step 5: Implement separate performance observations**

Track successful request count, timeout count, and integer response latency (e.g. microseconds as `u64`) separately from misbehavior points. Expose a read-only `PeerPerformance` snapshot for sync preference; never subtract misbehavior because a peer is fast.

- [ ] **Step 6: Verify peer behavior**

```bash
cargo test -p oregon-peer --all-targets
cargo fmt --all -- --check
cargo clippy -p oregon-peer --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/oregon-peer

git commit -m "feat: enforce peer request and scoring policy"
```

---

### Task 10: Implement pure fork-aware sync intent, locator construction, and bounded block scheduler

**Files:**
- Create/Modify: `crates/oregon-sync/src/view.rs`
- Create/Modify: `crates/oregon-sync/src/locator.rs`
- Create/Modify: `crates/oregon-sync/src/scheduler.rs`
- Create/Modify: `crates/oregon-sync/src/state.rs`
- Create/Modify: `crates/oregon-sync/src/error.rs`
- Modify: `crates/oregon-sync/src/lib.rs`
- Test: `crates/oregon-sync/src/tests.rs`

**Interfaces:**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncTip { pub block_id: Hash256, pub height: u64 }

#[async_trait::async_trait]
pub trait ChainSyncView: Send + Sync {
    async fn active_tip(&self) -> Result<SyncTip, SyncViewError>;
    async fn preferred_header_tip(&self) -> Result<SyncTip, SyncViewError>;
    async fn active_id_at_height(&self, height: u64) -> Result<Option<Hash256>, SyncViewError>;
    async fn preferred_header_id_at_height(&self, height: u64) -> Result<Option<Hash256>, SyncViewError>;
    async fn preferred_header_at_height(&self, height: u64) -> Result<Option<BlockHeader>, SyncViewError>;
    async fn body_retained(&self, block_id: Hash256) -> Result<bool, SyncViewError>;
}

pub enum SyncAction {
    SendGetHeaders { peer_id: PeerId, request: GetHeaders },
    RequestBlock { peer_id: PeerId, block_id: Hash256 },
    SubmitBlock { source: PeerId, block: Block },
    Stalled { block_id: Hash256 },
}
```

- [ ] **Step 1: Write locator tests first**

Given a fake preferred chain height 30, assert first ten entries step by one, then 2/4/8 doubling, anchor included, and length <=64. At exact 64 boundary, no 65th entry is emitted.

- [ ] **Step 2: Write header-response serving/validation tests**

Using a fake `ChainSyncView`, test highest locator hit on preferred chain, max 128 headers, 129 rejected at protocol boundary, contiguous first-parent requirement, outstanding request requirement (peer already guarantees this, sync still associates the response with current sync round), and remote best-height lies never alter the view-selected preferred tip.

- [ ] **Step 3: Write block scheduler tests**

Cover exactly: 32 global, 8 per peer, 32 buffered bodies, three total attempts, timeout releases ownership before reassignment, no assignment to score>=50 peer, late expired response never returns to in-flight, out-of-order blocks become `SubmitBlock` only in preferred-path order, and `Stalled` after attempt three.

- [ ] **Step 4: Run sync tests red**

```bash
cargo test -p oregon-sync --all-targets
```

Expected: FAIL before implementation.

- [ ] **Step 5: Implement locator and preferred-path topology without work comparison**

Sync may compare hashes/heights to find the common point between the authoritative active path and authoritative preferred-header path, but it never receives `ChainWork` and never computes/selects work. Populate the body target path from fork+1 through preferred header tip and skip ids whose bodies are already retained.

- [ ] **Step 6: Implement deterministic scheduler state**

Use `BTreeMap`/`BTreeSet` where ordering affects output. Choose eligible peers by: sync-eligible flag, then lower timeout count, then lower integer latency, then `PeerId` as deterministic tie-break. Cap global/per-peer in-flight before emitting `RequestBlock`.

- [ ] **Step 7: Verify sync**

```bash
cargo test -p oregon-sync --all-targets
cargo fmt --all -- --check
cargo clippy -p oregon-sync --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/oregon-sync

git commit -m "feat: add headers-first bounded sync scheduler"
```

---

### Task 11: Implement Oregon node composition, bounded core worker, sync adapter, and validation-before-relay

**Files:**
- Create/Modify: `crates/oregon-node/src/core.rs`
- Create/Modify: `crates/oregon-node/src/sync_adapter.rs`
- Create/Modify: `crates/oregon-node/src/relay.rs`
- Create/Modify: `crates/oregon-node/src/node.rs`
- Create/Modify: `crates/oregon-node/src/error.rs`
- Modify: `crates/oregon-node/src/lib.rs`
- Test: `crates/oregon-node/src/tests.rs`

**Interfaces:**

```rust
pub struct NodeConfig {
    pub listen_addr: SocketAddr,
    pub bootstrap_peers: Vec<SocketAddr>,
    pub peer: PeerConfig,
    pub mempool: MempoolConfig,
}

pub struct OregonNode<V, T = TcpTransport>
where
    V: SpendVerifier + Send + 'static,
    T: Transport;

pub struct CoreHandle { /* bounded sender + byte semaphore */ }
```

Core commands are internal and include `ImportHeaders`, `AcceptBlock`, `AdmitTransaction`, `SyncQuery`, and `Shutdown`, each with a `oneshot` reply where needed.

- [ ] **Step 1: Write core-queue tests first**

Test 64 exact command slots / 65th waits or fails by chosen bounded API; exact 8 MiB budget; permits released exactly once on receive/drop; `ImportHeaders` rejects a slice larger than 16; core work executes on the blocking worker rather than the async test thread (capture thread id in a test-only command).

- [ ] **Step 2: Write relay authorization tests**

Use fake peer output capture and a test-only verifier:

```rust
let result = core.admit_transaction(tx.clone(), source).await;
assert!(result.is_err());
assert!(!relay.sent_inventory(tx.txid()));
```

Add the positive case where mempool accepts then exactly one `Inv(TxId)` is eligible. Add invalid-block no-relay, accepted side-chain relay, active extension relay only after mempool reconciliation, and reconciliation-failure fallback to a new empty mempool on the new `ChainBase`.

- [ ] **Step 3: Run node tests red**

```bash
cargo test -p oregon-node --lib --all-targets
```

Expected: FAIL before implementation.

- [ ] **Step 4: Implement the bounded blocking core worker**

Use `tokio::sync::mpsc::channel(MAX_CORE_COMMANDS)` plus an `Arc<tokio::sync::Semaphore>` with `MAX_CORE_COMMAND_BYTES` permits. Each sender acquires byte permits before enqueue. Start one `tokio::task::spawn_blocking` loop and consume with `Receiver::blocking_recv()`. The worker exclusively owns `ChainState`, `Mempool`, the preserved `MempoolConfig`, and `V`.

Header batches from peers are split by node into chunks of at most 16 before `ImportHeaders` commands; yield between chunks by returning to the async event loop.

- [ ] **Step 5: Implement `ChainSyncView` adapter through core queries**

`sync_adapter.rs` implements the trait owned by `oregon-sync`; every method sends a bounded read query to the core worker and maps `ChainStateError` only to coarse `SyncViewError::Unavailable`. Do not expose consensus/storage error enums upward.

- [ ] **Step 6: Implement mempool/chainstate orchestration**

Construct `ChainBase` only from `ChainState::tip()`. For `AcceptOutcome::Extended`, call `reconcile_active_block`; for `Reorganized`, call `reconcile_reorg`; for `StoredSideChain`, keep the current mempool base. If reconciliation returns an unexpected error after durable chain acceptance, replace with `Mempool::new(new_base, saved_config.clone())` and continue with an empty pool.

- [ ] **Step 7: Implement inventory-first relay and bounded dedup**

Maintain per-peer known inventory at max 8,192 and node recent-relay cache at 65,536 using generation FIFO eviction. Never re-advertise immediately to the source peer. On `Inv`, request only objects the core/sync state says are needed, and register `PeerCommand::Expect` before `GetData` so a fast response cannot race the registry.

- [ ] **Step 8: Wire peer events and sync actions**

Sequence for requested tx/block must be:

```text
PeerEvent::MatchedResponse -> core command -> authoritative result -> relay cache/inventory -> PeerCommand::Send Inv
```

Sequence for headers:

```text
Matched Headers -> split <=16 -> core accept_header sequentially -> coarse preferred tip -> sync refresh -> register expected block response -> GetData
```

On `ReindexRequired`/storage-fault health, stop new core mutation/sync scheduling and surface node fault state; do not translate it into peer consensus blame.

- [ ] **Step 9: Verify node library**

```bash
cargo test -p oregon-node --lib --all-targets
cargo fmt --all -- --check
cargo clippy -p oregon-node --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add crates/oregon-node

git commit -m "feat: orchestrate validated Oregon peer relay"
```

---

### Task 12: Prove M6 with three real loopback nodes, architecture gates, and security mutations

**Files:**
- Create/Modify: `crates/oregon-node/tests/support/mod.rs`
- Create/Modify: `crates/oregon-node/tests/loopback.rs`
- Modify: `.github/workflows/oregon-rust.yml`
- Modify only if needed for test gating: crate manifests' `[dev-dependencies]` / test-only features

**Interfaces:**
- Test-only `AcceptAllSpends` stays under `crates/oregon-node/tests/support` and is never re-exported from `oregon-node`.
- Test harness exposes helpers only inside integration tests.

- [ ] **Step 1: Build a test-only three-node harness**

Use `127.0.0.1:0` listeners, independent temp RocksDB directories, the same `ChainConfig`, and a max-target fixture so RandomX hashes are accepted when all other header rules pass. Keep `AcceptAllSpends` private to the integration-test crate.

- [ ] **Step 2: Add handshake/duplicate/self end-to-end tests**

Assert real TCP A-B-C sessions establish, features negotiate, simultaneous A<->B dialing leaves exactly one physical session, and deliberate self-dial nonce identity is rejected.

- [ ] **Step 3: Add transaction relay test**

Seed a spendable test UTXO through accepted test chain state, create one valid transaction, inject it through one requested peer path, and assert it appears in downstream node mempool only after admission. Add a structurally/policy-invalid transaction and assert no downstream `Inv`/admission occurs.

- [ ] **Step 4: Add block relay and catch-up sync test**

Create/accept a multi-block chain on node A, start node C behind, connect through B, and assert C first imports headers then requests bounded bodies and reaches the same authoritative active tip. Observe that no peer exceeds eight in-flight and global in-flight never exceeds 32.

- [ ] **Step 5: Add fork/lying-height/reassignment tests**

Create competing branches where the peer advertising higher `best_height` does not have the preferred authoritative work path; assert chainstate result wins. Make one serving peer stop responding to a block request and assert ownership moves to another peer after `RESPONSE_START_TIMEOUT`; after three total failures assert `Stalled`.

- [ ] **Step 6: Extend CI architecture scans**

Add M6 implementation branch to workflow push branches. Add fail-closed scans such as:

```bash
if rg -n 'oregon-(protocol|network|peer|sync|node)' crates/oregon-{consensus,utxo,storage,chainstate,mempool}/Cargo.toml; then
  echo 'Core crate depends upward on M6 networking.' >&2
  exit 1
fi

if rg -n 'Vec::with_capacity\((input_count|output_count|witness_count|transaction_count)\)' crates/oregon-primitives/src; then
  echo 'Remote decoder preallocates from attacker count.' >&2
  exit 1
fi
```

Also scan all M6 crates for forbidden economic/consensus owner symbols where practical (`FOUNDER_ALLOCATION`, `block_subsidy`, `derive_randomx_key`, storage CF names), allowing only core adapter calls in `oregon-node` and no rule implementation.

- [ ] **Step 7: Run the full clean gate**

Run exactly:

```bash
cargo test --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo rustdoc -p oregon-chainstate -- -D warnings
cargo doc --workspace --no-deps
```

Expected: all exit 0.

- [ ] **Step 8: Perform required mutation experiments on throwaway branches from the exact clean SHA**

Run one mutation at a time and require a dedicated test failure for each:

1. bypass relay-before-validation guard -> invalid tx/block relay test must fail;
2. bypass frame-size check before payload allocation -> oversized-header transport test must fail;
3. permit gossip before handshake established -> handshake state test must fail;
4. raise/bypass sync in-flight cap -> exact 32/8 scheduler boundary test must fail;
5. use remote `best_height` as preferred chain -> lying-height/fork test must fail.

Do not merge mutation code. Record mutation branch SHA, failing test name, and clean re-run evidence later in the M6 checkpoint document.

- [ ] **Step 9: Re-run the exact clean SHA after all mutations**

Run the same full gate from Step 7 on the untouched implementation branch head. Expected: all exit 0.

- [ ] **Step 10: Commit only clean test/CI code**

```bash
git add crates/oregon-node/tests .github/workflows/oregon-rust.yml Cargo.toml Cargo.lock crates/*/Cargo.toml

git commit -m "test: prove Oregon M6 peer synchronization"
```

Do not create the acceptance checkpoint or merge `main` in this task. That requires separate independent review of the exact clean M6 implementation SHA.

---

## Plan Self-Review Checklist

- Spec coverage: every frozen frame/message constant, bounded queue/time/request value, headers-first behavior, relay authorization rule, peer-score rule, non-goal, and end-to-end acceptance condition maps to Tasks 1-12.
- Dependency direction: core changes are completed before M6 crates and never add upward dependencies; only `oregon-node` depends on core + M6 layers together.
- Type consistency: `HeaderTip`, `HeaderImportOutcome`, `PeerId`, `RequestKey`, `ChainSyncView`, `SyncAction`, `CoreHandle`, and `OregonNode<V,T>` are defined before downstream use.
- Persistence: preferred header tip is durable and migration-safe; active full-block tip remains separate.
- Header-first body promotion: the current known-header early return is explicitly replaced so downloaded bodies are actually accepted/stored.
- Remote allocation: primitive counts no longer drive immediate vector capacity; frame length is checked before payload allocation.
- CPU bounds: headers enter core in <=16-header slices even though protocol permits <=128 per message.
- Retry semantics: exactly three total block attempts; timed-out late responses remain bounded and never re-enter in-flight state.
- Spend verification: M6 has no permissive production verifier; real TCP acceptance uses test-only verifier code.
- No placeholders or deferred production APIs are required for QUIC/discovery/testnet/mainnet.
