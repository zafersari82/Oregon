# Oregon M6 Network Architecture Design

**Status:** design approved in conversation; implementation pending written-spec review

**Date:** 2026-09-04

**Base:** `main` after the accepted M5 mempool checkpoint and Oregon architecture unification

**Design branch:** `design/m6-network-architecture-2026-09-04`

## 1. Purpose

M6 adds Oregon's first peer-to-peer networking subsystem without changing accepted consensus, economic, UTXO, storage, chain-selection, durability, RandomX, or mempool-policy behavior.

The milestone succeeds when multiple Oregon node instances can establish real TCP connections, complete a bounded handshake, negotiate compatible protocol features, relay validated transactions and blocks, and allow a behind/new test node to synchronize from peers using headers-first, fork-aware synchronization.

M6 is not a launch-readiness milestone. It deliberately excludes peer discovery, production spend-authorization cryptography, and advanced transport/privacy features so the first P2P layer remains small, auditable, bounded, and replaceable.

## 2. Frozen invariants

The following rules are immutable during M6:

- The accepted `%5` founder allocation and all monetary/emission behavior remain frozen.
- RandomX validation, key scheduling, target/work rules, timestamp rules, transaction validity, UTXO transitions, maturity, reorganization limits, chain-selection rules, storage durability, and mempool policy remain owned by their current core crates.
- `oregon-primitives`, `oregon-pow`, `oregon-consensus`, `oregon-utxo`, `oregon-storage`, `oregon-chainstate`, and `oregon-mempool` must not depend on any M6 network crate.
- No network crate may implement, approximate, cache, or reinterpret a consensus, economic, UTXO, RandomX, chain-selection, or mempool-policy decision.
- A received block or transaction is never eligible for relay merely because it decoded successfully.
- All remotely influenced resources are bounded by count, bytes, time, or all three where applicable.
- `oregon-node` is a composition/orchestration boundary, not a second consensus or policy owner.
- No production path may substitute an accept-all or test-only `SpendVerifier`.

## 3. Crate architecture

M6 adds five crates:

```text
oregon-primitives
       ^
oregon-protocol
       ^
oregon-network
       ^
oregon-peer
       ^
oregon-sync
       ^
   oregon-node
       |
       +-- oregon-chainstate
       +-- oregon-mempool
```

The diagram expresses allowed dependency direction from higher-level orchestration toward lower-level facilities. Existing core direction remains unchanged.

Allowed direct Oregon-crate dependencies are:

```text
oregon-protocol -> oregon-primitives
oregon-network  -> oregon-protocol
oregon-peer     -> oregon-network, oregon-protocol
oregon-sync     -> oregon-peer, oregon-protocol, oregon-primitives
oregon-node     -> oregon-sync, oregon-peer, oregon-network, oregon-protocol,
                   oregon-chainstate, oregon-mempool, oregon-primitives, oregon-utxo
```

No other upward Oregon dependency is permitted without a new architecture review.

### `oregon-protocol`

Owns wire message definitions, frame encoding/decoding, protocol versions, feature negotiation data, message tags, protocol-level limits, and canonical parsing of network payloads. Block and transaction bodies reuse the existing canonical Oregon object encodings.

It does not define a second block, transaction, block ID, transaction ID, chain-work, or validity format.

### `oregon-network`

Owns the transport abstraction and the first production transport, TCP. It owns listener/dial behavior, framed read/write plumbing, socket deadlines, shutdown, and transport-level errors. The public transport API must not expose TCP-specific details that would prevent a future QUIC implementation.

### `oregon-peer`

Owns connection lifecycle, handshake state, self-peer prevention, duplicate-connection arbitration, negotiated capabilities, bounded per-peer queues, liveness timeouts, misbehavior accounting, performance observations, and disconnect/cooldown decisions.

### `oregon-sync`

Owns synchronization intent: locator construction, `GetHeaders` scheduling, headers-first progress, block-body request scheduling, in-flight ownership, timeout/retry/reassignment, and synchronization peer preference. It does not decide header validity, cumulative work, active-chain preference, reorganization validity, or block validity.

Any chain view needed by sync is exposed through a trait owned by `oregon-sync`. `oregon-node` implements that trait on a local adapter around `ChainState`; core crates never depend upward on `oregon-sync`.

### `oregon-node`

Owns composition only. It starts core state and network services, translates peer events into core calls, translates authoritative core results into coarse relay/sync feedback, coordinates mempool reconciliation after active-chain changes, and controls shutdown.

