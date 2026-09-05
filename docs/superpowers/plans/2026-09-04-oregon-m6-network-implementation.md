# Oregon M6 Network Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Oregon M6 so multiple nodes establish real bounded TCP peer sessions, relay only authoritatively accepted transactions/blocks, and synchronize a behind node through headers-first fork-aware synchronization without moving consensus, economics, UTXO, storage, RandomX, chain-selection, or mempool-policy ownership into networking.

**Architecture:** Preserve the current core dependency graph and add five one-way higher-level crates: `oregon-protocol -> oregon-network -> oregon-peer -> oregon-sync -> oregon-node`. `oregon-node` owns one bounded blocking core worker containing `ChainState`, `Mempool`, and the caller-supplied `SpendVerifier`; async socket tasks never mutate core state or run RandomX directly. Header validity/work and preferred-header selection remain in chainstate; sync consumes only coarse chain views/results; full objects become relay eligible only after core acceptance.

**Tech Stack:** Rust 1.85.0 / edition 2024, Tokio 1.x, existing RocksDB/BLAKE3 core dependencies, `thiserror` 2, `async-trait` 0.1, `getrandom` 0.3.

**Spec:** `docs/superpowers/specs/2026-09-04-oregon-m6-network-design.md`

## Global Constraints

- `%5` founder allocation and every accepted economic/consensus rule stay frozen.
- `oregon-consensus`, `oregon-utxo`, `oregon-storage`, `oregon-chainstate`, and `oregon-mempool` never depend on an M6 network crate.
- All five M6 crates declare `#![forbid(unsafe_code)]`.
- `chain_id = ChainConfig.anchor_header.block_id()`.
- Protocol tags: `Hello=0x01`, `HelloAck=0x02`, `Ping=0x03`, `Pong=0x04`, `Inv=0x10`, `GetData=0x11`, `GetHeaders=0x20`, `Headers=0x21`, `Transaction=0x30`, `Block=0x31`.
- `FRAME_VERSION=1`, `PROTOCOL_VERSION_CURRENT=1`, `PROTOCOL_VERSION_MIN=1`.
- `MAX_FRAME_PAYLOAD=2 MiB`, `MAX_HANDSHAKE_PAYLOAD=4 KiB`, `MAX_INV_ITEMS=4,096`, `MAX_GETDATA_ITEMS=128`, `MAX_LOCATOR_HASHES=64`, `MAX_HEADERS_PER_MESSAGE=128`, `HEADER_VALIDATION_SLICE=16`.
- `DEFAULT_MAX_PEERS=64`, `DEFAULT_MAX_OUTBOUND=16`, `DEFAULT_MAX_INBOUND=48`, `HARD_MAX_PEERS=128`.
- `MAX_QUEUE_FRAMES_PEER=256`, `MAX_QUEUE_BYTES_PEER=4 MiB`, `MAX_QUEUE_BYTES_GLOBAL=64 MiB`, `CONTROL_RESERVED_FRAMES=16`, `CONTROL_RESERVED_BYTES=64 KiB`, `QUEUE_ENQUEUE_TIMEOUT=2 s`.
- `MAX_CORE_COMMANDS=64`, `MAX_CORE_COMMAND_BYTES=8 MiB`.
- `HANDSHAKE_TIMEOUT=10 s`, `MAX_PENDING_HANDSHAKES=32`, `FRAME_NO_PROGRESS_TIMEOUT=15 s`, `MAX_FRAME_READ_DURATION=60 s`, `FRAME_WRITE_TIMEOUT=15 s`, `PING_INTERVAL=30 s`, `PONG_TIMEOUT=15 s`, `IDLE_TIMEOUT=120 s`, `RESPONSE_START_TIMEOUT=20 s`.
- `MAX_IN_FLIGHT_BLOCKS_GLOBAL=32`, `MAX_IN_FLIGHT_BLOCKS_PEER=8`, `MAX_BUFFERED_BLOCKS=32`, `MAX_BLOCK_ATTEMPTS=3`, `EXPIRED_REQUEST_GRACE=30 s`, `MAX_RECENTLY_EXPIRED_REQUESTS_PER_PEER=128`.
- `MAX_KNOWN_INVENTORY_PER_PEER=8,192`, `MAX_RECENT_RELAY_CACHE=65,536`, `DISCONNECT_COOLDOWN=10 min`, `MAX_COOLDOWN_ENTRIES=1,024`.
- Remote `best_height`/`best_block_id` are hints only.
- A decoded transaction/block is not relay eligible until `Mempool::admit` / `ChainState::accept_block` accepts it.
- No permissive test `SpendVerifier` is exported in production.

## File Structure

**Modify core:** `Cargo.toml`, `Cargo.lock`, primitive `transaction.rs`/`block.rs`; storage `schema.rs`/`records.rs`/`batch.rs`/`db.rs`/`migration_tests.rs`; chainstate `lib.rs`/`state.rs`/`recovery.rs`/`admission.rs`/`transition.rs`/`tests.rs`; `.github/workflows/oregon-rust.yml`.

