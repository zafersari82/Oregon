# Oregon v1 Architecture Unification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove M0-M5 architectural seams and make every rule, API, and module have one clear owner while preserving the accepted M5 behavior exactly.

**Architecture:** Work from the accepted M5 checkpoint on an isolated branch. Contract public behavior before moving code, replace cross-layer leaks with semantic interfaces, and split oversized modules only at responsibility boundaries. Each gate ends in a focused commit and GitHub CI verification.

**Tech Stack:** Rust 1.85.0, Cargo workspace, RandomX C FFI, RocksDB, GitHub Actions

**Spec:** `docs/superpowers/specs/2026-09-04-oregon-architecture-unification-design.md`

## Global Constraints

- Preserve every frozen behavior listed in the spec.
- Do not add P2P, wallet, mining RPC, production spend cryptography, production genesis, or mainnet activation.
- Keep `main` unchanged; work only on `oregon-v1-architecture-unification-2026-09-04`.
- Keep the accepted M5 recovery branch at `a2aab4b73489aa0cf21bd7d14f8b930328c3465c`.
- No duplicate consensus, storage, chainstate, UTXO, or mempool rule implementations.
- No unsafe Rust outside `oregon-pow`.
- Run workspace tests, formatting, and Clippy in GitHub Actions after every pushed gate.

---

### Task 1: Repository contract and obsolete-artifact cleanup

**Files:**
- Create: `docs/architecture/OREGON_ENGINEERING_CONSTITUTION.md`
- Modify: `README.md`
- Delete: `docs/checkpoints/OREGON_V1_M2_REVIEW_NOTE.md`
- Rename: `crates/oregon-chainstate/tests/task6_admission.rs` to `crates/oregon-chainstate/tests/block_admission.rs`
- Rename: `crates/oregon-chainstate/tests/task7_direct_extension.rs` to `crates/oregon-chainstate/tests/direct_extension.rs`
- Rename: `crates/oregon-chainstate/tests/task8_reorg.rs` to `crates/oregon-chainstate/tests/reorganization.rs`
- Rename: `crates/oregon-chainstate/tests/task9_prune.rs` to `crates/oregon-chainstate/tests/pruning.rs`
- Rename: `crates/oregon-chainstate/src/task7_storage_fault_tests.rs` to `crates/oregon-chainstate/src/storage_fault_tests.rs`
- Modify: `crates/oregon-chainstate/src/lib.rs`
- Modify: task-numbered temporary-directory labels in chainstate tests

**Interfaces:**
- Consumes: accepted checkpoint documents M0-M5.
- Produces: current repository contract and behavior-named test paths; no Rust public API change.

- [ ] **Step 1: Prove the obsolete review note has no live references**

Run:

```bash
rg -n "OREGON_V1_M2_REVIEW_NOTE|task[6789]" . --glob '!docs/superpowers/plans/*' --glob '!docs/checkpoints/OREGON_V0_PROTOCOL_PROGRESS.md'
```

Expected: the M2 note has no inbound reference; task numbers occur only in the paths, module declaration, historical checkpoint prose, and temporary labels already inventoried by the spec.

- [ ] **Step 2: Write the engineering constitution and replace README status**

The constitution must encode the ownership table, dependency direction, frozen-rule change procedure, unsafe boundary, deletion rule, and CI gates from the spec. README must identify M5 as the accepted development baseline and explicitly list both implemented and not-yet-implemented capabilities.

- [ ] **Step 3: Delete the superseded note and rename current tests**

Use behavior names from the Files block. Update the `mod storage_fault_tests;` declaration and replace temporary labels with `admission`, `direct-extension`, `reorganization`, `deep-reorg`, or `pruning`.

- [ ] **Step 4: Verify content and naming**

Run:

```bash
rg -n "current `oregon-v0-protocol`|does not yet implement the final emission engine|task[6789]" README.md crates/oregon-chainstate
test ! -e docs/checkpoints/OREGON_V1_M2_REVIEW_NOTE.md
```

Expected: no match from `rg`; `test` exits zero.

- [ ] **Step 5: Commit**

```bash
git add README.md docs crates/oregon-chainstate
git commit -m "docs: define unified Oregon architecture"
```

### Task 2: One PoW engine interface and safe-crate boundary

**Files:**
- Modify: `crates/oregon-pow/src/engine.rs`
- Modify: `crates/oregon-pow/src/lib.rs`
- Modify: `crates/oregon-consensus/src/pow.rs`
- Modify: `crates/oregon-consensus/src/lib.rs`
- Modify: `crates/oregon-primitives/src/lib.rs`
- Modify: `crates/oregon-utxo/src/lib.rs`

