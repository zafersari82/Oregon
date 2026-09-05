# Oregon Engineering Constitution

**Applies to:** all current and future Oregon protocol work

**Accepted baseline:** M6 networking and node-orchestration checkpoint

**Effective date:** 2026-09-05

## 1. One authoritative rule

Every externally observable rule has exactly one implementation owner. Callers reuse that owner; they do not copy, approximate, cache a second decision, or introduce a temporary compatibility path.

- `oregon-primitives` owns canonical data, encoding, identifiers, and Merkle commitments.
- `oregon-pow` owns RandomX resource safety and hashing engines.
- `oregon-consensus` owns monetary validity, header and block validity, ASERT, target/work, and PoW validation order.
- `oregon-utxo` owns confirmed-output transitions, undo, maturity, and spend-verifier enforcement.
- `oregon-storage` owns RocksDB representation, codecs, batches, migrations, and durability.
- `oregon-chainstate` owns active-chain selection, recovery, reorganization, preferred-header/active-chain views, and publication after durable storage.
- `oregon-mempool` owns unconfirmed transaction policy, dependency limits, eviction, and reconciliation.
- `oregon-protocol` owns wire messages, protocol-version/feature negotiation, frame format, inventory representation, and remote message/resource limits.
- `oregon-network` owns transport abstraction, framed I/O deadlines, and TCP transport mechanics; it does not own peer policy or consensus validity.
- `oregon-peer` owns peer lifecycle, handshake state, self/duplicate-peer prevention, bounded peer queues, request matching, liveness, scoring, and cooldown policy.
- `oregon-sync` owns headers-first synchronization mechanics, locator construction over authoritative local views, bounded block-request scheduling, timeout reassignment, buffering, and stall state; it does not own chain preference.
- `oregon-node` owns orchestration only: it connects peer/network/sync events to authoritative `ChainState` and `Mempool` operations, provides bounded core ownership/read commands, and authorizes relay only after successful validation/admission.

An implementation detail cannot become a cross-crate API merely because it is convenient. Cross-crate interfaces describe protocol or domain meaning, never an internal codec, column-family name, milestone, or migration step.

## 2. Dependency direction

The core dependency graph is acyclic:

`primitives <- pow <- consensus <- utxo <- storage <- chainstate`

`mempool` may depend on `primitives`, `consensus`, and `utxo`. It must not depend on storage representation or chainstate internals.

The M6 networking/orchestration direction is also one-way:

`protocol -> network -> peer -> sync -> node`

The exact Cargo graph may include lower-level shared domain dependencies, but the core crates (`consensus`, `utxo`, `storage`, `chainstate`, `mempool`) must never depend upward on `protocol`, `network`, `peer`, `sync`, or `node`. `oregon-node` may depend downward on the authoritative core and M6 layers because it is the composition/orchestration boundary. Network, peer, and sync layers must not duplicate monetary, consensus, storage, chain-selection, or mempool-policy rules.

New dependencies require an architectural review proving that they preserve these directions and do not create a second rule owner.

## 3. Frozen behavior

Accepted consensus vectors, monetary constants, RandomX domains and key schedule, maturity, target/work rules, durability semantics, chain-selection policy, reorg limits, schema fail-closed behavior, mempool policy, protocol framing/resource bounds, handshake/peer bounds, synchronization ownership limits, timeout/stall semantics, and validation-before-relay ordering cannot change during refactoring.

A behavior change requires all of the following before implementation:

1. a versioned design document naming the existing and proposed rules;
2. explicit owner approval;
3. test vectors or characterization tests that distinguish the behaviors;
4. an isolated implementation branch; and
5. a new acceptance checkpoint after independent verification.

Renaming, moving, or decomposing code is not permission to reinterpret a frozen rule.

## 4. Durable state boundary

Accepted active-chain state is published in memory only after the corresponding RocksDB batch succeeds with WAL enabled and synchronous durability. A durable-write failure faults the current chainstate session. Maintenance writes remain separate from acceptance writes.

No module may publish partial state, replay half a reorganization, or convert a storage failure into apparent acceptance.

## 5. Validation boundaries

Header prevalidation, required target, RandomX key selection, hashing, and target comparison form one consensus path. Both light and full RandomX engines must use that path.

Confirmed and unconfirmed spends use the mandatory `SpendVerifier` boundary. A mempool overlay may reuse UTXO transition logic but cannot define a second spend-validity algorithm.

Remote peer claims, including advertised best height, are never chain-selection authority. `oregon-sync` consumes authoritative local chain/preferred-header views supplied through its `ChainSyncView` boundary. Blocks and transactions may be advertised to other peers only after `oregon-node` receives successful authoritative ChainState/Mempool acceptance and produces an opaque relay authorization.

## 6. Module and API quality

- Files and modules are named after stable responsibilities, not task or milestone numbers.
- Production errors describe current states; completed-stage placeholders are deleted.
- A public API exists only for a real external consumer.
- Test hooks are compile-time gated and never re-exported in production builds.
- Identical test infrastructure is shared within its crate; behavior-specific fixtures remain local.
- Large files are split when they contain multiple independent reasons to change.
- Compatibility shims require an explicit expiry condition and are forbidden when every caller can migrate atomically.
- Bounded queues, byte budgets, remote collection limits, and in-flight limits are enforced before attacker-controlled growth can occur.

## 7. Unsafe code

Unsafe Rust is permitted only inside the `oregon-pow` RandomX FFI and engine boundary. All other crates declare `#![forbid(unsafe_code)]`.

Every unsafe operation must be tied to a nearby `SAFETY:` explanation of the native ownership, lifetime, flag, pointer, or buffer invariant that makes it valid. Safe wrappers must prevent callers from violating those invariants.

## 8. Deletion and history

Dead production code, unreachable error variants, superseded contradictory notes, unused exports, and replaced task-named paths are deleted once all references are resolved. Accepted checkpoint records, golden vectors, recovery tests, and security mutation tests remain historical or executable evidence and are not cleanup targets.

Recovery is provided by immutable accepted checkpoint branches and accepted main history, not by keeping obsolete code beside the current implementation.

## 9. Verification

Every coherent change must pass:

```bash
cargo test --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

RandomX changes also require the architecture-vector and native full/light parity workflows. Persistence and chainstate changes retain durable-failure, recovery, reorg atomicity, depth-boundary, and pruning coverage. Mempool changes retain admission, graph-limit, eviction, stale-context, reconciliation, and atomicity coverage. M6 protocol/network/peer/sync/node changes retain architecture-boundary scans, real TCP handshake/relay/synchronization coverage, exact queue/in-flight/resource bounds, fork/lying-height resilience, timeout/stall behavior, and required security mutation evidence.

The accepted checkpoint branch is never moved. `main` is updated only by a separate, explicit integration decision after all required checks pass.