**Create core files:** `crates/oregon-chainstate/src/header.rs`, `crates/oregon-chainstate/src/sync_view.rs`, `crates/oregon-primitives/tests/remote_allocation.rs`.

**Create M6 crates:**
- `oregon-protocol`: `constants.rs`, `error.rs`, `features.rs`, `frame.rs`, `message.rs`, `tests.rs`.
- `oregon-network`: `error.rs`, `io.rs`, `tcp.rs`, `transport.rs`, `tests.rs`.
- `oregon-peer`: `budget.rs`, `config.rs`, `cooldown.rs`, `error.rs`, `handshake.rs`, `request.rs`, `score.rs`, `service.rs`, `session.rs`, `tests.rs`.
- `oregon-sync`: `error.rs`, `locator.rs`, `scheduler.rs`, `state.rs`, `view.rs`, `tests.rs`.
- `oregon-node`: `core.rs`, `error.rs`, `node.rs`, `relay.rs`, `sync_adapter.rs`, `tests.rs`, `tests/loopback.rs`, `tests/support/mod.rs`.

---

### Task 1: Harden canonical primitive allocation

**Files:**
- Create: `crates/oregon-primitives/tests/remote_allocation.rs`
- Modify: `crates/oregon-primitives/src/transaction.rs`
- Modify: `crates/oregon-primitives/src/block.rs`

**Produces:** unchanged `Transaction::decode` / `Block::decode` signatures and bytes; no vector capacity is taken directly from remote element counts.

- [ ] **Step 1: Write malformed-count behavior tests**

```rust
#[test]
fn huge_declared_input_count_with_tiny_payload_is_bounded_failure() {
    let bytes = [0x01, 0x00, 0xfd, 0xff, 0xff];
    assert_eq!(
        Transaction::decode(&bytes, &DecodeLimits::default()),
        Err(PrimitiveError::UnexpectedEof)
    );
}
```

Add the equivalent block test: valid 114-byte header + canonical transaction count 65,536 + no tx bytes => `UnexpectedEof`.

- [ ] **Step 2: Add a separate source-contract test and verify it fails on current code**

`crates/oregon-primitives/tests/remote_allocation.rs` reads `../src/transaction.rs` and `../src/block.rs`; because the forbidden strings live in this separate integration-test file they do not self-match:

```rust
#[test]
fn remote_counts_never_drive_direct_vector_capacity() {
    let tx = include_str!("../src/transaction.rs");
    let block = include_str!("../src/block.rs");
    for forbidden in [
        "Vec::with_capacity(input_count)",
        "Vec::with_capacity(output_count)",
        "Vec::with_capacity(witness_count)",
        "Vec::with_capacity(transaction_count)",
    ] {
        assert!(!tx.contains(forbidden));
        assert!(!block.contains(forbidden));
    }
}
```

Run `cargo test -p oregon-primitives --test remote_allocation`; expected FAIL on the source-contract test.

- [ ] **Step 3: Implement the minimal hardening**

Replace only the four untrusted `Vec::with_capacity(count)` allocations with `Vec::new()`. Do not change varints, limits, versions, errors, or canonical encoding.

- [ ] **Step 4: Verify and commit**

```bash
cargo test -p oregon-primitives --all-targets
cargo fmt --all -- --check
cargo clippy -p oregon-primitives --all-targets -- -D warnings
git add crates/oregon-primitives
git commit -m "security: harden remote primitive allocation"
```

---

### Task 2: Add durable preferred-header metadata and real schema 1.0 -> 1.1 migration

**Files:** storage `schema.rs`, `records.rs`, `batch.rs`, `db.rs`, `migration_tests.rs`.

**Produces:**
```rust
StorageBatch::set_preferred_header_tip(block_id: Hash256, height: u64)
OregonDb::preferred_header_tip() -> Result<Option<(Hash256, u64)>, StorageError>
```

- [ ] **Step 1: Write red migration tests**

Cover fresh 1.1; legacy 1.0 active tip copied to preferred tip; empty 1.0 migrates without inventing a tip; interrupted migration resumes; partial preferred-tip pair is corrupt; unsupported major remains fail-closed.

```rust
assert_eq!(db.schema_version().unwrap(), SchemaVersion { major: 1, minor: 1 });
assert_eq!(db.preferred_header_tip().unwrap(), Some((active_id, active_height)));
```

Run `cargo test -p oregon-storage migration -- --nocapture`; expected FAIL.

- [ ] **Step 2: Replace the synthetic 1.1 migration harness with the production migration**

Set `SCHEMA_VERSION` to `{ major: 1, minor: 1 }`. Promote migration-marker encode/decode/constants from test-only to crate-private production code. Delete `open_with_synthetic_migration_1_1` and `run_synthetic_minor_migration_1_1`; their interruption/idempotence assertions move to the real 1.0 -> 1.1 tests.