It must never copy consensus or mempool decisions into orchestration code.

## 4. Runtime, core executor, and isolation

M6 uses Tokio internally for asynchronous TCP, timers, and bounded channels. Runtime-specific types remain inside the five new M6 crates; core crate public APIs remain synchronous and runtime-independent.

All five new M6 crates declare `#![forbid(unsafe_code)]`. Existing `oregon-pow` remains the only permitted unsafe/FFI boundary under the Oregon engineering constitution.

Core mutation/validation calls are serialized through one `oregon-node` core worker so async socket tasks never concurrently mutate `ChainState` or `Mempool` and never run RandomX/core validation directly on a Tokio reactor thread.

```text
MAX_CORE_COMMANDS      = 64
MAX_CORE_COMMAND_BYTES = 8 MiB
```

The core command queue is bounded by both items and bytes. Header work is broken into slices so the worker can yield between bounded units of work.

M6 bootstrap uses explicitly configured peer endpoints. DNS seeds, address gossip, DHT, UPnP, NAT traversal, Tor/I2P discovery, and persistent peer databases are out of scope.

M6 does not add a launch-ready daemon that silently uses permissive spend verification. Real TCP end-to-end tests use an explicit test-only `SpendVerifier` under test configuration. A production executable may expose transaction/block acceptance only when a production `SpendVerifier` exists; otherwise it must fail closed rather than weaken the existing verifier boundary.

## 5. Chain identity and network magic

M6 has one simple chain identity rule:

```text
chain_id = ChainConfig.anchor_header.block_id()
```

`oregon-node` derives this from the canonical existing block-header identity and passes the 32-byte value downward as opaque network identity. No network crate inspects founder allocation, emission, RandomX, UTXO, or chain-selection configuration.

Future incompatible Oregon networks must use a distinct anchor header. Testnet/mainnet network identity policy beyond that is outside M6.

The expected four-byte frame magic is:

```text
first4(BLAKE3("OREGON/NETMAGIC/V1\0" || chain_id))
```

This is transport identity, not a validity rule.

## 6. Frame format

Protocol v1 uses a fixed 16-byte frame header:

```text
+----------------------+----------+
| network_magic        | 4 bytes  |
| frame_version        | 1 byte   |
| message_type         | 1 byte   |
| flags                | 2 bytes  |
| payload_length       | 4 bytes  |
| checksum             | 4 bytes  |
+----------------------+----------+
| payload              | N bytes  |
+----------------------+----------+
```

All integer fields are little-endian. `FRAME_VERSION = 1`. Protocol-v1 flags must be zero; non-zero flags are rejected rather than guessed.

The checksum is the first four bytes of:

```text
BLAKE3("OREGON/FRAME/V1\0" || header_without_checksum || payload)
```

The checksum detects corruption; M6 plaintext TCP does not provide cryptographic peer authentication or protection from an active man-in-the-middle. No security decision may treat the checksum as authentication.

`payload_length` is checked against the hard frame limit before allocating or reading the payload.

### Frame/list limits

```text
MAX_FRAME_PAYLOAD       = 2 MiB
MAX_HANDSHAKE_PAYLOAD   = 4 KiB
MAX_INV_ITEMS           = 4,096
MAX_GETDATA_ITEMS       = 128
MAX_LOCATOR_HASHES      = 64
```

`MAX_FRAME_PAYLOAD` is a DoS/resource bound only. It is not Oregon's maximum valid block or transaction size. Canonical object decoding reuses `oregon-primitives`; actual transaction/block validity remains in existing authoritative validation paths.

Before remote exposure, canonical primitive decoders used by M6 must not preallocate vectors directly from an untrusted declared element count. They must grow only as elements are successfully decoded or reserve from a bound proven by already-available payload bytes. This is allocation hardening only; canonical bytes and accepted/rejected object semantics remain unchanged.

## 7. Exact message tags and payloads

Protocol-v1 message tags are frozen as:

```text
0x01 Hello
0x02 HelloAck
0x03 Ping
0x04 Pong
0x10 Inv
0x11 GetData
0x20 GetHeaders
0x21 Headers
0x30 Transaction
0x31 Block
```

Unknown message tags are rejected.

`Hello` is exactly 108 payload bytes:

```text
u16 min_protocol_version
u16 max_protocol_version
[32] chain_id
[16] instance_nonce
u64 offered_features
u64 required_features
u64 best_height
[32] best_block_id
```

`HelloAck` is exactly 26 payload bytes:

```text
u16 selected_protocol_version
u64 enabled_features
[16] remote_nonce_echo
```

`Ping` and `Pong` each contain one `u64` nonce.

Inventory item encoding is exactly:

```text
u8 kind     // 0x01 transaction, 0x02 block
[32] hash
```

`Inv` and `GetData` contain a canonical varint item count followed by items. Unknown inventory kinds are rejected.

`GetHeaders` contains a canonical varint locator count, locator hashes in order, a one-byte stop-present flag (`0` or `1` only), and an optional 32-byte stop hash.

`Headers` contains a canonical varint count followed by canonical 114-byte block headers.

`Transaction` and `Block` payloads are exactly the existing canonical primitive object bytes, with no wrapper serialization inside the frame payload.

No generic remote error/reject string is required in M6.

## 8. Protocol version and feature negotiation

```text
PROTOCOL_VERSION_CURRENT = 1
PROTOCOL_VERSION_MIN     = 1
FRAME_VERSION            = 1
```

Feature bits are:

```text
HEADERS_SYNC = 1 << 0
BLOCK_RELAY  = 1 << 1
TX_RELAY     = 1 << 2
```

Peers select the highest mutually supported protocol version. No version overlap fails closed.

`offered_features` and `required_features` are 64-bit bitmaps. Unknown optional features are ignored. A required feature unknown or unsupported by the remote peer rejects the handshake. Enabled features are the mutually supported offered set after required-feature checks pass.

`HelloAck` must match the locally computed selected version/features and echo the recipient's nonce. A mismatch fails the handshake.

## 9. Hello and handshake state

`best_height` and `best_block_id` in `Hello` are synchronization hints only. They never authorize chain selection.

Handshake state is explicit:

```text
Connected
  -> HelloSent
  -> HelloReceived
  -> Negotiated
  -> AckSent/AckReceived
  -> Established
```

Only `Hello`, `HelloAck`, `Ping`, and `Pong` are permitted before `Established`. Gossip/sync messages received early are handshake violations.

```text
HANDSHAKE_TIMEOUT      = 10 s
MAX_PENDING_HANDSHAKES = 32
```

Each node process creates its 128-bit `instance_nonce` from the operating system CSPRNG at startup.

## 10. Self-peer and duplicate connection rules

A remote nonce equal to the local nonce is treated as self-peer and closed.

Simultaneous A->B and B->A dialing is resolved deterministically:

```text
local_nonce < remote_nonce  => keep outbound, drop inbound
local_nonce > remote_nonce  => keep inbound, drop outbound
```

Both nodes therefore retain the same physical TCP connection. A random nonce collision between distinct processes is conservatively treated as self-peer.

## 11. Peer and queue bounds

```text
DEFAULT_MAX_PEERS       = 64
DEFAULT_MAX_OUTBOUND    = 16
DEFAULT_MAX_INBOUND     = 48
HARD_MAX_PEERS          = 128

MAX_QUEUE_FRAMES_PEER   = 256
MAX_QUEUE_BYTES_PEER    = 4 MiB
MAX_QUEUE_BYTES_GLOBAL  = 64 MiB
CONTROL_RESERVED_FRAMES = 16
CONTROL_RESERVED_BYTES  = 64 KiB
QUEUE_ENQUEUE_TIMEOUT   = 2 s
```

Configured inbound + outbound peer limits must not exceed the configured total, and total must not exceed `HARD_MAX_PEERS`.

Per-peer frame/byte limits apply independently to inbound and outbound application queues; the global byte limit covers all such queues together. No producer may create an unbounded side buffer.

The control reservation is inside the per-peer caps and counted against the global cap. It is reserved for handshake, ping, pong, and shutdown/control events so data traffic cannot starve liveness.

Low-priority inventory may be dropped when a data queue is full. Required request/response or control enqueue waits at most `QUEUE_ENQUEUE_TIMEOUT`; failure to make progress closes the peer rather than accumulating memory.

## 12. Time bounds

```text
FRAME_NO_PROGRESS_TIMEOUT = 15 s
MAX_FRAME_READ_DURATION   = 60 s
FRAME_WRITE_TIMEOUT       = 15 s
PING_INTERVAL             = 30 s
PONG_TIMEOUT              = 15 s
IDLE_TIMEOUT              = 120 s
RESPONSE_START_TIMEOUT    = 20 s
```

