# Oregon v1 Architecture Unification Design

**Status:** Owner-approved on 2026-09-04  
**Baseline:** `a2aab4b73489aa0cf21bd7d14f8b930328c3465c`  
**Recovery branch:** `oregon-v1-checkpoint-m5-mempool-accepted-2026-09-04`  
**Implementation branch:** `oregon-v1-architecture-unification-2026-09-04`

## Purpose

Unify the accepted M0-M5 implementation so the repository reads and behaves like one deliberately designed system. The cleanup removes milestone seams, duplicate or misplaced authority, stale claims, internal storage leaks, and obsolete scaffolding without changing accepted consensus, persistence, chain-selection, UTXO, or mempool behavior.

This is an architectural cleanup of the accepted M5 checkpoint. It is not the next protocol milestone and does not add P2P networking, wallet support, mining RPC, production spend cryptography, a production genesis block, or mainnet activation.

## Frozen behavior

The following behavior is invariant throughout this work:

- One OREG equals 100,000,000 base units.
- The maximum scheduled supply envelope is 1,000,000 OREG.
- The founder allocation is 50,000 OREG and is valid only at height 1.
- The initial mining subsidy is 2.375 OREG with a 200,000-block halving interval.
- There is no continuing founder tax, administrative mint, treasury tax, or fee burn.
- RandomX input encoding, key schedule, derivation domains, upstream commit, Argon salt, target byte order, and known vectors remain frozen.
- Header prevalidation precedes RandomX hashing; the RandomX key comes only from the validated chain.
- `SpendVerifier` remains mandatory and coinbase maturity remains exactly 120 blocks.
- Accepted active-chain state is published in memory only after a successful RocksDB write with WAL and `sync=true`.
- A durable storage failure faults the chainstate session.
- Only strictly heavier cumulative work replaces the active chain.
- Reorg depth 8,064 is accepted; depth 8,065 fails closed with `reindex required`.
- Pruning remains separate, idempotent, and outside the durable acceptance transaction.
- Supported schema-minor migration is automatic; unknown major versions fail closed.
- Mempool policy remains one spender per outpoint, with no RBF and no orphan pool.
- Mempool admission and chain reconciliation remain staged and atomic.
- Mempool limits remain 50,000 entries, 64 MiB total bytes, 25 ancestors, and 25 descendants.
- Mempool ordering and eviction remain deterministic.

## Ownership model

| Crate | Sole authority | Must not own |
| --- | --- | --- |
| `oregon-primitives` | Canonical types, encoding, identifiers, Merkle commitments | Consensus policy, persistence layout |
| `oregon-pow` | RandomX lifetime safety, hashing engine abstraction, frozen RandomX inputs | Chain selection, header-context validation |
| `oregon-consensus` | Monetary rules, block/header validity, ASERT, target/work, PoW validation order | RocksDB representation, active-chain mutation |
| `oregon-utxo` | UTXO transitions, undo, maturity, spend-verifier enforcement | Persistence terminology, mempool capacity policy |
| `oregon-storage` | RocksDB schema/codecs, atomic batches, durability modes, typed persistence API | Consensus decisions, chain publication |
| `oregon-chainstate` | Active-chain selection, recovery, reorg orchestration, durable publication | Raw column-family names or storage codecs |
| `oregon-mempool` | Unconfirmed dependency graph, policy admission, capacity, eviction, reconciliation | Consensus emission or persistent-chain authority |

The dependency direction remains acyclic:

`primitives <- pow <- consensus <- utxo <- storage <- chainstate`

`mempool` depends only on `primitives`, `consensus`, and `utxo`. It does not depend on storage or chainstate internals.

## Required changes

### 1. Repository contract and historical separation

- Replace the obsolete M0-oriented README with an M5-accurate project map and status.
- Add `docs/architecture/OREGON_ENGINEERING_CONSTITUTION.md` as the current architectural contract.
- Delete `docs/checkpoints/OREGON_V1_M2_REVIEW_NOTE.md`; it records a missing decision that was superseded by the accepted M2 checkpoint.
- Preserve accepted checkpoint documents and historical progress records unchanged except for links that must be repaired.
- Rename current test files and temporary-directory labels from task numbers to behavior names. The test coverage itself is retained.

### 2. One PoW validation path

- Add a public `PowEngine` trait in `oregon-pow`:

  ```rust
  pub trait PowEngine {
      fn key(&self) -> [u8; 32];
      fn hash(&mut self, input: &[u8]) -> [u8; 32];
  }
  ```