- [ ] **Step 3: Add preferred-tip keys and atomic batch operation**

```rust
pub(crate) const PREFERRED_HEADER_TIP_ID_KEY: &[u8] = b"headers/tip_id";
pub(crate) const PREFERRED_HEADER_TIP_HEIGHT_KEY: &[u8] = b"headers/tip_height";
```

`StorageOp::SetPreferredHeaderTip(Hash256,u64)` encodes both keys into the same RocksDB batch.

- [ ] **Step 4: Implement exact production migration**

`open_internal` accepts current 1.1 or exactly legacy 1.0. For 1.0: create/resume marker `(1.0,1.1)`; validate active tip is either fully absent or a complete id/height pair; if complete copy it to preferred tip; finish with one synchronous WAL batch writing schema 1.1 and deleting marker. Unknown older/newer versions remain `UnsupportedSchema`.

- [ ] **Step 5: Implement pair-consistent read API**

Use the same `(None,None)/(Some,Some)/partial-corrupt` rule as `active_tip()`.

- [ ] **Step 6: Verify and commit**

```bash
cargo test -p oregon-storage --all-targets
cargo fmt --all -- --check
cargo clippy -p oregon-storage --all-targets -- -D warnings
git add crates/oregon-storage
git commit -m "feat: persist preferred validated header tip"
```

---

### Task 3: Create one authoritative chainstate header-validation/import path

**Files:** create `header.rs`; modify chainstate `lib.rs`, `state.rs`, `recovery.rs`, `admission.rs`, `transition.rs`, `tests.rs`.

**Produces:**
```rust
pub struct HeaderTip { pub block_id: Hash256, pub height: u64, pub cumulative_work: ChainWork }
pub enum HeaderImportStatus { Known, Stored, Preferred }
pub struct HeaderImportOutcome { pub block_id: Hash256, pub height: u64, pub status: HeaderImportStatus, pub preferred_tip: HeaderTip }
ChainState::accept_header(&mut self, header: BlockHeader) -> Result<HeaderImportOutcome, ChainStateError>
ChainState::preferred_header_tip(&self) -> &HeaderTip
```

- [ ] **Step 1: Write red tests**

Test preferred header does not mutate active tip; lower-work header is stored but not preferred; duplicate => `Known`; unknown parent; invalid header/PoW; injected durable failure preserves in-memory tip; close/reopen restores preferred tip.

```rust
let active_before = state.tip().clone();
let out = state.accept_header(header.clone()).unwrap();
assert_eq!(out.status, HeaderImportStatus::Preferred);
assert_eq!(state.tip(), &active_before);
assert_eq!(state.preferred_header_tip().block_id, header.block_id());
```

- [ ] **Step 2: Extract the existing header path exactly once**

Create:
```rust
pub(crate) fn validate_candidate_header(
    state: &ChainState,
    header: &BlockHeader,
) -> Result<BlockIndexRecord, ChainStateError>
```

Preserve order: parent lookup/status -> `BranchView` MTP -> checked height -> `validate_header_pre_pow` -> RandomX key height/ancestor -> `LightEngine` -> `validate_header_pow` -> cumulative work. Return `HeaderValidated/body_retained=false`. `accept_block_healthy` calls this helper for unknown headers.

- [ ] **Step 3: Add durable-before-publication preferred header state**

Add `header_tip: HeaderTip` to `ChainState`. Bootstrap writes preferred tip `(anchor,0)` in the same durable batch as active tip. Reopen requires the pair and validates its record/height/work/ancestry. `accept_header` commits index + optional new preferred-tip metadata before changing in-memory `header_tip`. Only strictly greater cumulative work replaces it.

- [ ] **Step 4: Keep full-block transitions synchronized**

When a newly validated full block has work greater than current `header_tip`, include preferred-tip metadata in the same durable transition batch. A failed full block never advances preferred header state.

- [ ] **Step 5: Verify and commit**

```bash
cargo test -p oregon-chainstate --all-targets
cargo fmt --all -- --check
cargo clippy -p oregon-chainstate --all-targets -- -D warnings
git add crates/oregon-chainstate
git commit -m "feat: add authoritative header import path"
```

---

### Task 4: Promote downloaded bodies for known header-only indexes and expose sync queries

**Files:** create `sync_view.rs`; modify chainstate `lib.rs`, `admission.rs`, `tests.rs`.

**Produces:**
```rust
ChainState::preferred_header_id_at_height(u64) -> Result<Option<Hash256>, ChainStateError>
ChainState::preferred_header_at_height(u64) -> Result<Option<BlockHeader>, ChainStateError>
ChainState::active_id_at_height(u64) -> Result<Option<Hash256>, ChainStateError>
ChainState::body_retained(Hash256) -> Result<bool, ChainStateError>
ChainState::chain_id() -> Hash256
```

