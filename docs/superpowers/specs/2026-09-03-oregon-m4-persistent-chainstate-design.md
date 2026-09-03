# Oregon v1 M4 Persistent Chainstate Design

Date: 2026-09-03
Status: approved design, implementation not started
Development branch: `oregon-v1-m4-persistent-chainstate`
Accepted base: `8f3ee9043b9cf3beb7b8e4653c0f2ab183233b71` (M3 checkpoint commit)

## 1. Goal

M4 adds crash-safe persistent blockchain and chainstate storage to Oregon while preserving the accepted M1-M3 consensus, RandomX and UTXO semantics.

M4 is not a full node milestone. It does not add P2P networking, mempool policy, wallet/address support, RPC/miner integration, genesis launch, testnet or mainnet readiness.

## 2. Approved architecture

M4 introduces two new crates:

- `oregon-storage`: RocksDB ownership, column families, deterministic storage encodings, atomic write batches, WAL/sync durability, schema versions and migrations.
- `oregon-chainstate`: active-chain index, connect/disconnect orchestration, cumulative chainwork, reorg selection, pruning policy, restart validation and integration with the existing M3 `UtxoState` transition engine.

The storage layer does not make consensus decisions. The chainstate layer does not expose RocksDB details to consensus code. M3 remains authoritative for UTXO transition validity.

A generic multi-backend storage trait is intentionally not introduced in M4. RocksDB is the single persistence backend for this milestone. The exact Rust RocksDB crate release must be pinned through `Cargo.lock` during implementation and CI must use `--locked`.

## 3. RocksDB column families

The database contains these named column families:

### `blocks`

Key: block ID.

Value: the exact canonical bytes returned by `Block::encode()`.

This avoids defining a second representation for block bodies. Block bodies are prunable.

### `block_index`

Key: block ID.

Value: a versioned `BlockIndexRecord` containing at least:

- parent block ID
- height
- cumulative chainwork
- validation/index status required by M4
- body-retained/pruned state when needed for deterministic recovery

Index records are not pruned.

### `utxo`

Key: canonical 36-byte outpoint encoding: 32-byte txid followed by 4-byte little-endian output index.

Value: versioned deterministic encoding of `UtxoEntry`, including:

- output amount in base units
- locking program bytes
- creation height
- coinbase flag

No `HashMap` iteration order may affect keys or values.

### `undo`

Key: block ID.

Value: deterministic versioned encoding of `BlockUndo`.

M3 already canonicalizes spent and created collections by outpoint ordering; the disk codec must preserve that order and reject malformed, duplicate or trailing data.

Undo data is prunable outside the rollback window.

### `chain_meta`

Contains versioned metadata including:

- schema major/minor version
- active tip block ID and height
- active `height -> block_id` mapping
- prune horizon/cursor
- migration marker/state
- durable node health state such as `Healthy`, `ReindexRequired` or equivalent

## 4. Canonical storage encodings

All M4-owned storage records use explicit deterministic encodings with version bytes and bounded decoders. `serde`/`bincode`-style implementation-defined persistence is not the authoritative disk format.

Block bodies are stored using the existing protocol canonical `Block::encode()` bytes and decoded with the existing bounded block decoder.

Outpoints use a fixed 36-byte key.

`ChainWork` must not be truncated to 256 bits. It is persisted as a canonical non-negative integer representation. The encoding must reject non-minimal alternate representations so the same work value has one disk byte representation.

All M4 decoders reject truncation, unknown versions where no migration is defined, malformed lengths and trailing bytes.

## 5. Chain selection

Candidate chains are selected by cumulative validated chainwork.

A caller may not inject an arbitrary cumulative-work value. Per-block work originates from the accepted M2 pre-PoW validation path and is accumulated by chainstate with checked/validated semantics.

Only a candidate with strictly greater cumulative chainwork than the current active tip may trigger a reorg. Equal cumulative chainwork keeps the existing active tip.

M2 RandomX key provenance remains binding. Side-chain PoW validation must obtain scheduled key-block identity from an already validated branch-aware chain view. Candidate callers do not supply arbitrary key-block IDs.

## 6. Durable block acceptance

The active-state acceptance write is one atomic RocksDB `WriteBatch` containing all changes necessary for the accepted state, including as applicable:

- block body
- block index
- UTXO deletions and insertions
- block undo
- active height mapping
- active tip metadata
- related M4 chainstate metadata

WAL remains enabled and the acceptance write uses `sync=true`.

A block or reorg is not reported as accepted until that durable write succeeds. If the write fails, the API returns an error and neither the durable chainstate nor the authoritative in-memory chainstate may advance.