**Interfaces:**
- Consumes: `LightEngine::key`, `LightEngine::hash`, `FullEngine::key`, and `FullEngine::hash` behavior.
- Produces: `oregon_pow::PowEngine` and `validate_header_pow<E: PowEngine, S: PowKeyBlockSource + ?Sized>(..., engine: &mut E)`.

- [ ] **Step 1: Add a consensus compile-and-order test for a fake engine**

Add under `pow_bridge_tests`:

```rust
struct FakeEngine {
    key: [u8; 32],
    hash: [u8; 32],
    calls: usize,
}

impl PowEngine for FakeEngine {
    fn key(&self) -> [u8; 32] { self.key }
    fn hash(&mut self, _input: &[u8]) -> [u8; 32] {
        self.calls += 1;
        self.hash
    }
}
```

Test a matching key/max target succeeds with exactly one hash call and a mismatched key returns `PowEngineKeyMismatch` with zero hash calls.

- [ ] **Step 2: Run the focused test and observe the missing interface**

Run:

```bash
cargo test -p oregon-consensus pow_bridge_tests::generic_pow_engine_preserves_validation_order
```

Expected before implementation: compilation fails because `oregon_pow::PowEngine` does not exist and `validate_header_pow` still requires `LightEngine`.

- [ ] **Step 3: Add and implement the trait**

Add the exact trait from the spec to `engine.rs`, implement it for both engine types, export it from `lib.rs`, and change consensus validation to:

```rust
pub fn validate_header_pow<E, S>(
    header: &BlockHeader,
    facts: &PrePowHeaderFacts,
    key_blocks: &S,
    engine: &mut E,
) -> Result<[u8; 32], ConsensusError>
where
    E: PowEngine + ?Sized,
    S: PowKeyBlockSource + ?Sized,
```

Add `#![forbid(unsafe_code)]` to primitives, consensus, and UTXO crate roots.

- [ ] **Step 4: Run PoW and workspace gates**

Run:

```bash
cargo test -p oregon-pow --all-targets
cargo test -p oregon-consensus --all-targets
cargo test --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all commands pass. GitHub RandomX architecture-vector and full/light parity workflows remain green.

- [ ] **Step 5: Commit**

```bash
git add crates/oregon-pow crates/oregon-consensus crates/oregon-primitives crates/oregon-utxo
git commit -m "refactor: unify PoW engine validation"
```

### Task 3: Semantic UTXO construction and storage API contraction

**Files:**
- Modify: `crates/oregon-primitives/src/transaction.rs`
- Modify: `crates/oregon-utxo/src/state.rs`
- Modify: `crates/oregon-utxo/src/error.rs`
- Modify: `crates/oregon-utxo/src/lib.rs`
- Modify: `crates/oregon-chainstate/src/state.rs`
- Modify: `crates/oregon-chainstate/src/recovery_acceptance_tests.rs`
- Modify: `crates/oregon-mempool/src/pool.rs`
- Modify: `crates/oregon-mempool/src/reconcile.rs`
- Modify: `crates/oregon-mempool/tests/common/mod.rs`
- Modify: `crates/oregon-storage/src/lib.rs`
- Modify: storage-internal test imports

**Interfaces:**
- Consumes: iteration over `(OutPoint, UtxoEntry)` and typed `StorageBatch` operations.
- Produces: `UtxoState::try_from_entries<I>(entries: I) -> Result<UtxoState, UtxoError>` and `UtxoError::DuplicateOutpoint(OutPoint)`.

- [ ] **Step 1: Change the UTXO public-contract tests first**

Change the duplicate construction test to call:

```rust
let result = UtxoState::try_from_entries([(point, entry(100)), (point, entry(100))]);
assert_eq!(result, Err(UtxoError::DuplicateOutpoint(point)));
```

Add an `OutPoint` ordering test asserting txid order takes precedence and output index breaks equal-txid ties.

- [ ] **Step 2: Run focused tests and observe compile failures**

Run:

```bash
cargo test -p oregon-utxo duplicate_outpoint
cargo test -p oregon-primitives outpoint_order
```

Expected before implementation: compilation fails because the semantic names and ordering traits are absent.

- [ ] **Step 3: Implement semantic construction and ordering**

Derive `PartialOrd, Ord` on `OutPoint`. Rename the constructor and error variant at all call sites. Define chainstate delta as:

```rust
type UtxoDelta = BTreeMap<OutPoint, Option<UtxoEntry>>;
```

Insert semantic outpoints directly and apply entries in `BTreeMap` order. Remove `encode_outpoint_key` imports from chainstate and its tests.

- [ ] **Step 4: Contract the storage public surface**

Make storage codecs, column-family names, metadata keys, record codecs, and `SCHEMA_VERSION` crate-private. Remove their `pub use` entries from `oregon-storage/src/lib.rs`. Change storage unit tests to import from `crate::codec`, `crate::db`, `crate::records`, and `crate::schema`. Keep only typed cross-crate APIs public.

- [ ] **Step 5: Verify no cross-layer leak remains**

Run:

```bash
rg -n "encode_outpoint_key|CF_(BLOCKS|BLOCK_INDEX|UTXO|UNDO|CHAIN_META)|from_persisted_entries|DuplicatePersistedOutpoint" crates --glob '!crates/oregon-storage/**'
cargo test --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: `rg` returns no matches; all Cargo gates pass.