- [ ] **Step 1: Add an exact valid height-one coinbase fixture and red body-promotion test**

In chainstate tests create height-one coinbase using public frozen rules:

```rust
fn height_one_coinbase(config: &ChainConfig) -> Transaction {
    let mut height = Vec::new();
    write_varint(1, &mut height);
    let mut founder_program = vec![0x01]; // KEY_COMMIT_V1 frozen value
    founder_program.extend_from_slice(&config.params.founder_key_commitment);
    Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_txid: Hash256::from_bytes([0; 32]),
            previous_output_index: u32::MAX,
            sequence: u32::MAX,
            witness: vec![height],
        }],
        outputs: vec![TxOutput {
            value: Amount::from_base_units(FOUNDER_ALLOCATION_BASE_UNITS).unwrap(),
            locking_program: founder_program,
        }],
        lock_time: 0,
    }
}
```

Build a block from this tx, import header first, then call `accept_block`; expected `Extended`, active height 1, retained body and `FullyValidated`. Run the focused test first; current known-index early return must fail the assertions.

- [ ] **Step 2: Correct known-index admission with this exact table**

```text
Invalid                              -> existing error
FullyValidated + body retained       -> idempotent existing outcome
HeaderValidated + body retained      -> existing stored-sidechain semantics
HeaderValidated + body not retained  -> process incoming body using stored header/height/work
```

For the last case require incoming header equality with stored header. Use existing `extend_active`, side-chain durable store, or `reorganize`; do not redo chain work in networking.

- [ ] **Step 3: Implement thin network-independent query methods**

`preferred_header_*` walk `BranchView` from `header_tip`; `active_id_at_height` delegates storage; `body_retained` reads index; `chain_id` returns `config.anchor_header.block_id()`. No peer/protocol types enter chainstate.

- [ ] **Step 4: Verify and commit**

```bash
cargo test -p oregon-chainstate --all-targets
cargo test -p oregon-storage --all-targets
cargo fmt --all -- --check
cargo clippy -p oregon-chainstate --all-targets -- -D warnings
git add crates/oregon-chainstate
git commit -m "feat: support header-first body promotion"
```

---

### Task 5: Add the five M6 crate boundaries

**Files:** root `Cargo.toml`, `Cargo.lock`; each new crate `Cargo.toml` + `src/lib.rs`.

**Dependency sets:**
```text
oregon-protocol: oregon-primitives, blake3, thiserror
oregon-network:  oregon-protocol, tokio, async-trait, thiserror
oregon-peer:     oregon-network, oregon-protocol, tokio, getrandom, thiserror
oregon-sync:     oregon-peer, oregon-protocol, oregon-primitives, async-trait, thiserror
oregon-node:     oregon-chainstate, oregon-mempool, oregon-network, oregon-peer, oregon-protocol, oregon-sync, oregon-primitives, oregon-utxo, tokio, async-trait, thiserror
```

- [ ] **Step 1: Add workspace members/manifests**

Use Tokio `version="1"`; node features `["macros","rt-multi-thread","sync","time"]`, network `["io-util","net","sync","time"]`. Every `lib.rs` begins `#![forbid(unsafe_code)]`. Do not scaffold QUIC/discovery/testnet APIs.

- [ ] **Step 2: Verify and commit**

```bash
cargo check --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
git add Cargo.toml Cargo.lock crates/oregon-{protocol,network,peer,sync,node}
git commit -m "build: add M6 network crate boundaries"
```

---

### Task 6: Implement exact protocol-v1 codec and golden vectors

**Files:** protocol modules listed in File Structure.

**Produces:**
```rust
pub struct FeatureSet(u64);
pub enum InventoryKind { Transaction, Block }
pub struct InventoryItem { pub kind: InventoryKind, pub hash: Hash256 }
pub struct Hello { pub min_protocol_version:u16, pub max_protocol_version:u16, pub chain_id:Hash256, pub instance_nonce:[u8;16], pub offered_features:FeatureSet, pub required_features:FeatureSet, pub best_height:u64, pub best_block_id:Hash256 }
pub struct HelloAck { pub selected_protocol_version:u16, pub enabled_features:FeatureSet, pub remote_nonce_echo:[u8;16] }
pub struct GetHeaders { pub locator:Vec<Hash256>, pub stop:Option<Hash256> }
pub enum Message { Hello(Hello), HelloAck(HelloAck), Ping(u64), Pong(u64), Inv(Vec<InventoryItem>), GetData(Vec<InventoryItem>), GetHeaders(GetHeaders), Headers(Vec<BlockHeader>), Transaction(Transaction), Block(Block) }
pub struct FrameHeader { pub network_magic:[u8;4], pub frame_version:u8, pub message_type:u8, pub flags:u16, pub payload_length:u32, pub checksum:[u8;4] }
pub fn network_magic(chain_id: Hash256) -> [u8;4];
pub fn encode_message(&Message) -> Result<(u8,Vec<u8>),ProtocolError>;
pub fn decode_message(u8,&[u8]) -> Result<Message,ProtocolError>;
pub fn build_frame_header([u8;4],u8,&[u8]) -> Result<FrameHeader,ProtocolError>;
pub fn verify_frame_payload(&FrameHeader,[u8;4],&[u8]) -> Result<(),ProtocolError>;
```

