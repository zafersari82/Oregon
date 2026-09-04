# Oregon v1 M5 Mempool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a bounded, deterministic, in-memory Oregon mempool that reuses the accepted consensus/UTXO validation path, supports accepted-parent then child chains, rejects conflicts and missing dependencies, and rebuilds atomically across active-chain changes.

**Architecture:** Add `oregon-mempool` as a policy-only crate depending on primitives, consensus, and UTXO. Admission validates against a narrow temporary `UtxoState`, replaying already accepted unconfirmed ancestors through the existing mandatory `SpendVerifier`; only a fully validated candidate receives a non-mutating capacity/removal plan, and live bookkeeping changes occur after every fallible check. Reconciliation constructs a fresh staged mempool from retained transaction objects against the new chain UTXO snapshot, then replaces the live pool in one final publication step.

**Tech Stack:** Rust `1.85.0`, edition `2024`, `oregon-primitives`, `oregon-consensus`, `oregon-utxo`, `thiserror`, standard-library `BTreeMap`/`BTreeSet`/lookup-only `HashMap`, GitHub Actions locked workspace gates.

**Spec:** `docs/superpowers/specs/2026-09-04-oregon-m5-mempool-design.md`

## Global Constraints

- Development branch is `oregon-v1-m5-mempool`, descended from accepted M4 checkpoint `6ff8168bb79b0f7e1aa015ce910cedaf108614ae`. Do not merge or modify `main`.
- New crate starts with `#![forbid(unsafe_code)]`.
- M5 does not add P2P, transaction relay, orphan storage, RBF, package relay, CPFP package scoring, persistence, wallet/address support, miner RPC, production Schnorr/KeyCommitV1 implementation, testnet/mainnet launch behavior, or new lock-time/sequence semantics.
- Child-before-parent submission is rejected and not retained.
- Block validation and mempool admission use one `validate_normal_transaction_skeleton()` implementation.
- Every candidate input, ancestor replay input, and full-revalidation input goes through the caller-supplied `SpendVerifier`. No production permissive verifier exists.
- Canonical transaction size is `Transaction::encode().len()`; txid is `Transaction::txid()`; consensus transaction ceiling remains exactly `102_400` bytes.
- Default policy limits are exactly `50_000` entries, `64 * 1024 * 1024` canonical bytes, `25` unconfirmed ancestors, and `25` unconfirmed descendants. Ancestor/descendant counts exclude self.
- Validation spend height is `tip_height.checked_add(1)`.
- At most one mempool transaction claims an outpoint. M5 never replaces a conflict by fee.
- Admission/reconciliation are fail-atomic: entry map, spend index, graph, byte counter, and chain base never partially publish.
- Observable ordering never depends on `HashMap`/`HashSet` iteration.
- Topological ready-set ties use ascending txid. Eviction chooses lowest individual fee-rate by exact integer cross multiplication, then lower fee, then ascending txid.
- Eviction of an unconfirmed parent removes its descendant subtree. If capacity would remove the new candidate directly or through one of its ancestors, admission returns `CapacityRejected` and live state is unchanged.
- Active chain is authoritative during reconciliation. Reorg reconciliation never synthesizes/resurrects disconnected transactions not already present.
- TDD sequence for every production task: write RED -> observe intended failure -> minimal GREEN -> focused tests -> full workspace test/fmt/clippy -> commit.
- Security mutants exist only on throwaway branches.

---

## File Map

**Modify**
- `Cargo.toml` — add `crates/oregon-mempool`.
- `Cargo.lock` — Cargo-generated workspace lock update.
- `.github/workflows/oregon-rust.yml` — add M5 development branch trigger without altering pins or permissions.
- `crates/oregon-consensus/src/block.rs` — use shared normal-transaction validator while preserving block error precedence.
- `crates/oregon-consensus/src/error.rs` — add `NormalTransactionError`.
- `crates/oregon-consensus/src/lib.rs` — export validator/error and `MAX_TRANSACTION_BYTES`.

