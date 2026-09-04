# Oregon M6 Network Architecture Design

**Status:** design approved in conversation; implementation pending written-spec review

**Date:** 2026-09-04

**Base:** `main` after the accepted M5 mempool checkpoint and Oregon architecture unification

**Design branch:** `design/m6-network-architecture-2026-09-04`

## 1. Purpose

M6 adds Oregon's first production peer-to-peer networking layer without changing accepted consensus, economic, UTXO, storage, chain-selection, durability, RandomX, or mempool-policy behavior.

The milestone succeeds when multiple Oregon nodes can establish real TCP connections, complete a bounded handshake, negotiate compatible protocol features, relay validated transactions and blocks, and allow a new node to synchronize from an existing peer using headers-first, fork-aware synchronization.

M6 is not a launch-readiness milestone. It deliberately excludes peer discovery and advanced transport/privacy features so the first P2P layer remains small, auditable, bounded, and replaceable.

## 2. Frozen invariants

The following rules are immutable during M6:

- The accepted `%5` founder allocation and all monetary/emission behavior remain frozen.
- RandomX validation, key scheduling, target/work rules, timestamp rules, transaction validity, UTXO transitions, maturity, reorganization limits, chain-selection rules, storage durability, and mempool policy remain owned by their current core crates.
- `oregon-consensus`, `oregon-utxo`, `oregon-storage`, `oregon-chainstate`, and `oregon-mempool` must not depend on any network crate.
- No network crate may implement, approximate, cache, or reinterpret a consensus, economic, UTXO, RandomX, chain-selection, or mempool-policy decision.
- A received block or transaction is never eligible for relay merely because it decoded successfully.
- All remotely influenced resources are bounded by count, bytes, time, or all three where applicable.
- `oregon-node` is a composition/orchestration boundary, not a second consensus or policy owner.

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

### `oregon-protocol`

Owns wire message definitions, frame encoding/decoding, protocol versions, feature negotiation data, message tags, protocol-level limits, and canonical parsing of network payloads. It depends on `oregon-primitives` so block and transaction bodies reuse the existing canonical Oregon object encodings.

It does not define a second block, transaction, block ID, transaction ID, chain-work, or validity format.

### `oregon-network`

Owns the transport abstraction and the first production transport, TCP. It owns listener/dial behavior, framed read/write plumbing, socket deadlines, shutdown, and transport-level errors. The public transport API must not expose TCP-specific details that would prevent a future QUIC implementation.

### `oregon-peer`

Owns connection lifecycle, handshake state, self-peer prevention, duplicate-connection arbitration, negotiated capabilities, bounded per-peer queues, liveness timeouts, misbehavior accounting, performance observations, and disconnect/cooldown decisions.

### `oregon-sync`

Owns synchronization intent: locator construction, `GetHeaders` scheduling, headers-first progress, block-body request scheduling, in-flight ownership, timeout/retry/reassignment, and synchronization peer preference. It does not decide header validity, cumulative work, active-chain preference, reorganization validity, or block validity.

Any view of the chain needed by sync is exposed through a consumer-owned sync interface. `oregon-node` adapts `ChainState` to that interface; core crates never depend upward on `oregon-sync`.

### `oregon-node`

Owns composition only. It starts core state and network services, translates peer events into core calls, translates authoritative core results into relay/sync feedback, coordinates mempool reconciliation after active-chain changes, and controls shutdown.

It must never copy consensus or mempool decisions into orchestration code.

## 4. Runtime and isolation

M6 uses Tokio internally for asynchronous TCP, timers, and bounded channels. Runtime-specific types remain inside `oregon-network`, `oregon-peer`, `oregon-sync`, and `oregon-node`; core crate public APIs remain synchronous and runtime-independent.

All M6 crates except existing RandomX FFI boundaries declare `#![forbid(unsafe_code)]`.

M6 bootstrap uses explicitly configured peer endpoints. DNS seeds, address gossip, DHT, UPnP, NAT traversal, Tor/I2P discovery, and persistent peer databases are out of scope.

## 5. Chain identity and network magic

Peers must reject cross-chain connections before normal gossip or synchronization begins.