- [ ] **Step 6: Commit**

```bash
git add crates/oregon-primitives crates/oregon-utxo crates/oregon-storage crates/oregon-chainstate crates/oregon-mempool
git commit -m "refactor: seal UTXO storage boundary"
```

### Task 4: Chainstate responsibility split

**Files:**
- Create: `crates/oregon-chainstate/src/admission.rs`
- Create: `crates/oregon-chainstate/src/transition.rs`
- Create: `crates/oregon-chainstate/src/recovery.rs`
- Create: `crates/oregon-chainstate/src/utxo_delta.rs`
- Modify: `crates/oregon-chainstate/src/state.rs`
- Modify: `crates/oregon-chainstate/src/lib.rs`

**Interfaces:**
- Consumes: `ChainState`, `ChainConfig`, `BranchView`, `ReorgPlan`, `OregonDb`, `StorageBatch`, and semantic `UtxoDelta`.
- Produces: crate-private admission, transition, recovery, and UTXO-delta functions; public `ChainState` API remains unchanged.

- [ ] **Step 1: Record the public API before moving code**

Run:

```bash
cargo rustdoc -p oregon-chainstate -- -D warnings
rg -n "^    pub fn|^pub (struct|enum)" crates/oregon-chainstate/src
```

Save the command output in the task notes of the commit message; do not add a generated API file to the repository.

- [ ] **Step 2: Move recovery as one unit**

Move `bootstrap`, `reopen`, `validate_config`, and their private corruption helper into `recovery.rs`. Expose only the crate-private functions needed by `ChainState::open`; keep persistent invariant checks together.

- [ ] **Step 3: Move semantic UTXO delta handling**

Move `UtxoDelta`, `build_utxo_delta`, disconnect/connect recorders, and batch application into `utxo_delta.rs`. Keep delta ordering semantic and deterministic.

- [ ] **Step 4: Move transitions and admission**

Move direct extension, reorganization, and reorg-plan application to `transition.rs`. Move candidate header/pre-PoW/RandomX/chainwork decision logic to `admission.rs`. Leave the public `accept_block` fault-boundary wrapper in `state.rs`.

- [ ] **Step 5: Run chainstate mutation-sensitive tests**

Run:

```bash
cargo test -p oregon-chainstate --all-targets
cargo test -p oregon-storage recovery
cargo test --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all commands pass; durable-failure, direct-extension, reorg-depth, reorg-cycle atomicity, recovery, and pruning tests remain present and green.

- [ ] **Step 6: Commit**

```bash
git add crates/oregon-chainstate
git commit -m "refactor: separate chainstate responsibilities"
```

### Task 5: Mempool admission and capacity split

**Files:**
- Create: `crates/oregon-mempool/src/admission.rs`
- Create: `crates/oregon-mempool/src/capacity.rs`
- Modify: `crates/oregon-mempool/src/pool.rs`
- Modify: `crates/oregon-mempool/src/lib.rs`

**Interfaces:**
- Consumes: existing graph closure/order functions, eviction comparator, `UtxoState::try_from_entries`, and `SpendVerifier`.
- Produces: crate-private `PreparedCandidate`, `AdmissionPlan`, candidate preparation, capacity planning, and atomic commit helpers; public `Mempool` API remains unchanged.

- [ ] **Step 1: Add a regression assertion for the single byte calculation**

Extend the existing capacity-boundary test so a candidate exactly filling `max_total_bytes` succeeds and the next candidate either deterministically evicts or returns `CapacityRejected`; in both results assert `pool.total_bytes() <= config.max_total_bytes` and unchanged state on rejection.

- [ ] **Step 2: Remove the discarded return value**

Change:

```rust
fn prepare_admission<V: SpendVerifier>(...) -> Result<AdmissionPlan, MempoolError>
```

Delete its `new_total_bytes` calculation and return only the plan. `plan_capacity` remains the sole owner of post-admission byte calculation.

- [ ] **Step 3: Move preparation and capacity code**

Move candidate validation/replay and the admission plan types into `admission.rs`. Move `plan_capacity` and `removed_bytes` into `capacity.rs`. Keep the final mutation in one preflighted commit path so every error occurs before mutation.

- [ ] **Step 4: Run policy and atomicity gates**

Run:

```bash
cargo test -p oregon-mempool --all-targets
cargo test --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: admission, dependency, ancestor/descendant, deterministic eviction, stale-base, reconciliation, and atomicity tests all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/oregon-mempool
git commit -m "refactor: separate mempool policy stages"
```

### Task 6: Test-support consolidation

**Files:**
- Create: `crates/oregon-utxo/src/test_support.rs`
- Create: `crates/oregon-storage/src/test_support.rs`
- Create: `crates/oregon-chainstate/src/test_support.rs`
- Modify: crate unit-test modules that duplicate builders and temporary directories

**Interfaces:**
- Consumes: existing test-only builders and verifier implementations.
- Produces: crate-private, `#[cfg(test)]` helpers; no production or public API.