The implementation must stage consensus/state changes first and publish the new in-memory active state only after the durable write succeeds, or use an equivalent ordering that proves memory and disk cannot diverge on a failed durable commit.

## 7. Restart invariants

On database open, M4 validates at least:

- supported schema state
- active tip metadata
- active tip index existence
- active height mapping continuity required for the retained chain view
- parent/height consistency of retained active-chain index records
- required body and undo presence throughout the live rollback window
- deterministic decoding of retained UTXO, undo and index records
- migration marker validity

Missing body/undo data behind the valid prune horizon is expected. Missing or corrupt body/undo data inside the rollback window is not repaired heuristically.

Critical inconsistency causes fail-closed startup with `ReindexRequired`, `CorruptData`, `UnsupportedSchema` or another explicit non-healthy result. M4 does not silently invent replacement state.

## 8. Reorg state machine

For a candidate chain with strictly greater cumulative work:

1. Locate the common fork point using validated index ancestry.
2. Compute active-chain disconnect depth before mutating live state.
3. If disconnect depth is greater than `8_064`, perform no UTXO or active-mapping mutation and enter/report durable `ReindexRequired` or `ResyncRequired` state.
4. For an allowed reorg, preflight all required old-branch undo data and new-branch block bodies before applying state changes.
5. Clone/stage the current M3 UTXO state.
6. Disconnect old active blocks from tip to fork using validated `BlockUndo`.
7. Connect candidate blocks from fork forward through the existing M3 `UtxoState::connect_block()` path with the required production `SpendVerifier` boundary.
8. Do not persist or publish partial candidate state if any candidate block fails.
9. If the complete candidate branch succeeds, persist all resulting active-state changes in one durable `WriteBatch` with WAL and `sync=true`.
10. Only after the durable commit succeeds may the new in-memory active tip/state be published.

There is no production `AcceptAll` spend verifier introduced by M4.

## 9. Pruning policy

M4 is pruning-aware from its first accepted version.

The configured/frozen rollback retention window for this milestone is `8_064` active blocks, approximately 28 days at the 300-second target interval.

If active tip height is `H`, active block body and undo data for heights:

`H - 8063 ... H`

must remain retained when those heights exist.

The deepest permitted disconnect has depth exactly `8_064`, whose fork point is height `H - 8064`.

Therefore:

- disconnect depth `8_064`: allowed, assuming all retained data required for the operation exists
- disconnect depth `8_065`: rejected fail-closed

The implementation and tests must explicitly guard this off-by-one boundary.

`block_index` and chain identity metadata are not pruned. Large block bodies and undo data older than the safe horizon may be removed.

Side-branch bodies that can still participate in an otherwise permitted reorg must not be pruned prematurely.

## 10. Pruning transaction model

Pruning is separate from block acceptance.

First, the accepted block/reorg is committed durably. Then pruning runs as an idempotent maintenance operation in a separate atomic batch.

Pruning is allowed to use `sync=false` because a pruning crash may leave extra old data but must never cause accepted consensus state to disappear. On restart, the same maintenance operation can safely run again.

Pruning must delete only data proven older than the safe retention horizon. It must never delete index/history metadata required to identify the active chain or support RandomX ancestor/key scheduling.

## 11. Schema versioning and migration

The M4 database schema uses a major/minor version.

Supported minor upgrades may migrate automatically. Minor migrations must be:

- explicit
- idempotent
- restart-resumable
- covered by crash/interruption tests

Before mutating a database during migration, a durable migration marker/state is written. After a crash, reopening the same supported migration resumes or safely repeats deterministic steps.

An unknown/incompatible major schema version is never modified automatically. Opening it fails closed with `UnsupportedSchema` and/or `ReindexRequired`.

M4 does not promise indefinite migration support for all future historical schema versions.

## 12. Error model

`oregon-storage` owns storage-specific errors such as I/O/RocksDB failures, corrupt encoded records, unsupported schema and durability failure.

`oregon-chainstate` exposes higher-level state errors without collapsing distinct failure causes. The design must preserve actionable distinctions including equivalents of:

- `CorruptData`
- `UnsupportedSchema`
- `MissingUndo`
- `MissingBlockBody`
- `DeepReorg`
- `DurabilityFailure`
- `ReindexRequired`

Consensus-invalid blocks remain distinct from database corruption or local storage failure.

## 13. UTXO persistence strategy

M4 does not rewrite the complete UTXO set for every accepted block.

The existing M3 transition and `BlockUndo` result determine the delta. Persisted UTXO changes are proportional to block state changes:

- consumed pre-existing outpoints are deleted
- newly surviving outpoints are inserted
- disconnect applies the inverse using validated undo data