`ChainState` exposes an opaque chain identity derived authoritatively from immutable `ChainConfig` inputs. The network stack receives only the resulting 32-byte identity; it never inspects founder, emission, RandomX, or chain-selection rules to construct identity itself.

The chain identity commits to the anchor header and immutable consensus parameters required to distinguish incompatible Oregon chains. Its exact canonical hash construction is owned below the network boundary and receives dedicated golden vectors.

The expected four-byte frame magic is deterministically derived from the opaque chain identity using a domain-separated BLAKE3 construction:

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

`payload_length` is checked against the hard frame limit before allocating or reading the payload.

### Frame limits

```text
MAX_FRAME_PAYLOAD       = 2 MiB
MAX_HANDSHAKE_PAYLOAD   = 4 KiB
MAX_INV_ITEMS           = 4,096
MAX_GETDATA_ITEMS       = 128
MAX_LOCATOR_HASHES      = 64
```

`MAX_FRAME_PAYLOAD` is a DoS/resource bound only. It is not Oregon's maximum valid block or transaction size. Canonical object decoding reuses `oregon-primitives`, and actual transaction/block validity remains in existing authoritative validation paths.

## 7. Message set

M6 protocol v1 contains only the messages required by the milestone:

- `Hello`
- `HelloAck`
- `Ping`
- `Pong`
- `Inv`
- `GetData`
- `GetHeaders`
- `Headers`
- `Transaction`
- `Block`

No generic remote error/reject string is required in M6. Local disconnect reasons remain structured local state and are not used as a new consensus vocabulary.

List lengths use Oregon's existing canonical varint rules. Fixed hashes reuse `Hash256`; block headers reuse the canonical 114-byte `BlockHeader` encoding; transaction and block bodies reuse the canonical primitive encodings.

## 8. Protocol version and feature negotiation

```text
PROTOCOL_VERSION_CURRENT = 1
PROTOCOL_VERSION_MIN     = 1
FRAME_VERSION            = 1
```

`Hello` advertises a minimum and maximum supported protocol version. Peers select the highest mutually supported version. No overlap fails closed.

Feature negotiation uses two 64-bit bitmaps:

```text
offered_features
required_features
```

M6 defines:

- `HEADERS_SYNC`
- `BLOCK_RELAY`
- `TX_RELAY`

Unknown optional features are ignored. A required feature unknown or unsupported by the remote peer rejects the handshake. Enabled features equal the mutually supported offered set after required-feature checks pass.

`HelloAck` echoes the selected protocol version, enabled feature bitmap, and the remote instance nonce so both sides prove they negotiated the same session.

## 9. Hello and handshake

`Hello` contains only bounded fixed-size data:

```text
Hello {
    min_protocol_version: u16,
    max_protocol_version: u16,
    chain_id: [u8; 32],
    instance_nonce: [u8; 16],
    offered_features: u64,
    required_features: u64,
    best_height: u64,
    best_block_id: Hash256,
}
```

`best_height` and `best_block_id` are synchronization hints only. They never authorize chain selection.

Handshake state is explicit:

```text
Connected
  -> HelloSent
  -> HelloReceived
  -> Negotiated
  -> AckSent/AckReceived
  -> Established
```

Only handshake, ping, pong, and shutdown processing is permitted before `Established`. Gossip and sync messages received early are protocol violations.

```text
HANDSHAKE_TIMEOUT       = 10 s
MAX_PENDING_HANDSHAKES  = 32
```

## 10. Self-peer and duplicate connection rules

Each node process creates a cryptographically random 128-bit `instance_nonce` at startup. A remote nonce equal to the local nonce is a self-peer connection and is closed.

Simultaneous A->B and B->A dialing is resolved deterministically:

```text
local_nonce < remote_nonce  => keep outbound, drop inbound
local_nonce > remote_nonce  => keep inbound, drop outbound
```

Both nodes therefore retain the same physical TCP connection without coordinator state.

A nonce collision between distinct processes is treated conservatively as self-peer and closed.

## 11. Peer and queue bounds

Production defaults and hard limits:

```text
DEFAULT_MAX_PEERS       = 64
DEFAULT_MAX_OUTBOUND    = 16
DEFAULT_MAX_INBOUND     = 48
HARD_MAX_PEERS          = 128

MAX_QUEUE_FRAMES_PEER   = 256
MAX_QUEUE_BYTES_PEER    = 4 MiB
MAX_QUEUE_BYTES_GLOBAL  = 64 MiB
```

Every outbound queue is bounded by both item count and bytes. No producer may convert a full bounded queue into an unbounded side buffer.

Queue classes:

- low-priority inventory gossip may be dropped when full;
- request/response traffic applies bounded backpressure and disconnects peers that cannot make progress;
- a small reserved control lane exists for ping, pong, handshake, and shutdown so block traffic cannot starve liveness/control traffic.

## 12. Time bounds

```text
FRAME_NO_PROGRESS_TIMEOUT = 15 s
MAX_FRAME_READ_DURATION   = 60 s
FRAME_WRITE_TIMEOUT       = 15 s
PING_INTERVAL             = 30 s
PONG_TIMEOUT              = 15 s
IDLE_TIMEOUT              = 120 s
```

The read no-progress timer resets only when bytes are actually received; absolute frame duration remains bounded at 60 seconds. A peer cannot keep a frame alive forever by trickling bytes.

Timeouts affect peer health/performance but do not become consensus invalidity.

## 13. Headers-first synchronization

Synchronization always discovers and validates headers before scheduling block bodies.

Flow:

```text
Headers from peer
  -> protocol structural checks
  -> oregon-node
  -> ChainState header-import boundary
  -> existing consensus header/PoW validation
  -> durable HeaderValidated index publication
  -> authoritative branch/work result
  -> opaque result returned to oregon-sync
  -> block-body scheduling
```

M6 adds network-independent chainstate APIs for header import and sync views. They must not reference peer, transport, gossip, or protocol types.

Header-only publication is durable before chainstate reports acceptance. Header import never changes the active full-block tip merely because a higher-work header branch exists.

The preferred header branch is selected by chainstate using authoritative cumulative work rules. `oregon-sync` consumes only the result.

### Header batch hardening

Because Oregon header validation includes RandomX work rather than a cheap hash-only check, a single remote header batch must not authorize an unbounded CPU burst.

The M6 written spec therefore hardens the earlier conversational draft from 2,000 headers per message to:

```text
MAX_HEADERS_PER_MESSAGE = 128
HEADER_VALIDATION_SLICE = 16
```

A 128-header response is structurally bounded, while validation is yielded in slices of at most 16 headers so one peer cannot monopolize the async node loop. This changes only a network/CPU resource bound; it does not skip validation or weaken proof-of-work.

This is the one intentional safety tightening discovered during written-spec self-review and requires explicit acceptance with this document before implementation.

## 14. Locator behavior

Locators start at the current preferred validated header tip:

- first 10 entries use step 1;
- subsequent steps double: 2, 4, 8, 16, ...;
- the chain anchor is always included if not already present;
- the list is capped at `MAX_LOCATOR_HASHES = 64`.

`GetHeaders` contains the locator and an optional stop hash. The responding node selects the highest locator entry it recognizes on its authoritative validated-header view and returns following headers up to the message limit.

A response is rejected if its headers are not contiguous, the first returned header does not attach to the selected common ancestor, or structural limits are violated.

## 15. Fork-aware behavior

Remote `best_height` is never trusted as chain preference. The only valid path from remote data to preferred work is:

```text
remote headers
  -> authoritative header validation
  -> chainstate branch/index state
  -> authoritative cumulative-work comparison
```

A peer advertising an absurd height can cause at most a bounded sync attempt; it cannot alter active-chain state or create chain work.

The active full-block chain remains governed by existing chainstate acceptance and reorganization logic. If the existing deep-reorg policy reaches `ReindexRequired`, node orchestration stops block mutation/sync work that would require further acceptance and surfaces the authoritative chainstate health state. Network code does not reinterpret the reorg limit.

## 16. Block scheduler

After a preferred validated header path identifies missing bodies, sync schedules bounded parallel downloads:

```text
MAX_IN_FLIGHT_BLOCKS_GLOBAL = 32
MAX_IN_FLIGHT_BLOCKS_PEER   = 8
MAX_BUFFERED_BLOCKS         = 32
BLOCK_REQUEST_TIMEOUT       = 20 s
MAX_BLOCK_RETRIES           = 3
```