- [ ] **Step 1: Inventory exact duplicate helpers**

Run:

```bash
rg -n "struct TestDir|struct AcceptAll|fn test_config|fn test_path|fn outpoint|fn spend" crates/oregon-{utxo,storage,chainstate}/src
```

Expected: every match is assigned either to a shared `test_support` helper or retained because its behavior is intentionally test-specific.

- [ ] **Step 2: Centralize only identical semantics**

Move identical RAII temporary-directory handling, accepting spend verifier, UTXO builders, and chain configuration builders into the owning crate's `test_support.rs`. Give specialized rejecting/panic verifiers behavior-specific names and keep them beside the test that owns them.

- [ ] **Step 3: Ensure test support cannot enter production builds**

Declare each module only as:

```rust
#[cfg(test)]
mod test_support;
```

Do not re-export test helpers from a crate root.

- [ ] **Step 4: Run all gates**

Run:

```bash
cargo test --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all commands pass and production public APIs are unchanged.

- [ ] **Step 5: Commit**

```bash
git add crates/oregon-utxo crates/oregon-storage crates/oregon-chainstate
git commit -m "test: unify crate test support"
```

### Task 7: Final architecture, security, and checkpoint gate

**Files:**
- Modify: `docs/architecture/OREGON_ENGINEERING_CONSTITUTION.md` only if verification reveals an inaccurate statement
- Create: `docs/checkpoints/OREGON_V1_ARCHITECTURE_UNIFICATION.md`

**Interfaces:**
- Consumes: all prior task commits and GitHub Actions results.
- Produces: a reviewed architecture-unification checkpoint; no protocol API.

- [ ] **Step 1: Run static architecture scans**

Run:

```bash
rg -n "DeferredTransition|from_persisted_entries|DuplicatePersistedOutpoint|oregon-task[0-9]|mod task[0-9]" crates README.md
rg -n "encode_(outpoint|utxo|block_undo)|decode_(outpoint|utxo|block_undo)|CF_(BLOCKS|BLOCK_INDEX|UTXO|UNDO|CHAIN_META)" crates --glob '!crates/oregon-storage/**'
rg -n "unsafe" crates --glob '!crates/oregon-pow/**'
git diff a2aab4b73489aa0cf21bd7d14f8b930328c3465c -- tests crates/*/tests
```

Expected: the first three scans return no matches. The test diff contains renames, interface adaptations, consolidation, and added coverage but no unexplained deletion of security behavior.

- [ ] **Step 2: Run complete verification**

Run:

```bash
cargo test --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
```

Expected: all commands pass.

- [ ] **Step 3: Verify GitHub Actions**

Push the implementation branch and require successful Rust workspace, RandomX architecture-vector, and RandomX full/light parity workflows. Record run identifiers and conclusions in the checkpoint document.

- [ ] **Step 4: Write and commit the checkpoint**

The checkpoint records baseline and final SHAs, exact deletions/renames, public API changes, frozen-behavior confirmation, CI run identifiers, residual non-production scope, and the statement that `main` remains unchanged.

```bash
git add docs/checkpoints/OREGON_V1_ARCHITECTURE_UNIFICATION.md
git commit -m "docs: record Oregon architecture checkpoint"
```

- [ ] **Step 5: Create the accepted recovery ref**

After review, create `oregon-v1-checkpoint-architecture-unified-accepted-2026-09-04` at the final checkpoint commit and verify the remote SHA exactly. Do not update `main` without a separate integration decision.