- [ ] **Step 1: Write red exact-wire tests**

Assert all numeric tags; Hello payload exactly 108 bytes; HelloAck 26; Ping/Pong 8; unknown tag/kind rejected; non-zero v1 flags rejected; list limits exact; protocol highest mutual version; unknown optional ignored; unsupported required rejected; canonical tx/block payload uses existing primitive encode/decode.

- [ ] **Step 2: Add one fixed frame golden vector**

Fixed chain id + Ping nonce must assert exact full frame bytes. Add checksum corruption/wrong magic/truncation/oversize tests. Run `cargo test -p oregon-protocol`; expected FAIL.

- [ ] **Step 3: Implement without serde/bincode**

Use existing `Decoder`/canonical varints. Frame checksum is first four bytes of `BLAKE3("OREGON/FRAME/V1\0" || 12-byte-header-without-checksum || payload)`; magic is first four of `BLAKE3("OREGON/NETMAGIC/V1\0" || chain_id)`.

- [ ] **Step 4: Verify and commit**

```bash
cargo test -p oregon-protocol --all-targets
cargo fmt --all -- --check
cargo clippy -p oregon-protocol --all-targets -- -D warnings
git add crates/oregon-protocol
git commit -m "feat: define Oregon protocol v1 wire format"
```

---

### Task 7: Implement bounded framed TCP transport

**Files:** network modules listed in File Structure.

**Produces:**
```rust
#[async_trait::async_trait]
pub trait TransportListener: Send + 'static {
    type Connection: TransportConnection;
    fn local_addr(&self) -> SocketAddr;
    async fn accept(&mut self) -> Result<Self::Connection, NetworkError>;
}

#[async_trait::async_trait]
pub trait Transport: Send + Sync + 'static {
    type Connection: TransportConnection;
    type Listener: TransportListener<Connection=Self::Connection>;
    async fn bind(&self, addr:SocketAddr, magic:[u8;4]) -> Result<Self::Listener,NetworkError>;
    async fn connect(&self, addr:SocketAddr, magic:[u8;4]) -> Result<Self::Connection,NetworkError>;
}

#[async_trait::async_trait]
pub trait TransportConnection: Send + 'static {
    fn remote_addr(&self) -> SocketAddr;
    async fn read_message(&mut self) -> Result<Message,NetworkError>;
    async fn write_message(&mut self, message:&Message) -> Result<(),NetworkError>;
    async fn shutdown(&mut self) -> Result<(),NetworkError>;
}
```

- [ ] **Step 1: Write red `tokio::io::duplex` tests**

Write only a 16-byte frame header advertising `MAX_FRAME_PAYLOAD+1`; reader must immediately return `OversizedFrame` without waiting for payload. Test roundtrip, checksum error, truncation, 15s no-progress, 60s absolute trickle deadline, 15s write deadline.

- [ ] **Step 2: Implement progress-aware reader**

Read fixed header first. Validate payload length before `Vec` allocation. Loop payload reads with a no-progress timeout reset only on received bytes plus independent absolute deadline.

- [ ] **Step 3: Implement TCP transport**

`TcpTransport`/listener/connection implement traits; set `TCP_NODELAY`; no peer policy or core state here.

- [ ] **Step 4: Verify and commit**

```bash
cargo test -p oregon-network --all-targets
cargo fmt --all -- --check
cargo clippy -p oregon-network --all-targets -- -D warnings
git add crates/oregon-network
git commit -m "feat: add bounded TCP transport"
```

---

### Task 8: Implement peer limits, queues, handshake, self/duplicate rules

**Files:** peer `config.rs`, `budget.rs`, `handshake.rs`, `session.rs`, `service.rs`, `error.rs`, `lib.rs`, `tests.rs`.

**Produces:**
```rust
pub struct PeerId(pub u64);
pub enum Direction { Inbound, Outbound }
pub enum QueueClass { Control, RequiredData, Gossip }
pub struct PeerConfig { pub max_peers:usize, pub max_outbound:usize, pub max_inbound:usize }
pub struct EstablishedPeer { pub peer_id:PeerId, pub remote_addr:SocketAddr, pub direction:Direction, pub negotiated_version:u16, pub features:FeatureSet, pub remote_best_height:u64, pub remote_best_block_id:Hash256 }
pub enum PeerEvent {
    Established(EstablishedPeer),
    Message { peer_id:PeerId, message:Message },
    Disconnected { peer_id:PeerId, reason:DisconnectReason },
}
```

