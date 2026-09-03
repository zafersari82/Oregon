# Oregon v1 M4 Persistent Chainstate Checkpoint

Date: 2026-09-03
Branch: `oregon-v1-m4-persistent-chainstate`
Accepted M3 base: `8f3ee9043b9cf3beb7b8e4653c0f2ab183233b71`
M4 final reviewed code commit: `e2158b9dc8d6f151ccebe34872260bf24af6afe6`
Post-mutation verification commit: `21e2ea1df3b88cb4828fac2ca7e2ec46af3179ee`

## Accepted scope

M4 adds crash-safe persistent blockchain and active chainstate storage while preserving the accepted M1-M3 consensus, RandomX and UTXO semantics.

Frozen M4 behavior:

- `oregon-storage` owns RocksDB persistence, deterministic codecs, schema state, atomic write batches and durability modes.
- `oregon-chainstate` owns active-chain indexing, cumulative-work chain selection, direct extension, reorg orchestration, pruning and restart validation.
- Accepted active-state writes use one RocksDB `WriteBatch` with WAL enabled and `sync=true`.
- New in-memory UTXO/tip state is published only after the durable active-state write succeeds.
- A storage failure on a chain mutation faults the current session and prevents subsequent chain mutations until reopen/recovery.
- Reorg selection is based on strictly greater cumulative validated chainwork; equal or lower work never replaces the active tip.
- Candidate cumulative work is derived from validated target/work and parent chainwork, not caller-supplied work.
- RandomX key-block identity is derived from the candidate branch ancestry through the validated branch view.
- Reorg disconnect depth exactly `8_064` remains allowed when required data exists; depth `8_065` fails closed and enters/requires reindex handling before rollback data is applied.
- Pruning is a separate idempotent maintenance operation and may use non-sync writes because pruning can leave extra old data after interruption but cannot remove accepted active state.
- The active rollback window retains exactly 8,064 active block bodies/undos where those heights exist; block index/history identity is not pruned.
- Side-branch body retention is bound to both live height and a common-fork depth that remains within the permitted reorg window.
- Restart validates persisted config identity, node health, prune cursor, active mapping continuity, index identity/height/parent/work, required retained bodies/undos and UTXO decoding before publishing a healthy chainstate.
- Persisted UTXO reconstruction uses a narrow checked M3 bridge, rejects duplicate outpoints and does not bypass `SpendVerifier`, coinbase maturity, fee or block-connect validation.
- Storage formats are explicit/versioned/bounded and reject malformed lengths, non-canonical values, invalid flags, duplicates, unsorted undo records and trailing bytes.
- `ChainWork` consensus calculation is unchanged; M4 adds only canonical non-negative big-endian persistence with non-minimal encodings rejected.
- M3 founder/miner coinbase metadata and the exact 120-block maturity rule remain unchanged; no founder-specific exemption was introduced.
- Supported minor migration steps are marker-based, synchronous, idempotent and restart-resumable; unknown major schema versions fail closed without automatic rewrite.
- The new M4 Rust crates forbid unsafe Rust code.

## Task 10 — recovery / corruption acceptance matrix

Task 10 was completed on an isolated test-first branch and then fast-forwarded into M4 without carrying the temporary branch-only CI trigger into the accepted tree.

The additional recovery matrix covers, together with the existing M4 tests:

- multi-block close/reopen equivalence for active tip and persisted UTXOs
- deterministic sorted UTXO reconstruction after a real spend
- accepted active state reopening correctly when pruning has not yet run
- harmless extra old body data after skipped/interrupted pruning
- expected absence of body data behind a valid prune horizon
- fail-closed startup on active index height corruption
- fail-closed startup on active parent-link corruption
- fail-closed typed reads on block-index key/header and parent/header identity mismatch
- fail-closed corrupt persisted UTXO/body/undo paths
- fail-closed tampered cumulative chainwork
- deterministic UTXO/undo bytes independent of operation insertion order
- supported minor migration convergence across restart points before/after the migration marker
- unknown-major schema rejection without rewrite
- lower/equal-work candidate non-publication
- RandomX key provenance through candidate ancestry
- durable-write fault behavior and storage-faulted session behavior
- pruning retention/off-by-one and idempotent retry behavior