No single peer can own more than eight in-flight block requests. With several eligible peers, the scheduler distributes work based on capability, misbehavior state, and measured request performance.

Blocks may arrive out of order, but completed bodies waiting for predecessors are kept only within `MAX_BUFFERED_BLOCKS`. The node submits bodies to authoritative chainstate in a controlled order derived from the validated header plan.

A timed-out request releases ownership and may be reassigned to another eligible peer. After three failed attempts across scheduling opportunities, sync enters `Stalled` for that target and reports the condition to node orchestration rather than retrying forever.

## 17. Gossip and relay authorization

M6 uses inventory-first gossip rather than unsolicited full-object flooding:

```text
accepted transaction -> Inv(TxId) -> GetData -> Transaction
accepted block       -> Inv(BlockId) -> GetData -> Block
```

Relay authorization is explicit.

### Transaction path

```text
received Transaction
  -> canonical decode
  -> oregon-node
  -> current Mempool admission using current chain base
  -> accepted
  -> relay eligible
```

A transaction rejected by mempool is not relayed. Policy/context outcomes such as duplicate, capacity rejection, or missing dependency do not automatically become peer misbehavior because peer mempool state can legitimately differ.

### Block path

```text
received Block
  -> canonical decode
  -> oregon-node
  -> ChainState::accept_block(...)
  -> authoritative acceptance result
  -> active-chain mempool reconciliation when required
  -> relay eligible
```

`StoredSideChain` blocks may be announced after authoritative acceptance even though they did not become active. `Extended` and `Reorganized` outcomes trigger mempool reconciliation against the newly active chain before normal transaction admission/relay resumes.

If mempool reconciliation unexpectedly fails after durable chainstate acceptance, chainstate is not rolled back. `oregon-node` replaces the in-memory mempool with a new empty pool using the same validated M5 configuration and the new authoritative chain base, then resumes transaction service. Mempool is policy-only and in-memory; stale mempool state is never allowed to remain live against a changed active chain.

No network module decides whether a block is valid enough to relay.

## 18. Gossip deduplication

```text
MAX_KNOWN_INVENTORY_PER_PEER = 8,192
MAX_RECENT_RELAY_CACHE       = 65,536
```

Known-inventory and recent-relay structures use deterministic bounded eviction. No unbounded `HashSet` of historical inventory is permitted.

The source peer is not immediately re-advertised the same object. Duplicate inventory advertisements are cheap no-ops once bounded dedup state recognizes them.

Unsolicited full objects are accepted only within a small bounded allowance required for race tolerance; repeated unsolicited-object behavior becomes peer misbehavior and cannot bypass queue or frame limits.

## 19. Peer scoring model

M6 separates security misbehavior from performance. A fast peer is not considered safe merely because it is fast.

### Misbehavior points

```text
malformed frame                +25
oversized frame                immediate disconnect
handshake violation            +25
invalid response shape         +10
unsolicited object flood       +10
request abuse                  +10
repeated timeout               +5
objectively invalid header     +50
objectively invalid block      +50
```

Thresholds:

```text
50  -> peer is no longer eligible for new sync requests
100 -> disconnect
```

Consensus details are not exported upward. Node translates authoritative outcomes into coarse feedback such as `InvalidHeader` or `InvalidBlock`; peer code never receives or branches on founder-allocation, emission, RandomX, UTXO, maturity, or chain-selection error variants.

### Performance observations

The scheduler may observe response latency, successful requests, and timeouts. Performance influences request preference only. It never overrides misbehavior state or validity.

### Cooldown

```text
DISCONNECT_COOLDOWN   = 10 min
MAX_COOLDOWN_ENTRIES  = 1,024
```

Cooldown keys use canonical remote IP identity rather than ephemeral inbound source ports. IPv4-mapped IPv6 addresses are normalized before keying. M6 does not implement subnet bans or a persistent ban database.

## 20. Failure handling