- [ ] **Step 1: Write red exact-bound tests**

Test invalid config sums; total >128; 256/257 frames; 4 MiB per-peer; 64 MiB global; control reservation exact 16 frames/64 KiB; gossip drop; required/control enqueue timeout 2s. Test handshake legal states, pre-established gossip violation, version/features/Ack mismatch, 10s timeout, 32/33 pending, self nonce, both duplicate-arbitration perspectives.

- [ ] **Step 2: Implement byte+count queue budgets**

Reserve count and bytes before enqueue; release exactly once. Control reservation is inside the same caps/global budget. No fallback Vec/queue.

- [ ] **Step 3: Implement process nonce and handshake**

```rust
let mut nonce = [0u8;16];
getrandom::fill(&mut nonce).map_err(PeerError::Entropy)?;
```

Use exact nonce direction rule. Remote best height remains metadata only.

- [ ] **Step 4: Implement generic `PeerService<T:Transport>` and verify**

```bash
cargo test -p oregon-peer --all-targets
cargo fmt --all -- --check
cargo clippy -p oregon-peer --all-targets -- -D warnings
git add crates/oregon-peer
git commit -m "feat: add bounded peer lifecycle and handshake"
```

---

### Task 9: Add peer request matching, liveness, scoring, cooldown

**Files:** peer `request.rs`, `score.rs`, `cooldown.rs`, `service.rs`, `lib.rs`, `tests.rs`.

**Produces:**
```rust
pub enum RequestKey { Headers, Object(InventoryItem) }
pub enum PeerFeedback { InvalidHeader, InvalidBlock, RequestAbuse }
pub enum PeerCommand {
    Send { peer_id:PeerId, message:Message, class:QueueClass },
    Expect { peer_id:PeerId, key:RequestKey },
    Disconnect { peer_id:PeerId },
    Feedback { peer_id:PeerId, feedback:PeerFeedback },
}
pub enum PeerEvent {
    Established(EstablishedPeer),
    Message { peer_id:PeerId, message:Message },
    MatchedResponse { peer_id:PeerId, key:RequestKey, message:Message },
    RequestTimedOut { peer_id:PeerId, key:RequestKey },
    Unsolicited { peer_id:PeerId, message:Message },
    Disconnected { peer_id:PeerId, reason:DisconnectReason },
}
```

- [ ] **Step 1: Write red request/grace tests**

One outstanding Headers per peer; object match by kind/hash; 20s response-start timeout; timeout moves key to 30s grace; grace response discarded without score; 128 exact grace cap; deterministic earliest-expiry eviction; non-grace unsolicited +10.

- [ ] **Step 2: Write red score/liveness/cooldown tests**

Exact points: malformed +25; handshake +25; invalid response +10; unsolicited +10; request abuse +10; sync timeout +5; invalid header/block +50; oversized immediate disconnect. At 50 no new sync; at 100 disconnect. Test 30s ping/15s pong/120s idle. Normalize IPv4-mapped IPv6; cooldown cap 1,024, earliest expiry evicted.

- [ ] **Step 3: Implement request registry before upper-layer exposure**

Headers/Transaction/Block only surface as `MatchedResponse`; bounded grace matches are dropped; unmatched full object surfaces as `Unsolicited`. Performance snapshot stores success count, timeout count, integer response latency separately from score.

- [ ] **Step 4: Verify and commit**

```bash
cargo test -p oregon-peer --all-targets
cargo fmt --all -- --check
cargo clippy -p oregon-peer --all-targets -- -D warnings
git add crates/oregon-peer
git commit -m "feat: enforce peer request and scoring policy"
```

---

### Task 10: Implement locator and bounded fork-aware sync scheduler

**Files:** sync modules listed in File Structure.

**Produces:**
```rust
pub struct SyncTip { pub block_id:Hash256, pub height:u64 }
#[async_trait::async_trait]
pub trait ChainSyncView: Send + Sync {
    async fn active_tip(&self) -> Result<SyncTip,SyncViewError>;
    async fn preferred_header_tip(&self) -> Result<SyncTip,SyncViewError>;
    async fn active_id_at_height(&self,u64) -> Result<Option<Hash256>,SyncViewError>;
    async fn preferred_header_id_at_height(&self,u64) -> Result<Option<Hash256>,SyncViewError>;
    async fn preferred_header_at_height(&self,u64) -> Result<Option<BlockHeader>,SyncViewError>;
    async fn body_retained(&self,Hash256) -> Result<bool,SyncViewError>;
}
pub enum SyncAction {
    SendGetHeaders { peer_id:PeerId, request:GetHeaders },
    RequestBlock { peer_id:PeerId, block_id:Hash256 },
    SubmitBlock { source:PeerId, block:Block },
    Stalled { block_id:Hash256 },
}
```