The frame no-progress timer resets only when bytes are actually received. Absolute frame duration remains 60 seconds, so trickle traffic cannot keep a frame alive indefinitely.

`RESPONSE_START_TIMEOUT` bounds how long an outstanding request may wait for the matching response frame to begin. Once a matching frame begins, normal frame progress/absolute read deadlines govern completion.

Timeouts affect peer health/performance; they do not become consensus invalidity.

## 13. Headers-first synchronization

Synchronization discovers and validates headers before scheduling block bodies:

```text
Headers from peer
  -> protocol structural checks
  -> oregon-node core worker
  -> network-independent ChainState header-import boundary
  -> existing consensus header/PoW validation
  -> durable HeaderValidated index publication
  -> authoritative branch/work result
  -> coarse result to oregon-sync
  -> block-body scheduling
```

Header-only publication is durable before chainstate reports acceptance. A higher-work header branch does not itself mutate the active full-block tip.

The preferred validated header branch is selected below sync using authoritative cumulative-work rules. `oregon-sync` consumes only that result.

### Header CPU hardening

Oregon headers require RandomX work. The earlier conversational draft's 2,000-header batch is therefore tightened in the written design to:

```text
MAX_HEADERS_PER_MESSAGE = 128
HEADER_VALIDATION_SLICE = 16
```

A response may contain at most 128 headers. The core worker validates at most 16 sequential headers per work slice before yielding to its bounded command scheduler. No header is trusted or skipped; this changes only remote CPU scheduling/resource exposure.

## 14. Locator behavior

Locators start at the current preferred validated header tip:

- first 10 entries use step 1;
- subsequent steps double: 2, 4, 8, 16, ...;
- the chain anchor is always included if not already present;
- the list is capped at `MAX_LOCATOR_HASHES = 64`.

The responding node selects the highest locator entry it recognizes on its authoritative validated-header view and returns following headers up to the message limit.

A `Headers` response is accepted for processing only if it corresponds to an outstanding request, its headers are contiguous, its first header attaches to the selected common ancestor, and all protocol limits pass.

## 15. Fork-aware behavior

Remote height is never trusted as chain preference. The only valid path from remote data to preferred work is:

```text
remote headers
  -> authoritative header validation
  -> chainstate branch/index state
  -> authoritative cumulative-work comparison
```

An absurd remote height can cause at most a bounded synchronization probe; it cannot alter active state or manufacture work.

The active full-block chain remains governed by existing chainstate acceptance/reorganization logic. If existing deep-reorg policy reaches `ReindexRequired`, node orchestration stops acceptance/sync mutation work and surfaces the authoritative chainstate health state. Network code does not reinterpret the reorg limit.

## 16. Block scheduler

```text
MAX_IN_FLIGHT_BLOCKS_GLOBAL = 32
MAX_IN_FLIGHT_BLOCKS_PEER   = 8
MAX_BUFFERED_BLOCKS         = 32
MAX_BLOCK_ATTEMPTS          = 3
```

No peer owns more than eight block requests. Eligible peers share work according to capability, misbehavior state, and measured request performance.

Blocks may arrive out of order, but completed bodies waiting for predecessors are bounded by `MAX_BUFFERED_BLOCKS`. Bodies are submitted to authoritative chainstate in the order required by the validated header plan.

If a matching response frame does not begin within `RESPONSE_START_TIMEOUT`, in-flight ownership is released before reassignment. A block receives at most `MAX_BLOCK_ATTEMPTS` total peer attempts. After the third failed attempt, that target enters `Stalled` and sync reports the condition rather than retrying forever.

Recently timed-out object hashes are retained only to avoid falsely punishing a late response:

```text
EXPIRED_REQUEST_GRACE                    = 30 s
MAX_RECENTLY_EXPIRED_REQUESTS_PER_PEER   = 128
```

A late response matching this bounded grace set is discarded without misbehavior points. It is not reinserted into active in-flight state.

## 17. Gossip and relay authorization

M6 uses inventory-first gossip:

```text
accepted transaction -> Inv(TxId) -> GetData -> Transaction
accepted block       -> Inv(BlockId) -> GetData -> Block
```

A full `Transaction`, `Block`, or `Headers` payload without a matching outstanding request is discarded. If it is not a bounded late-response grace case, it is an unsolicited-object protocol offense.