Task 10 exact formatted test commit:

- `9fe32c5253f15cb6b063bd374678b58554cc1a0f`
- Oregon Rust CI run `33798122037`
- workspace tests: SUCCESS
- rustfmt: SUCCESS
- clippy with warnings denied: SUCCESS

Task 10 final integration commit:

- `e2158b9dc8d6f151ccebe34872260bf24af6afe6`
- the temporary task-branch workflow trigger was removed before integration
- prior M4 head -> integration relation: fast-forward, 11 commits ahead / 0 behind

## Clean CI evidence

### Pre-mutation clean gate

Exact reviewed code commit: `e2158b9dc8d6f151ccebe34872260bf24af6afe6`

- Oregon Rust CI run `33800133553`
- workspace tests: SUCCESS
- rustfmt: SUCCESS
- clippy with `-D warnings`: SUCCESS

### Post-mutation clean gate

A new commit was created on the exact same source tree solely to obtain fresh verification after all required security mutations had been run:

- verification commit: `21e2ea1df3b88cb4828fac2ca7e2ec46af3179ee`
- compare `e2158b9... -> 21e2ea1...`: 1 commit ahead, 0 behind, **0 changed files**
- Oregon Rust CI run `33801324293`
- workspace tests: SUCCESS
- rustfmt: SUCCESS
- clippy with `-D warnings`: SUCCESS

Therefore the post-mutation clean gate tested the same M4 source tree as the final reviewed code commit.

## Task 11 — required security mutation evidence

All mutation code remained isolated on throwaway branches. None of the mutation source changes or branch-only CI trigger entries entered `oregon-v1-m4-persistent-chainstate`.

### Mutation A — reorg/pruning off-by-one boundary

Branch: `mutation-m4-reorg-off-by-one-2026-09-03`

Mutation code commit: `2d98026fafac28d604caa92477dc5a0b48e86675`

Mutation:

- changed `reorg_depth_allowed(depth)` from `depth <= REORG_WINDOW` to `depth <= REORG_WINDOW + 1`
- this intentionally made depth `8_065` eligible

CI trigger commit: `13eb143f056d2c7045645645744dc9eeff62536f`

Oregon Rust CI run: `33800216850`

Result: expected FAILURE.

Killed by:

- `reorg::tests::reorg_window_accepts_8064_and_rejects_8065`
- `state::deep_reorg_tests::depth_8065_marks_reindex_before_loading_any_rollback_data`

Observed symptom: the unit boundary no longer rejected `8_065`, and the deep-reorg state path no longer returned the required `ReindexRequired` result before rollback loading.

### Mutation B — durability weakening

Branch: `mutation-m4-durability-nosync-2026-09-03`

Mutation code commit: `146c1dcae3ec5b9f9a0ab4285542df9948923f6a`

Mutation:

- changed `OregonDb::commit_durable()` from `DurabilityMode::Sync` to `DurabilityMode::NoSync`
- the commit also contained one non-semantic blank-line deletion caused by connector whole-file replacement; the only behavioral change was the durability mode

CI trigger commit: `9b5d531d7c8cd41aeadbe54c113b29bbb9279025`

Oregon Rust CI run: `33800586601`

Result: expected FAILURE.

Killed by:

- `task7_storage_fault_tests::durable_failure_faults_session_without_publishing_or_persisting_candidate`

Observed symptom: the test's injected durable failure is attached to the synchronous durable mode; weakening `commit_durable` to NoSync bypassed that failure boundary and the candidate operation no longer returned the required storage error. The test therefore detected the durability weakening before the mutated behavior could be accepted.

### Mutation C — early/partial reorg publication

Branch: `mutation-m4-early-reorg-publication-2026-09-03`

Mutation code commit: `022d85abbdd24a7eacf50535c613cc801923245e`

Mutation:

- intentionally truncated `load_reorg_plan()` after the first candidate node
- this made a multi-block candidate capable of being applied/published before the complete candidate path had been loaded and validated