- [ ] **Step 1: Write red locator/header tests**

First ten locator entries step 1, then 2/4/8 doubling, anchor included, max 64. Serve from highest locator hit on preferred path, max 128 headers. Remote advertised height never modifies preferred tip.

- [ ] **Step 2: Write red scheduler tests**

Exact 32 global/8 peer/32 buffered; three total attempts; timeout releases ownership before reassignment; score>=50 peer ineligible; out-of-order blocks emit `SubmitBlock` only in preferred-path order; third failure emits `Stalled`.

- [ ] **Step 3: Implement topology without `ChainWork`**

Find common point by opaque active/preferred ids+heights from `ChainSyncView`; never receive/compare work. Build missing-body target list from preferred fork path and skip retained bodies.

- [ ] **Step 4: Implement deterministic peer choice**

Sort eligible peers by timeout count, then integer latency, then `PeerId`; capability/sync-eligibility is a prerequisite. Enforce caps before emitting `RequestBlock`.

- [ ] **Step 5: Verify and commit**

```bash
cargo test -p oregon-sync --all-targets
cargo fmt --all -- --check
cargo clippy -p oregon-sync --all-targets -- -D warnings
git add crates/oregon-sync
git commit -m "feat: add headers-first bounded sync scheduler"
```

---

### Task 11: Implement node core worker, sync adapter, orchestration, validation-before-relay

**Files:** node modules listed in File Structure.

**Produces:**
```rust
pub struct NodeConfig { pub listen_addr:SocketAddr, pub bootstrap_peers:Vec<SocketAddr>, pub peer:PeerConfig, pub mempool:MempoolConfig }

pub(crate) struct CoreHandle {
    tx: tokio::sync::mpsc::Sender<CoreEnvelope>,
    bytes: std::sync::Arc<tokio::sync::Semaphore>,
}
```

`OregonNode<V,T>` is generic with `V: SpendVerifier + Send + 'static` and `T: Transport`; no default permissive verifier exists.

- [ ] **Step 1: Write red core-queue/thread tests**

64 exact commands; byte budget exact 8 MiB; permit release on receive/drop; header command >16 rejected; test-only command proves execution thread differs from Tokio reactor task.

- [ ] **Step 2: Write red relay tests**

Rejected tx => no Inv; accepted tx => Inv; invalid block => no Inv; accepted sidechain => block Inv; active change reconciles mempool before tx service resumes; reconciliation failure rebuilds empty pool with saved `MempoolConfig` + new authoritative `ChainBase`.

- [ ] **Step 3: Implement one blocking core owner**

Use `mpsc::channel(64)` plus an 8-MiB semaphore. Sender acquires byte permits before enqueue. One `spawn_blocking` loop consumes with `blocking_recv()` and exclusively owns `ChainState`, `Mempool`, saved config, and verifier. Split remote header batches to <=16 before core commands.

- [ ] **Step 4: Implement `ChainSyncView` adapter via bounded core read commands**

Map chainstate failures to coarse `SyncViewError::Unavailable`; never export consensus/storage variants upward.

- [ ] **Step 5: Implement authoritative orchestration**

Build `ChainBase` only from `ChainState::tip()`. `Extended` => `reconcile_active_block`; `Reorganized` => `reconcile_reorg`; `StoredSideChain` => no active mempool-base change. If post-commit reconciliation unexpectedly fails, replace with `Mempool::new(new_base,saved_config.clone())`.

- [ ] **Step 6: Implement bounded inventory relay**

Per-peer known inventory cap 8,192 and recent relay 65,536 with FIFO generation eviction. Source peer is excluded. Register `Expect` before sending `GetData` to avoid response race.

Exact event order:
```text
Matched tx/block -> core acceptance -> coarse result -> relay authorization -> Inv
Matched headers -> chunks <=16 -> core accept_header -> sync refresh -> Expect object -> GetData
```

`StorageFaulted`/`ReindexRequired` stops new mutation/sync work and is not converted to peer blame.

- [ ] **Step 7: Verify and commit**

```bash
cargo test -p oregon-node --lib --all-targets
cargo fmt --all -- --check
cargo clippy -p oregon-node --all-targets -- -D warnings
git add crates/oregon-node
git commit -m "feat: orchestrate validated Oregon peer relay"
```

---

### Task 12: Prove three-node TCP sync, CI architecture boundaries, and mutations

**Files:** node integration tests/support, `.github/workflows/oregon-rust.yml`, dev-dependencies only when needed.

- [ ] **Step 1: Create exact test-only valid-chain helpers**

`tests/support/mod.rs` defines private `AcceptAllSpends` and a max-target `ChainConfig`. Valid blocks contain one coinbase: height 1 output index 0 is exact founder allocation/program; heights >1 may use zero miner outputs (underclaim is valid). Coinbase witness[0] is canonical height varint; header timestamp increments 300s; transaction root is canonical; max target makes every computed RandomX hash satisfy target while still executing the real PoW path.