**Create**
- `crates/oregon-mempool/Cargo.toml`
- `crates/oregon-mempool/src/lib.rs`
- `crates/oregon-mempool/src/config.rs`
- `crates/oregon-mempool/src/error.rs`
- `crates/oregon-mempool/src/entry.rs`
- `crates/oregon-mempool/src/graph.rs`
- `crates/oregon-mempool/src/eviction.rs`
- `crates/oregon-mempool/src/pool.rs`
- `crates/oregon-mempool/src/reconcile.rs`
- `crates/oregon-mempool/tests/common/mod.rs`
- `crates/oregon-mempool/tests/admission.rs`
- `crates/oregon-mempool/tests/dependencies.rs`
- `crates/oregon-mempool/tests/eviction.rs`
- `crates/oregon-mempool/tests/reconciliation.rs`

## Frozen Public Types and Signatures

```rust
use std::collections::{BTreeMap, BTreeSet, HashMap};
use oregon_primitives::{Block, Hash256, OutPoint, Transaction};
use oregon_utxo::{SpendVerifier, UtxoState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChainBase {
    pub tip_id: Hash256,
    pub tip_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MempoolConfig {
    pub max_entries: usize,
    pub max_total_bytes: usize,
    pub max_ancestors: usize,
    pub max_descendants: usize,
}

impl Default for MempoolConfig {
    fn default() -> Self {
        Self {
            max_entries: 50_000,
            max_total_bytes: 64 * 1024 * 1024,
            max_ancestors: 25,
            max_descendants: 25,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MempoolEntry {
    transaction: Transaction,
    txid: Hash256,
    fee: u64,
    encoded_bytes: usize,
    parents: BTreeSet<Hash256>,
    children: BTreeSet<Hash256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionOutcome {
    pub txid: Hash256,
    pub fee: u64,
    pub encoded_bytes: usize,
    pub evicted: Vec<Hash256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconcileReport {
    pub removed: Vec<Hash256>,
    pub retained: usize,
}

pub struct Mempool {
    config: MempoolConfig,
    base: ChainBase,
    entries: BTreeMap<Hash256, MempoolEntry>,
    spenders: HashMap<OutPoint, Hash256>,
    total_bytes: usize,
}

impl MempoolEntry {
    pub fn transaction(&self) -> &Transaction;
    pub fn txid(&self) -> Hash256;
    pub fn fee(&self) -> u64;
    pub fn encoded_bytes(&self) -> usize;
    pub fn parents(&self) -> &BTreeSet<Hash256>;
    pub fn children(&self) -> &BTreeSet<Hash256>;
}

impl Mempool {
    pub fn new(base: ChainBase, config: MempoolConfig) -> Result<Self, MempoolError>;
    pub fn base(&self) -> ChainBase;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn total_bytes(&self) -> usize;
    pub fn contains(&self, txid: &Hash256) -> bool;
    pub fn entry(&self, txid: &Hash256) -> Option<&MempoolEntry>;
    pub fn deterministic_order(&self) -> Result<Vec<Hash256>, MempoolError>;

    pub fn admit<V: SpendVerifier>(
        &mut self,
        transaction: Transaction,
        chain_base: ChainBase,
        chain_utxos: &UtxoState,
        verifier: &V,
    ) -> Result<AdmissionOutcome, MempoolError>;

    pub fn reconcile_active_block<V: SpendVerifier>(
        &mut self,
        active_block: &Block,
        new_base: ChainBase,
        chain_utxos: &UtxoState,
        verifier: &V,
    ) -> Result<ReconcileReport, MempoolError>;

    pub fn reconcile_reorg<V: SpendVerifier>(
        &mut self,
        new_base: ChainBase,
        chain_utxos: &UtxoState,
        verifier: &V,
    ) -> Result<ReconcileReport, MempoolError>;
}
```