CI trigger commit: `16542aa8c4a94b8192ed199d8cda41ea4c740077`

Oregon Rust CI run: `33800401977`

Result: expected FAILURE.

Killed by:

- `valid_reorg_is_atomic_and_reopens_on_strictly_heavier_candidate`
- `invalid_candidate_body_marks_failing_block_and_descendants_without_active_publication`

Observed symptoms:

- the valid heavy-candidate test observed a partial candidate tip instead of the requested final candidate tip
- the invalid-candidate test no longer received the expected UTXO/consensus failure because the intentionally invalid later candidate body was never reached before the truncated path was applied

These failures directly guard the rule that the complete candidate branch must be preflighted/validated before active state is durably published.

## M3 -> M4 manual security review

Reviewed range:

- accepted M3 base: `8f3ee9043b9cf3beb7b8e4653c0f2ab183233b71`
- reviewed M4 code: `e2158b9dc8d6f151ccebe34872260bf24af6afe6`
- relation: M4 is 121 commits ahead and 0 commits behind the accepted M3 base

Manual review covered the M4 design-required surfaces:

- RocksDB WAL and synchronous acceptance write options
- one-batch atomic coverage of block/index/UTXO/undo/active-map/tip state
- disk/memory publication ordering
- ambiguous storage-error fail-stop behavior and `StorageFaulted` gating
- direct extension and reorg staged-state publication
- restart validation ordering and fail-closed corruption handling
- deterministic/bounded block-index, UTXO and undo encodings
- canonical chainwork persistence without changing consensus work calculation
- UTXO restart reconstruction boundary and duplicate rejection
- pruning horizon and exact 8,064/8,065 off-by-one behavior
- side-branch body retention predicate
- deep-reorg fail-closed behavior
- supported minor migration marker/step/finalization durability
- unknown-major schema non-rewrite behavior
- cumulative-work provenance
- M2 RandomX candidate-ancestry key-block provenance
- M3 `SpendVerifier` enforcement
- founder/miner coinbase maturity regression risk
- checked consensus-value arithmetic
- consensus/storage-visible ordering and `HashMap` iteration dependence
- dependency/CI surface and production exposure of test hooks

Review disposition:

- RocksDB is exactly pinned to `0.24.0`; default features are disabled and the selected features are explicit.
- acceptance writes keep WAL enabled and use synchronous durability; pruning maintenance is separate and non-sync by design.
- active in-memory state is not published before the durable acceptance/reorg batch succeeds.
- storage failure faults the session and blocks later mutations until reopen.
- reorg preflight/staging does not publish partial candidate state in the accepted code.
- branch ancestry validates parent/height/cumulative-work consistency before use.
- RandomX key identity remains bound to candidate ancestry rather than caller input.
- M3 UTXO spend, fee, duplicate-input, coinbase maturity and block-overlay semantics were not weakened by M4's persistence bridge.
- no founder-specific maturity bypass was found.
- persisted undo/UTXO ordering is canonical and does not rely on `HashMap` iteration order.
- startup does not construct a healthy chainstate until persisted active-chain and UTXO invariants pass.
- no known Critical or Important M4 security finding remains open at this checkpoint.

## Acceptance

M4 persistent chainstate is accepted at reviewed code tree `e2158b9dc8d6f151ccebe34872260bf24af6afe6`, with the same tree freshly re-verified post-mutation at `21e2ea1df3b88cb4828fac2ca7e2ec46af3179ee` by Oregon Rust CI run `33801324293`.

Acceptance includes deterministic RocksDB persistence, durable active-state publication, restart reconstruction/validation, cumulative-work branch selection, bounded reorgs, pruning, supported migration recovery and the narrow persisted-UTXO bridge defined by the approved M4 design.

This acceptance does **not** claim completion of mempool policy, P2P networking, initial block download/network synchronization, wallet/address encoding, production Schnorr/KeyCommitV1 spend cryptography beyond the existing verifier boundary, RPC/miner integration, testnet, genesis launch or mainnet readiness.

`main` was not merged or modified as part of M4 implementation or acceptance.