- Oversized payload length is rejected before allocation.
- Unsupported frame/protocol versions fail closed.
- Unsupported required features fail the handshake.
- Handshake timeout closes the pending connection.
- Queue overflow never creates an unbounded fallback queue.
- Slow/trickling frames hit progress or absolute duration limits.
- Sync request timeout releases in-flight ownership before reassignment.
- Invalid remote data cannot alter active state until the relevant authoritative core owner accepts it.
- Storage failure and `ReindexRequired` remain authoritative chainstate faults and are surfaced upward unchanged in meaning.
- Node shutdown drains only bounded control work; it is not required to flush gossip.

## 21. Test strategy

Every new crate receives unit/property tests for its own boundary plus integration tests spanning real loopback TCP nodes.

Required coverage includes:

### Protocol

- golden frame vectors
- checksum corruption
- wrong network magic
- non-zero v1 flags
- truncated headers/payloads
- payload length before-allocation rejection
- canonical varint enforcement
- exact list limits
- unknown optional vs required features
- protocol overlap/no-overlap
- canonical block/transaction round-trip reuse

### Peer

- handshake state transitions
- handshake timeout
- self-peer detection
- simultaneous duplicate arbitration from both perspectives
- pending-handshake cap
- per-peer/global queue count and byte caps
- control-lane availability under block pressure
- ping/pong and idle timeout
- scoring thresholds and cooldown bounds

### Sync

- locator exact stepping and cap
- contiguous header response requirement
- 128-header exact boundary and 129 rejection
- 16-header validation slicing
- lying best-height hint does not select a chain
- fork competition follows authoritative chainstate work result
- global/per-peer in-flight exact boundaries
- timeout release and reassignment
- three-retry stall
- out-of-order block buffering exact boundary

### Relay

- received-but-rejected tx is never relayed
- received-but-invalid block is never relayed
- accepted side-chain block can be announced without active-chain mutation
- active-block acceptance reconciles mempool before tx service resumes
- reconciliation failure resets to an empty pool on the new chain base
- inventory dedup exact bounds

### End-to-end

At least three loopback Oregon nodes must demonstrate:

1. real TCP connection and handshake;
2. feature negotiation;
3. duplicate/self protection;
4. transaction propagation only after mempool acceptance;
5. block propagation only after chainstate acceptance;
6. a node starting behind and catching up through headers-first sync;
7. competing forks where local authoritative work selection wins over remote height claims;
8. peer timeout causing block-request reassignment;
9. all network resources remaining within configured bounds under stress.

The standard Oregon workspace gate remains mandatory:

```bash
cargo test --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

M6 also requires mutation tests proving at minimum that the suite kills deliberate bypasses of: relay-before-validation, frame-size enforcement, handshake-required messaging, sync in-flight caps, and authoritative fork/work selection.

## 22. Explicit non-goals

M6 does not implement:

- QUIC transport
- NAT traversal or UPnP
- DNS seeds
- peer address gossip
- DHT discovery
- Tor/I2P integration
- persistent bans
- compact blocks
- erasure coding
- encrypted application framing beyond the transport provided in M6
- RBF
- package relay or CPFP scoring
- orphan transaction retention
- mempool persistence
- wallet/address protocol
- mining RPC
- testnet/mainnet launch readiness

These remain future milestones and may not be pre-scaffolded with dead production APIs in M6.

## 23. Acceptance criteria

M6 is acceptable only when all of the following are true:

- five new crates respect the dependency direction in this document;
- existing core crates have no upward network dependencies;
- all transport, queue, handshake, message, inventory, request, buffering, and retry resources are bounded;
- protocol v1 compatibility behavior is deterministic and fail-closed where required;
- TCP nodes establish and maintain real peer sessions;
- transaction and block relay occurs only after authoritative validation/admission;
- headers-first sync validates proof-of-work through existing consensus ownership;
- fork choice is never based on remote height alone;
- a fresh/behind node can synchronize to an existing peer;
- storage faults and deep-reorg `ReindexRequired` behavior remain unchanged;
- `%5` founder allocation and every other accepted economic/consensus rule remain unchanged;
- full workspace tests, formatting, clippy, end-to-end network tests, and required M6 security mutations pass on the reviewed commit.

No checkpoint is created and `main` is not integrated merely because the design or implementation branch exists. M6 acceptance requires a separately reviewed implementation checkpoint under the existing Oregon engineering constitution.