- Implement the trait for both `LightEngine` and `FullEngine`.
- Keep the existing inherent `key` and `hash` methods only if they delegate to the same private implementation and do not create a second rule path.
- Make `oregon_consensus::validate_header_pow` generic over `PowEngine` so validation order and error semantics are identical for light and full engines.
- Add a lightweight fake-engine consensus test that proves the validator consumes the trait, checks the expected key before hashing, and preserves failure ordering.
- Preserve the existing native full/light RandomX parity workflow.

### 3. Semantic UTXO construction and storage encapsulation

- Rename `UtxoState::from_persisted_entries` to `UtxoState::try_from_entries`.
- Rename `UtxoError::DuplicatePersistedOutpoint` to `DuplicateOutpoint`.
- Use the semantic constructor for both database recovery and narrow mempool validation overlays.
- Derive `PartialOrd` and `Ord` for `OutPoint`; its order is canonical `Hash256` followed by the numeric output index.
- Change chainstate UTXO deltas from storage-encoded byte keys to `BTreeMap<OutPoint, Option<UtxoEntry>>`.
- Stop exporting storage codecs, column-family constants, metadata keys, record codecs, and `SCHEMA_VERSION` from `oregon-storage` unless a non-storage production crate genuinely consumes them.
- Keep `SchemaVersion` public because it appears in public storage errors. Keep typed storage operations public.
- Storage-internal tests import crate-private items directly; other crates use only typed storage operations.

### 4. Responsibility-based modules

Split files without changing public behavior:

- `oregon-chainstate/src/state.rs` retains `ChainState`, `Tip`, `SessionHealth`, accessors, and the high-level acceptance entrypoint.
- `oregon-chainstate/src/admission.rs` owns candidate prevalidation, RandomX invocation, and branch-choice planning.
- `oregon-chainstate/src/transition.rs` owns direct extension and reorg publication.
- `oregon-chainstate/src/recovery.rs` owns bootstrap, reopen, persistent invariant checks, and configuration validation.
- `oregon-chainstate/src/utxo_delta.rs` owns semantic deterministic UTXO delta construction and batch application.
- `oregon-storage/src/db.rs` owns the database handle and typed reads.
- `oregon-storage/src/commit.rs` owns batch encoding, write options, WAL/sync selection, and commit execution.
- `oregon-storage/src/migration.rs` owns schema initialization and supported migration.
- `oregon-mempool/src/pool.rs` owns state, construction, public queries, and public admission/reconciliation entrypoints.
- `oregon-mempool/src/admission.rs` owns candidate validation, replay, and atomic admission commit planning.
- `oregon-mempool/src/capacity.rs` owns deterministic capacity and eviction planning.

Private types may be `pub(crate)` only where adjacent modules require them. No new public API is introduced solely to make the split compile.

### 5. Dead code and test structure

- Delete `ChainStateError::DeferredTransition`; M4 is complete and the variant has no callers.
- Remove the discarded `usize` from `Mempool::prepare_admission`; capacity planning remains the sole calculation of post-admission bytes.
- Add crate-local `test_support` modules where unit tests currently duplicate temporary-directory, transaction, configuration, or verifier builders.
- Keep integration-test helpers under each crate's `tests/common` directory.
- Add `#![forbid(unsafe_code)]` to `oregon-primitives`, `oregon-consensus`, and `oregon-utxo`.
- Keep unsafe code confined to the RandomX FFI and engine boundary in `oregon-pow`.

## Deletion policy

Direct deletion is permitted only when all references have been resolved and one of these conditions holds:

1. the item is unreachable production code;
2. the document is superseded and contradicts an accepted checkpoint;
3. the old path is replaced by a behavior-named path with equivalent test coverage; or
4. the public export exposes an implementation detail and no production consumer depends on it.

Consensus vectors, accepted checkpoint records, recovery tests, security mutation tests, and durable-failure tests are not cleanup targets.

## Verification gates

Every coherent change is committed separately and must pass:

```bash
cargo test --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

RandomX-specific changes must also pass both architecture-vector and native full/light parity workflows. The final branch must pass the GitHub Actions checks because the local workspace does not contain a Rust toolchain.

The final review must confirm:

- no current production or test filename contains a milestone task number;
- no safe crate contains `unsafe` outside `oregon-pow`;
- no non-storage production crate imports storage codecs or column-family constants;
- no persistence-named UTXO constructor remains;
- no second PoW consensus-validation path exists;
- no frozen behavior or golden vector changed;
- README status matches the accepted M5 checkpoint; and
- `main` remains untouched until a separately approved integration decision.