Same-block intermediate outputs that do not survive the final M3 UTXO state are not materialized as persistent UTXOs merely because they existed transiently during block execution.

## 14. Crash-safety model

The principal invariant is:

> After an API reports a block or reorg accepted, a clean restart observes that accepted active state. If the durable acceptance write fails, the previous active state remains authoritative in both memory and storage.

Crash points tested in M4 include at minimum:

- before durable acceptance write
- failed durable acceptance write
- after successful durable acceptance write but before subsequent pruning
- during pruning
- during supported minor migration

A startup path may not infer that a partially durable state was accepted unless all active-state invariants prove it.

## 15. TDD acceptance tests

Implementation follows strict RED -> observed failure -> minimal GREEN -> fresh verification.

M4 acceptance tests include at least:

1. Close/reopen preserves active tip, UTXO state and chain index exactly.
2. Failed durable block write leaves both memory and disk at the old state.
3. Reorg depth exactly `8_064` is permitted when required data exists.
4. Reorg depth `8_065` is rejected with no active-state mutation.
5. Missing or tampered undo prevents reorg without partial mutation.
6. Invalid final candidate block rolls back/stages away all earlier candidate branch changes.
7. Equal cumulative chainwork retains the current active tip.
8. Lower-work candidate never becomes active.
9. Pruning does not delete any body/undo still required by the rollback window.
10. Interrupted pruning is idempotently recoverable.
11. Storage codecs reject truncation, trailing bytes, malformed lengths and unsupported record versions.
12. Supported minor migration interrupted mid-step resumes deterministically.
13. Unknown major schema fails closed without rewriting the database.
14. Corrupt/missing retained-window records cause explicit non-healthy startup.
15. On-disk UTXO/undo bytes are deterministic and independent of `HashMap` iteration order.
16. Reorg durable commit is all-or-nothing across UTXO, undo, active mapping and tip metadata.
17. Restart after successful durable acceptance but before pruning sees the accepted tip and merely retains extra old data.
18. Side-chain validation preserves M2 key-block provenance and does not accept caller-injected RandomX key identity.

## 16. Required security mutations

M4 is not accepted until targeted throwaway mutations are killed by intended tests. At minimum:

### Mutation A: pruning/reorg off-by-one

Change the `8_064` boundary so a depth-`8_065` reorg becomes allowed or a valid depth-`8_064` reorg becomes impossible. Boundary tests must fail.

### Mutation B: durability weakening

Remove or bypass the required synchronous durable acceptance semantics, or otherwise make acceptance publish before the durable write has succeeded. Crash/durability tests must fail.

### Mutation C: early reorg publication

Publish/persist active UTXO or tip state before the complete candidate branch has validated. Atomic reorg rollback tests must fail.

Mutations live only on throwaway branches and never enter the accepted M4 branch.

## 17. Security review scope

Before M4 checkpoint acceptance, manual review covers at least:

- RocksDB WAL/sync write options on acceptance paths
- memory/disk publish ordering
- WriteBatch atomic coverage
- block/index/UTXO/undo encoding determinism
- bounds and corruption handling
- pruning horizon off-by-one behavior
- side-branch retention
- deep-reorg fail-closed path
- migration restart safety
- cumulative-work provenance
- M2 RandomX key-block provenance through side-chain validation
- M3 `SpendVerifier` enforcement
- no founder/coinbase maturity regression
- no unchecked consensus-value arithmetic introduced by M4
- no `HashMap` iteration order influencing stored canonical records

## 18. Definition of M4 accepted

M4 is accepted only when all of the following are true at one reviewed code commit:

- `oregon-storage` and `oregon-chainstate` implement this approved scope
- workspace tests pass
- rustfmt passes
- clippy passes with warnings denied
- restart/crash/reorg/pruning/migration tests pass
- required security mutations are each killed by intended tests
- fresh post-mutation clean CI passes
- M3 -> M4 manual security review has no known Critical or Important finding left open
- an M4 checkpoint document records exact commits and CI/mutation evidence
- an accepted M4 recovery branch is created

`main` is not merged or modified as part of M4 implementation unless the user later explicitly requests that integration.

## 19. Explicit exclusions

M4 does not claim completion of:

- mempool
- peer-to-peer networking
- initial block download or network sync protocol
- wallet or address encoding
- production Schnorr/KeyCommitV1 spend cryptography beyond existing boundaries
- mining RPC
- public RPC server
- node daemon packaging
- production genesis
- testnet launch
- mainnet launch
- indefinite historical block archival
- snapshot-based deep rollback beyond the 8,064-block retained window

These remain later milestones.