### Transaction path

```text
received requested Transaction
  -> canonical decode
  -> oregon-node core worker
  -> current Mempool admission using authoritative chain base
  -> accepted
  -> relay eligible
```

A mempool-rejected transaction is not relayed. Duplicate, capacity, missing-dependency, and other contextual/policy outcomes do not automatically become peer misbehavior because peer mempool state may legitimately differ.

### Block path

```text
received requested Block
  -> canonical decode
  -> oregon-node core worker
  -> ChainState::accept_block(...)
  -> authoritative acceptance result
  -> active-chain mempool reconciliation when required
  -> relay eligible
```

An authoritatively accepted `StoredSideChain` block may be announced even if it does not become active. `Extended` and `Reorganized` outcomes reconcile the mempool against the new active chain before normal transaction admission/relay resumes.

If mempool reconciliation unexpectedly fails after durable chainstate acceptance, chainstate is not rolled back. `oregon-node` replaces the in-memory mempool with a new empty pool using the same validated M5 configuration and the new authoritative chain base. Stale mempool state is never left live against a changed chain.

No network module decides whether a block is valid enough to relay.

## 18. Gossip deduplication

```text
MAX_KNOWN_INVENTORY_PER_PEER = 8,192
MAX_RECENT_RELAY_CACHE       = 65,536
```

Both stores use FIFO-by-generation bounded eviction. The oldest generation is evicted first once the exact cap would be exceeded. No unbounded historical inventory set is permitted.

The source peer is not immediately re-advertised the same object. Duplicate inventory advertisements recognized by current bounded state are cheap no-ops.

## 19. Peer scoring model

Security misbehavior and performance are separate axes.

### Misbehavior points

```text
malformed frame                +25
oversized frame                immediate disconnect
handshake violation            +25
invalid response shape         +10
unsolicited object             +10
request abuse                  +10
sync request timeout           +5
objectively invalid header     +50
objectively invalid block      +50
```

Thresholds:

```text
50  -> no new sync requests assigned to peer
100 -> disconnect
```

Node translates authoritative outcomes into coarse feedback such as `InvalidHeader` or `InvalidBlock`. Peer code never receives or branches on founder-allocation, emission, RandomX, UTXO, maturity, or chain-selection error variants.

Performance observations such as successful responses, integer response latency, and timeouts influence scheduler preference only; they never override validity or misbehavior state.

### Cooldown

```text
DISCONNECT_COOLDOWN  = 10 min
MAX_COOLDOWN_ENTRIES = 1,024
```

Cooldown keys use canonical remote IP identity rather than ephemeral source ports. IPv4-mapped IPv6 addresses are normalized. When the cap is full, the entry with the earliest expiry is evicted first. M6 has no subnet ban or persistent ban database.

## 20. Failure handling

- Oversized payload length is rejected before allocation.
- Unsupported frame/protocol versions fail closed.
- Unsupported required features fail the handshake.
- Handshake timeout closes the pending connection.
- Queue overflow never creates an unbounded fallback queue.
- Slow/trickling frames hit progress or absolute duration limits.
- Request timeout releases in-flight ownership before reassignment.
- Invalid remote data cannot alter active state until the relevant authoritative core owner accepts it.
- Storage failure and `ReindexRequired` remain authoritative chainstate faults and are surfaced upward unchanged in meaning.
- A missing production `SpendVerifier` cannot be replaced by a permissive implementation to make networking appear operational.
- Shutdown drains only bounded control work; gossip need not be flushed.

## 21. Test strategy

Every new crate receives unit/property tests for its boundary plus integration tests spanning real loopback TCP node instances.

### Protocol

- golden frame/message vectors including exact numeric tags
- checksum corruption
- wrong network magic
- non-zero v1 flags
- truncated headers/payloads
- payload length before-allocation rejection
- canonical varint enforcement
- exact list limits
- unknown optional vs required features
- protocol overlap/no-overlap
- exact `Hello`/`HelloAck` lengths
- canonical block/transaction encoding reuse
- remote decode allocation hardening for extreme declared counts with tiny payloads

### Peer

- handshake state transitions and timeout
- self-peer detection
- simultaneous duplicate arbitration from both perspectives
- pending-handshake cap
- inbound/outbound per-peer and global queue count/byte caps
- exact control reservation under block pressure
- enqueue timeout
- ping/pong and idle timeout
- exact scoring thresholds
- deterministic bounded cooldown eviction