Both `AdmissionOutcome.evicted` and `ReconcileReport.removed` are sorted ascending by txid bytes before return. This freezes an insertion-order-independent external representation.

## Test Fixtures

`tests/common/mod.rs` defines only test-side permissive/rejecting verifiers:

```rust
pub struct AcceptTestSpends;
impl SpendVerifier for AcceptTestSpends {
    fn verify_spend(
        &self,
        _transaction: &Transaction,
        _input_index: usize,
        _prevout: &UtxoEntry,
    ) -> Result<(), UtxoError> {
        Ok(())
    }
}

pub struct RejectTestSpends;
impl SpendVerifier for RejectTestSpends {
    fn verify_spend(
        &self,
        _transaction: &Transaction,
        _input_index: usize,
        _prevout: &UtxoEntry,
    ) -> Result<(), UtxoError> {
        Err(UtxoError::SpendAuthorizationFailed)
    }
}

pub fn state_with(entries: Vec<(OutPoint, UtxoEntry)>) -> UtxoState {
    UtxoState::from_persisted_entries(entries).unwrap()
}
```

No new UTXO insertion bypass is introduced.

---

### Task 1: Shared Normal-Transaction Structural Validator

**Files:**
- Modify `crates/oregon-consensus/src/error.rs`
- Modify `crates/oregon-consensus/src/block.rs`
- Modify `crates/oregon-consensus/src/lib.rs`

**Produces**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum NormalTransactionError {
    #[error("transaction exceeds canonical byte limit")]
    TooLarge,
    #[error("normal transaction has no inputs")]
    EmptyInputs,
    #[error("normal transaction has no outputs")]
    EmptyOutputs,
    #[error("coinbase form is not a normal transaction")]
    CoinbaseForm,
    #[error("normal transaction uses null outpoint")]
    NullOutpoint,
}

pub fn validate_normal_transaction_skeleton(
    transaction: &Transaction,
) -> Result<(), NormalTransactionError>;
```

- [ ] **Step 1 — RED helper tests.** Add direct tests for valid normal tx and each five error variants. Example:

```rust
#[test]
fn normal_helper_rejects_empty_inputs() {
    let tx = Transaction {
        version: 1,
        inputs: vec![],
        outputs: vec![TxOutput {
            value: Amount::from_base_units(1).unwrap(),
            locking_program: vec![],
        }],
        lock_time: 0,
    };
    assert_eq!(
        validate_normal_transaction_skeleton(&tx),
        Err(NormalTransactionError::EmptyInputs)
    );
}
```

- [ ] **Step 2 — Observe RED.**

```bash
cargo +1.85.0 test --locked -p oregon-consensus normal_helper -- --nocapture
```

Expected: helper/type missing.

- [ ] **Step 3 — Implement helper with exact rejection order.**

```rust
if transaction.encode().len() > MAX_TRANSACTION_BYTES {
    return Err(NormalTransactionError::TooLarge);
}
if transaction.inputs.is_empty() {
    return Err(NormalTransactionError::EmptyInputs);
}
if transaction.outputs.is_empty() {
    return Err(NormalTransactionError::EmptyOutputs);
}
if is_coinbase_form(transaction) {
    return Err(NormalTransactionError::CoinbaseForm);
}
let null_txid = Hash256::from_bytes([0u8; 32]);
if transaction.inputs.iter().any(|input| {
    input.previous_txid == null_txid && input.previous_output_index == u32::MAX
}) {
    return Err(NormalTransactionError::NullOutpoint);
}
Ok(())
```

- [ ] **Step 4 — Preserve block error precedence.** Keep block-size check first. For the existing all-transaction size preflight, use direct size check for transaction index `0`; for every normal index call the shared helper and treat only `TooLarge` during this preflight. After Merkle/coinbase checks, call the helper again for each normal tx and map all variants to the existing indexed `ConsensusError` values. This retains the old “oversized tx before later shape/Merkle errors” behavior while making all normal shape decisions come from one helper.

- [ ] **Step 5 — Add parity regressions** for `TransactionTooLarge(index)`, `EmptyNormalTransactionInputs(index)`, `EmptyNormalTransactionOutputs(index)`, `MultipleCoinbase`, and `NullOutpointInNormalTransaction`.

- [ ] **Step 6 — Full gate.**

```bash
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