- [ ] **Step 2: Add real loopback handshake tests**

Use `127.0.0.1:0`, independent temp DBs, same chain config. Prove A-B-C established, feature negotiation, simultaneous duplicate leaves one session, self nonce rejected.

- [ ] **Step 3: Add relay tests over TCP**

Block relay: accepted block propagates only after chainstate acceptance; invalid block does not. Transaction relay: prepare a spendable test UTXO using test-only chain fixture state (never a production insertion API), submit a valid tx through a matched requested response, and assert downstream mempool admission precedes relay; invalid/policy-rejected tx is not relayed.

- [ ] **Step 4: Add behind-node headers-first catch-up**

Prebuild accepted multi-block chain on A, start C behind, connect through B, assert C imports headers then bodies and reaches same active tip. Record scheduler snapshots proving <=8 per peer and <=32 global.

- [ ] **Step 5: Add fork/lying-height/timeout tests**

Competing fork: remote higher `best_height` cannot override chainstate preferred header result. Timeout: nonresponding peer loses ownership after 20s and another peer receives request. Three failures => `Stalled`.

- [ ] **Step 6: Extend CI scans exactly**

Add the M6 implementation branch to workflow push branches. Add:

```bash
if rg -n 'oregon-(protocol|network|peer|sync|node)' \
  crates/oregon-consensus/Cargo.toml crates/oregon-utxo/Cargo.toml \
  crates/oregon-storage/Cargo.toml crates/oregon-chainstate/Cargo.toml \
  crates/oregon-mempool/Cargo.toml; then
  echo 'Core crate depends upward on M6 networking.' >&2
  exit 1
fi

if rg -n 'Vec::with_capacity\((input_count|output_count|witness_count|transaction_count)\)' crates/oregon-primitives/src; then
  echo 'Remote decoder preallocates from attacker count.' >&2
  exit 1
fi

if rg -n 'FOUNDER_ALLOCATION_BASE_UNITS|block_subsidy|derive_randomx_key|CF_(BLOCKS|BLOCK_INDEX|UTXO|UNDO|CHAIN_META)' \
  crates/oregon-protocol crates/oregon-network crates/oregon-peer crates/oregon-sync; then
  echo 'Core rule leaked below oregon-node orchestration boundary.' >&2
  exit 1
fi
```

- [ ] **Step 7: Run full clean verification**

```bash
cargo test --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo rustdoc -p oregon-chainstate -- -D warnings
cargo doc --workspace --no-deps
```

All commands must exit 0 before any completion claim.

- [ ] **Step 8: Run five throwaway mutation experiments from the exact clean SHA**

1. bypass relay-before-validation -> relay test fails;
2. bypass before-allocation frame-size check -> oversized transport test fails;
3. permit gossip before handshake -> handshake test fails;
4. bypass sync in-flight cap -> exact 32/8 test fails;
5. use remote best height as chain preference -> fork/lying-height test fails.

Mutation code is never merged. Record mutation SHA + killed test for the later M6 checkpoint.

- [ ] **Step 9: Re-run exact clean SHA and commit only clean CI/test code**

Repeat Step 7; require all exit 0.

```bash
git add crates/oregon-node/tests .github/workflows/oregon-rust.yml Cargo.toml Cargo.lock crates/*/Cargo.toml
git commit -m "test: prove Oregon M6 peer synchronization"
```

Do not create the M6 acceptance checkpoint and do not merge `main`; both require separate review of the exact implementation SHA.

---

## Plan Self-Review

- Spec coverage: Tasks 1-12 map every protocol tag, frame/list limit, peer/queue/time bound, core queue bound, header slice, locator rule, request/retry/grace rule, scoring threshold, relay gate, and M6 end-to-end acceptance condition.
- No placeholders: no `TBD`, `TODO`, “similar to”, incomplete interface comment, or unnamed error-handling step remains.
- Type consistency: `HeaderTip`, `HeaderImportOutcome`, `PeerId`, `PeerEvent`, `RequestKey`, `ChainSyncView`, `SyncAction`, and `CoreHandle` are defined before downstream use.
- Persistence: preferred header tip is separately durable; active full-block tip is unchanged; 1.0 -> 1.1 is the only automatic migration added.
- Header-first correctness: downloaded body for `HeaderValidated/body_retained=false` cannot hit the old idempotent early return.
- Resource safety: remote count does not drive immediate allocation; payload length is checked before allocation; headers enter core in <=16 slices; queues are count+byte bounded; exactly three block attempts.
- Rule ownership: sync never receives `ChainWork`; protocol/network/peer/sync never import founder/emission/RandomX/storage internals; node only calls authoritative core owners.
- Spend verification: real networking code remains generic over a supplied verifier; permissive verifier exists only in test code.