### Sync

- locator stepping and cap
- unsolicited headers rejection
- contiguous header response requirement
- 128-header exact boundary and 129 rejection
- 16-header validation slicing
- lying best-height hint does not select a chain
- fork competition follows authoritative chainstate work result
- global/per-peer in-flight boundaries
- timeout release, late-response grace, and reassignment
- exact three-attempt stall
- out-of-order block buffering boundary

### Relay

- unsolicited full objects are discarded and scored
- received-but-rejected tx is never relayed
- received-but-invalid block is never relayed
- accepted side-chain block can be announced without active-chain mutation
- active-chain acceptance reconciles mempool before tx service resumes
- reconciliation failure resets to an empty pool on the new chain base
- inventory dedup exact caps/FIFO eviction

### End-to-end

At least three loopback Oregon node instances must demonstrate:

1. real TCP connection and handshake;
2. feature negotiation;
3. duplicate/self protection;
4. transaction propagation only after mempool acceptance;
5. block propagation only after chainstate acceptance;
6. a behind node catching up through headers-first sync;
7. competing forks where authoritative work selection defeats remote height claims;
8. peer timeout causing block-request reassignment;
9. all application queues/request tables/buffers staying within configured bounds under stress.

End-to-end spend authorization uses a test-only verifier explicitly scoped to tests; the test harness must not create a production export of that verifier.

The standard Oregon workspace gate remains mandatory:

```bash
cargo test --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

M6 also requires mutation tests proving the suite kills deliberate bypasses of at least:

- relay-before-validation;
- frame-size enforcement;
- handshake-required messaging;
- sync in-flight caps;
- authoritative fork/work selection;
- unbounded decode preallocation reintroduction;
- test-only/permissive spend-verifier exposure to production code.

## 22. Explicit non-goals

M6 does not implement:

- QUIC transport
- TLS or application-layer peer authentication/encryption
- NAT traversal or UPnP
- DNS seeds
- peer address gossip
- DHT discovery
- Tor/I2P integration
- persistent bans
- compact blocks
- erasure coding
- RBF
- package relay or CPFP scoring
- orphan transaction retention
- mempool persistence
- wallet/address protocol
- mining RPC
- production spend-authorization cryptography beyond the existing `SpendVerifier` boundary
- testnet/mainnet launch readiness

These remain future milestones and may not be pre-scaffolded with dead production APIs in M6.

## 23. Written-spec self-review decisions

The written pass found and resolved four issues before implementation:

1. The conversational `MAX_HEADERS_PER_MESSAGE = 2,000` was too large for RandomX-backed header validation. It is tightened to 128, with validation work sliced at 16 headers.
2. Existing canonical decode code must not preallocate proportional to attacker-declared counts before bytes are validated. M6 requires allocation hardening without changing canonical encoding or validity semantics.
3. Retry wording was ambiguous. M6 now defines exactly three total block attempts, plus a bounded late-response grace table.
4. M5 deliberately has no production spend-authorization implementation beyond the `SpendVerifier` boundary. M6 therefore forbids disguising test verifiers as production dependencies; real TCP acceptance tests stay test-scoped.

These are safety/clarity hardenings, not changes to Oregon consensus or economics.

## 24. Acceptance criteria

M6 is acceptable only when all of the following are true:

- five new crates respect the exact dependency direction in this document;
- existing core crates have no upward network dependencies;
- all transport, queue, handshake, message, inventory, request, core-command, buffering, and retry resources are bounded;
- protocol-v1 numeric tags and compatibility behavior are deterministic;
- TCP node instances establish and maintain real peer sessions;
- transaction and block relay occurs only after authoritative validation/admission;
- headers-first sync validates proof-of-work through existing consensus ownership;
- fork choice is never based on remote height alone;
- a behind node can synchronize to an existing peer;
- no remote decode path performs attacker-count-sized preallocation before validating available bytes;
- storage faults and deep-reorg `ReindexRequired` behavior remain unchanged;
- no test-only permissive spend verifier is exported into production;
- `%5` founder allocation and every other accepted economic/consensus rule remain unchanged;
- full workspace tests, formatting, clippy, end-to-end network tests, and required M6 security mutations pass on the reviewed implementation commit.

No checkpoint is created and `main` is not integrated merely because this design or a later implementation branch exists. M6 acceptance requires a separately reviewed implementation checkpoint under the Oregon engineering constitution.