- [ ] **Step 7 — Commit** `refactor: share Oregon normal transaction validation`.

---

### Task 2: Mempool Crate/Foundation and CI

**Files:** all new crate files from File Map, root `Cargo.toml`, `Cargo.lock`, `.github/workflows/oregon-rust.yml`.

**Produces:** public types/accessors above and complete error enum consumed later.

- [ ] **Step 1 — Add manifest/workspace member.**

```toml
[package]
name = "oregon-mempool"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true

[dependencies]
oregon-consensus = { path = "../oregon-consensus" }
oregon-primitives = { path = "../oregon-primitives" }
oregon-utxo = { path = "../oregon-utxo" }
thiserror = "2"
```

Run Cargo once to generate the lock update; do not invent lock entries manually.

- [ ] **Step 2 — Add `oregon-v1-m5-mempool` to CI push branches** without altering the checkout pin, permissions, Rust version, RocksDB prerequisites, or gate commands.

- [ ] **Step 3 — RED default/config tests.**

```rust
#[test]
fn default_limits_are_exact() {
    let c = MempoolConfig::default();
    assert_eq!(c.max_entries, 50_000);
    assert_eq!(c.max_total_bytes, 64 * 1024 * 1024);
    assert_eq!(c.max_ancestors, 25);
    assert_eq!(c.max_descendants, 25);
}

#[test]
fn zero_entry_capacity_is_invalid() {
    let config = MempoolConfig { max_entries: 0, ..MempoolConfig::default() };
    assert!(matches!(
        Mempool::new(base(1, 10), config),
        Err(MempoolError::InvalidConfig)
    ));
}
```

Also prove `max_total_bytes = 0` invalid, while `max_ancestors = 0` and `max_descendants = 0` are valid intentional policies.

- [ ] **Step 4 — Observe RED** because crate/types are missing.

- [ ] **Step 5 — Implement exact structs/accessors and error enum.**

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MempoolError {
    #[error("invalid mempool configuration")]
    InvalidConfig,
    #[error("chain height overflow")]
    HeightOverflow,
    #[error("transaction already known: {0}")]
    AlreadyKnown(Hash256),
    #[error("mempool chain context is stale")]
    StaleChainContext,
    #[error("mempool conflict on {outpoint:?}; existing transaction {existing_txid}")]
    Conflict { outpoint: OutPoint, existing_txid: Hash256 },
    #[error("missing transaction dependency: {0:?}")]
    MissingDependency(OutPoint),
    #[error("parent output does not exist: {0:?}")]
    InvalidParentOutput(OutPoint),
    #[error("too many unconfirmed ancestors")]
    TooManyAncestors,
    #[error("too many unconfirmed descendants")]
    TooManyDescendants,
    #[error("mempool capacity rejected transaction")]
    CapacityRejected,
    #[error("mempool dependency cycle")]
    DependencyCycle,
    #[error("mempool invariant violation")]
    InvariantViolation,
    #[error(transparent)]
    Structural(#[from] NormalTransactionError),
    #[error(transparent)]
    Utxo(#[from] UtxoError),
}
```

`lib.rs` wires all seven private modules and exports only config/entry/error/pool public types. `graph.rs`, `eviction.rs`, and `reconcile.rs` are created as empty Rust source files in this commit so module resolution is valid; they gain behavior in later tasks.

- [ ] **Step 6 — Full gate and commit** `feat: add Oregon mempool foundation`.

---

### Task 3: Chain-Backed Atomic Admission

**Files:** `src/pool.rs`, `src/entry.rs`, `tests/common/mod.rs`, `tests/admission.rs`.

**Internal preparation types**

```rust
struct PreparedCandidate {
    entry: MempoolEntry,
    spend_claims: Vec<OutPoint>,
    ancestors: BTreeSet<Hash256>,
}

struct AdmissionPlan {
    candidate: PreparedCandidate,
    remove: BTreeSet<Hash256>,
}
```

Task 3 uses an empty `ancestors` set and empty `remove`; later tasks populate them without changing the public API.

- [ ] **Step 1 — RED valid admission.**

```rust
#[test]
fn valid_chain_backed_transaction_records_fee_and_size() {
    let previous = outpoint(0x11, 0);
    let chain = state_with(vec![(previous, entry(100, 1, false))]);
    let tx = spend(vec![previous], &[60, 30], 1);
    let base = base(0x22, 20);
    let mut pool = Mempool::new(base, MempoolConfig::default()).unwrap();
    let out = pool.admit(tx.clone(), base, &chain, &AcceptTestSpends).unwrap();
    assert_eq!(out.fee, 10);
    assert_eq!(out.encoded_bytes, tx.encode().len());
    assert_eq!(pool.total_bytes(), tx.encode().len());
}
```

- [ ] **Step 2 — RED failure atomicity** for duplicate txid, different tx spending an already claimed outpoint, missing chain input, structural rejection, rejecting verifier, stale base, and `tip_height == u64::MAX`. Snapshot public pool observations before each call and require exact equality afterward.

- [ ] **Step 3 — RED coinbase maturity boundary.** A coinbase entry created at height `10` with base height `128` validates candidate at height `129` and must reject; base height `129` validates at `130` and must accept.

- [ ] **Step 4 — Observe RED.**

- [ ] **Step 5 — Implement admission preflight in exact order:** base equality -> checked next height -> canonical bytes/txid -> shared structural helper -> duplicate txid -> live spend-index conflicts -> every input available in chain UTXO -> narrow `UtxoState::from_persisted_entries` -> `apply_normal_transaction` -> checked byte total -> construct `PreparedCandidate`/`AdmissionPlan`.

- [ ] **Step 6 — Publish after last fallible check.** The commit section inserts spend claims, entry, and precomputed `total_bytes`; it returns no validation/policy error after the first live-field mutation.

- [ ] **Step 7 — Full gate and commit** `feat: admit Oregon mempool transactions atomically`.

---

### Task 4: Dependency Graph, Parent Replay, Exact 25/25 Limits, Deterministic Topology

**Files:** `src/graph.rs`, `src/pool.rs`, `src/entry.rs`, `tests/dependencies.rs`.

**Produces**

```rust
pub(crate) fn ancestor_closure(
    entries: &BTreeMap<Hash256, MempoolEntry>,
    direct_parents: &BTreeSet<Hash256>,
) -> Result<BTreeSet<Hash256>, MempoolError>;

pub(crate) fn descendant_closure(
    entries: &BTreeMap<Hash256, MempoolEntry>,
    root: Hash256,
) -> Result<BTreeSet<Hash256>, MempoolError>;

pub(crate) fn topological_order(
    entries: &BTreeMap<Hash256, MempoolEntry>,
) -> Result<Vec<Hash256>, MempoolError>;
```

- [ ] **Step 1 — RED parent-then-child / child-before-parent.** Child-first returns `MissingDependency` and pool stays empty. Parent then child succeeds, and `deterministic_order()` is parent then child.

- [ ] **Step 2 — RED invalid parent index.** If parent tx exists but referenced output index is `>= parent.outputs.len()`, return `InvalidParentOutput` with no mutation.

- [ ] **Step 3 — RED exact limits.** With `max_ancestors = 2`, a candidate with exactly two unique ancestors is allowed and three is `TooManyAncestors`. With `max_descendants = 2`, exactly two unique descendants per ancestor is allowed and a third is `TooManyDescendants`. Each rejection preserves pool state.

- [ ] **Step 4 — RED topology determinism.** Equivalent independent tx sets admitted in opposite orders produce the same ascending-txid ready ordering; DAG edges always force parent before child.

- [ ] **Step 5 — Implement cycle-safe closures and Kahn topology.** Use `BTreeSet<Hash256>` as ready set. If emitted count differs from entry count, return `DependencyCycle`.

- [ ] **Step 6 — Dependency discovery order per input:** existing live spend conflict -> if chain UTXO contains outpoint, chain-backed -> else if parent txid exists in pool, output index must exist and parent is recorded -> otherwise `MissingDependency`.

- [ ] **Step 7 — Narrow ancestor replay.** Seed only chain-backed outpoints needed by ancestor closure and candidate. Replay ancestors in deterministic parent-before-child order through `apply_normal_transaction`. Require replayed fee to equal stored entry fee; mismatch is `InvariantViolation`. Apply candidate last through the same verifier path.

- [ ] **Step 8 — Limit preflight.** Candidate unique ancestor closure length must be `<= max_ancestors`. Every unique ancestor’s current descendant closure length plus one must be `<= max_descendants`, using checked addition.

- [ ] **Step 9 — Atomic graph publication.** Preflight all parent entries and byte/count arithmetic; then insert candidate and reciprocal parent child-links with no remaining error path.

- [ ] **Step 10 — Full gate and commit** `feat: add Oregon mempool dependency graph`.

---

### Task 5: Deterministic Bounded Capacity and Eviction

**Files:** `src/eviction.rs`, `src/graph.rs`, `src/pool.rs`, `tests/eviction.rs`.

**Produces**

```rust
pub(crate) fn eviction_cmp(
    left: &MempoolEntry,
    right: &MempoolEntry,
) -> std::cmp::Ordering;
```

- [ ] **Step 1 — RED comparator.** Compare rates only with:

```rust
let left_cross = u128::from(left.fee()) * right.encoded_bytes() as u128;
let right_cross = u128::from(right.fee()) * left.encoded_bytes() as u128;
```

Lower cross-ratio is evicted first; equal rate -> lower fee -> smaller txid.

- [ ] **Step 2 — RED hard bounds.** Tiny configs prove equality with entry/byte limit is accepted and eviction starts only when virtual state is greater than a limit.

- [ ] **Step 3 — RED subtree behavior.** If low-fee parent is selected, parent and every current descendant are removed; no survivor points at a removed parent.

- [ ] **Step 4 — RED candidate-self-eviction rollback.** With `max_entries = 1`, a worse candidate against a better existing tx returns `CapacityRejected` and every public pool observation is unchanged.

- [ ] **Step 5 — RED insertion-order independence.** Logically identical pools built in different independent order evict the same txid set.

- [ ] **Step 6 — Implement non-mutating virtual plan.** Start from validated `PreparedCandidate`, maintain `BTreeSet<Hash256> remove`. Virtual counts are checked `existing + candidate - removed`. Select lowest eviction priority among existing-not-removed plus candidate. If candidate itself is selected, reject. If selected existing root belongs to `candidate.ancestors`, candidate would be its descendant, so reject. Otherwise add root plus its full existing descendant closure to `remove`; repeat until limits pass.

- [ ] **Step 7 — Preflight removal consistency.** Before mutating, every txid in `remove` must exist; each stored input spend claim must point back to that txid; reciprocal parent/child edges must be internally consistent; checked byte subtraction/addition must succeed. Any mismatch is `InvariantViolation` before publication.

- [ ] **Step 8 — Commit.** Remove planned entries/spend claims/reciprocal edges in ascending txid order, then insert candidate. Return `evicted` as the sorted removed txid vector.

- [ ] **Step 9 — Full gate and commit** `feat: bound Oregon mempool with deterministic eviction`.

---

### Task 6: Active-Block Reconciliation and Confirmed-Parent Promotion

**Files:** `src/reconcile.rs`, `src/pool.rs`, `src/graph.rs`, `tests/reconciliation.rs`.

**Produces internal rebuild**

```rust
fn rebuild_against_chain<V: SpendVerifier>(
    &self,
    ordered_source: &[(Hash256, Transaction)],
    new_base: ChainBase,
    chain_utxos: &UtxoState,
    verifier: &V,
) -> Result<Mempool, MempoolError>;
```

The helper creates `Mempool::new(new_base, self.config.clone())`, validates/replays source transactions into that staged pool, and never touches `self`.

- [ ] **Step 1 — RED confirmed-parent promotion.** Parent->child is in pool. New chain snapshot contains parent output and active block contains parent. After reconciliation, parent is absent, child remains, child parent set is empty, and base equals new base.

- [ ] **Step 2 — RED active-chain conflict.** Pool A spends X and has descendant C; active block contains different B spending X. A and C are removed.

- [ ] **Step 3 — RED ordinary tip update.** Unrelated valid entries survive; an entry whose chain input disappeared is filtered.

- [ ] **Step 4 — RED atomic invariant failure** in a crate-unit test that constructs broken reciprocal internal graph state under `#[cfg(test)]`; reconciliation returns `InvariantViolation` and original base/entries/bytes remain unchanged. No corruption constructor is exported.

- [ ] **Step 5 — Stage preprocessing.** Compute original deterministic topology before removal. Build a source list in that order. Remove confirmed txids from the source without recursively removing their children. Remove active-block conflicting roots plus their descendants from source. Do not touch live pool.

- [ ] **Step 6 — Rebuild against actual input availability.** For each source tx: if input exists in new chain UTXO it is chain-backed; otherwise it may depend only on an earlier successfully retained staged transaction. Missing dependencies/structural/UTXO/verifier failures are expected transaction invalidity during rebuild and filter that tx; its unchain-backed descendants subsequently fail naturally. Internal cycle/bookkeeping failures abort the whole rebuild.

- [ ] **Step 7 — Reapply normal admission limits/capacity** to staged state; therefore rebuilt entries receive the same 25/25/bounded/deterministic policy as new admission.

- [ ] **Step 8 — Final publication.** Compute old minus rebuilt txids, sort ascending, then assign `*self = rebuilt`; return report. No base update occurs before this assignment.

- [ ] **Step 9 — Full gate and commit** `feat: reconcile Oregon mempool with active blocks`.

---

### Task 7: Reorg Revalidation and Recovery Matrix

**Files:** `src/reconcile.rs`, `src/pool.rs`, `tests/reconciliation.rs`, `tests/admission.rs`, `tests/dependencies.rs`.

- [ ] **Step 1 — RED retained-valid reorg.** Compatible new UTXO snapshot retains valid txs and advances base.

- [ ] **Step 2 — RED disappeared confirmed parent.** A child previously promoted to chain-backed loses that chain output after reorg; it is removed. No parent transaction is created.

- [ ] **Step 3 — RED non-resurrection.** A disconnected transaction absent from current pool remains absent because `reconcile_reorg` accepts no disconnected transaction bodies.

- [ ] **Step 4 — RED stale-context gate.** Admission with a new tip before reconciliation is `StaleChainContext`; after successful reconciliation, admission using that exact base may proceed.

- [ ] **Step 5 — RED deterministic rebuild.** Logically identical pools with different independent insertion history reconcile to identical txid order, byte count, entry fee/size/parent metadata, and sorted removed vector.

- [ ] **Step 6 — Implement `reconcile_reorg` as staged rebuild.** Compute current deterministic source topology, clone only `(txid, Transaction)` into ordered source, call `rebuild_against_chain`, compute sorted removed vector, then single final `*self = rebuilt` publication.

- [ ] **Step 7 — Recovery matrix tests:** `tip_height = u64::MAX` returns `HeightOverflow` before publication; zero-fee transaction is valid if capacity allows; changed witness changes txid and canonical bytes; rejecting verifier during rebuild filters affected tx; internal graph cycle aborts rebuild and preserves old live state; unordered lookup insertion never changes observable outputs.

- [ ] **Step 8 — Full gate and commit** `feat: revalidate Oregon mempool across reorgs`.

---

### Task 8: Security Mutations, Review, and M5 Checkpoint

**Files:** throwaway mutation branches; clean-branch checkpoint `docs/checkpoints/OREGON_V1_M5_MEMPOOL.md`.

- [ ] **Step 1 — Fresh pre-mutation CI** on exact reviewed M5 code SHA. Record SHA plus test/fmt/clippy run and job IDs.

- [ ] **Step 2 — Mutation A: conflict bypass.** Fresh throwaway branch; ignore/overwrite existing outpoint spend claim. Direct conflict and consistency tests must fail for the intended reason.

- [ ] **Step 3 — Mutation B: missing-parent bypass.** Fresh throwaway branch; accept an input absent from chain UTXO and mempool parents. Child-before-parent and missing-dependency tests must fail.

- [ ] **Step 4 — Mutation C: early publication.** Fresh throwaway branch; move one live entry/spend/graph/byte mutation before final verifier or capacity success. Failure-atomicity and candidate-self-eviction tests must fail.

- [ ] **Step 5 — Boundary mutation if review reveals weak coverage.** Change ancestor/descendant exact comparison by one. Exact-boundary tests must fail. This supplements rather than replaces A/B/C.

- [ ] **Step 6 — Fresh post-mutation clean CI** on clean branch and verify mutation commits are absent from clean ancestry/diff.

- [ ] **Step 7 — Manual M4->M5 review** covers: single structural helper; unchanged block error behavior; mandatory verifier on candidate/ancestors/rebuild; exact maturity height; one-spender invariant/no RBF; orphan rejection; parent index bounds; unique 25/25 closure boundaries; checked arithmetic; integer-only fee-rate order; deterministic txid ties; non-mutating capacity plan; candidate-self-eviction rollback; reciprocal graph/spend consistency; confirmed-parent promotion; active-chain conflict descendants; staged rebuild/base publication; reorg non-resurrection; no M4 storage/chainstate dependency regression; no unsafe Rust; no unexpected production dependency.

- [ ] **Step 8 — Write checkpoint from observed evidence only.** Record accepted M4 base, final M5 code/checkpoint SHAs, design/plan paths, exact CI run IDs, mutation branch/commit/run/killed tests, manual review disposition, and exclusions. No future-state evidence is recorded.

- [ ] **Step 9 — Final checkpoint CI** on checkpoint commit.

- [ ] **Step 10 — Create accepted recovery branch** `oregon-v1-checkpoint-m5-mempool-accepted-2026-09-04` exactly at checkpoint commit; verify identical SHA and verify `main` remains unchanged.

---

## Definition of M5 Accepted

- `oregon-mempool` implements the approved policy-only scope with no persistence/network/wallet/miner expansion.
- Block/mempool structural validation shares one authoritative helper and M1-M4 regressions remain green.
- Valid chain-backed and accepted-parent child transactions admit through mandatory `SpendVerifier`; child-before-parent is not retained.
- Conflict, exact ancestor/descendant limits, byte/entry bounds, deterministic topology, deterministic eviction, and candidate-self-eviction rollback are tested.
- Active-block confirmation/conflict handling and reorg full rebuild publish atomically and do not invent disconnected transactions.
- Workspace tests, rustfmt, and clippy pass at the reviewed code and final checkpoint commits.
- Mutations A/B/C are killed by intended tests; fresh post-mutation clean CI is green.
- Manual M4->M5 review has no known Critical or Important finding open.
- Accepted recovery branch points exactly to final checkpoint commit.
- `main` is not merged or modified.
