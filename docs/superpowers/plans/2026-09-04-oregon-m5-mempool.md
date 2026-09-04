# Oregon v1 M5 Mempool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a bounded, deterministic, in-memory Oregon mempool that reuses authoritative consensus/UTXO validation, supports parent-child unconfirmed chains, rejects conflicts/orphans, reconciles safely across active-chain changes, and cannot partially publish failed admission or rebuild state.

**Architecture:** Introduce a focused `oregon-mempool` crate that depends only on primitives, consensus, and UTXO layers. It never owns RocksDB and never becomes part of consensus. Admission builds a narrow temporary `UtxoState`, replays accepted unconfirmed ancestors through the existing mandatory `SpendVerifier` path, validates the candidate through the same path, computes a complete mutation/eviction plan without mutating live state, and only then commits infallible bookkeeping changes. Full reconciliation constructs a brand-new staged mempool against the new active-chain UTXO snapshot and publishes it only after all invariants pass.

**Tech Stack:** Rust `1.85.0`, edition `2024`, existing `oregon-primitives`, `oregon-consensus`, `oregon-utxo`, `thiserror`, standard-library ordered collections, GitHub Actions with locked workspace test/fmt/clippy gates.

**Spec:** `docs/superpowers/specs/2026-09-04-oregon-m5-mempool-design.md`

## Global Constraints

- Work on `oregon-v1-m5-mempool`, starting from accepted M4 checkpoint `6ff8168bb79b0f7e1aa015ce910cedaf108614ae`; do not merge or modify `main`.
- New M5 crate must contain `#![forbid(unsafe_code)]`.
- M5 is policy, not consensus. Do not add P2P, orphan storage, RBF, package relay, CPFP package scoring, persistence, wallet, miner RPC, testnet/mainnet launch behavior, or new lock-time/sequence semantics.
- A child submitted before its parent is rejected and not retained.
- No production `AcceptAll` or other permissive `SpendVerifier` is allowed.
- Every normal spend, including ancestor replay and full revalidation, must pass the caller-supplied `SpendVerifier`.
- Block validation and mempool admission must share one authoritative `validate_normal_transaction_skeleton()` implementation.
- Canonical transaction byte length is exactly `Transaction::encode().len()`; txid is exactly `Transaction::txid()`.
- Consensus transaction byte ceiling remains `MAX_TRANSACTION_BYTES = 102_400`.
- Mempool default limits are exactly: `50_000` entries, `64 * 1024 * 1024` canonical transaction bytes, `25` unconfirmed ancestors, `25` unconfirmed descendants.
- Ancestor/descendant counts exclude the transaction itself.
- Mempool admission validates at next possible spend height `tip_height.checked_add(1)`.
- One mempool spender per outpoint. No fee-based replacement in M5.
- Admission and reconciliation are fail-atomic: no partial entry/index/graph/byte/base publication.
- Externally observed order and eviction must never depend on `HashMap`/`HashSet` iteration order.
- Deterministic ready-set/topological ties use ascending txid bytes.
- Eviction selects the lowest individual fee-rate using exact integer cross multiplication; equal rate -> lower absolute fee -> lexicographically smaller txid first.
- Evicting an entry evicts its current descendant subtree unless confirmation has explicitly promoted a child dependency to chain-backed state.
- If capacity planning would evict the new candidate, reject admission and leave the original pool unchanged.
- Active-chain state is authoritative during reconciliation.
- Reorg reconciliation does not resurrect disconnected transactions not already in the pool.
- TDD is mandatory: RED test -> observe intended failure -> minimal GREEN -> focused test -> full workspace gate -> commit.
- Each task ends with fresh `cargo +1.85.0 test --locked --workspace --all-targets`, `cargo +1.85.0 fmt --all -- --check`, and `cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings`.
- Required security mutations live only on throwaway branches and never enter the clean M5 branch.

---

## File Structure

### Existing files modified

- `Cargo.toml` — add `crates/oregon-mempool` workspace member.
- `Cargo.lock` — Cargo-generated local package/dependency lock update.
- `.github/workflows/oregon-rust.yml` — add `oregon-v1-m5-mempool` push trigger while preserving read-only permissions and all existing pins.
- `crates/oregon-consensus/src/block.rs` — move normal-transaction shape checks behind the shared helper while preserving current block error precedence.
- `crates/oregon-consensus/src/error.rs` — add focused `NormalTransactionError`.
- `crates/oregon-consensus/src/lib.rs` — export shared normal-transaction validator/error and `MAX_TRANSACTION_BYTES` for policy consumers.

### New `oregon-mempool` files

- `crates/oregon-mempool/Cargo.toml` — Oregon dependencies and `thiserror` only; no persistence/network dependency.
- `crates/oregon-mempool/src/lib.rs` — `#![forbid(unsafe_code)]`, module wiring, public exports.
- `crates/oregon-mempool/src/config.rs` — `MempoolConfig`, defaults and validation.
- `crates/oregon-mempool/src/error.rs` — typed policy/invariant errors.
- `crates/oregon-mempool/src/entry.rs` — `ChainBase`, `MempoolEntry`, `AdmissionOutcome`, `ReconcileReport`.
- `crates/oregon-mempool/src/graph.rs` — ancestor/descendant closure, deterministic topological ordering, cycle detection.
- `crates/oregon-mempool/src/eviction.rs` — exact fee-rate comparison and non-mutating capacity removal planning.
- `crates/oregon-mempool/src/pool.rs` — pool ownership, admission preflight, narrow UTXO replay, atomic bookkeeping commit/removal.
- `crates/oregon-mempool/src/reconcile.rs` — active-block and reorg staged rebuild paths.

### New integration tests

- `crates/oregon-mempool/tests/common/mod.rs` — test-only transaction/UTXO/verifier fixtures.
- `crates/oregon-mempool/tests/admission.rs` — chain-backed admission, structural parity, conflict, maturity, stale-base and failure atomicity.
- `crates/oregon-mempool/tests/dependencies.rs` — parent-child, orphan rejection, ancestor/descendant limits, deterministic graph ordering.
- `crates/oregon-mempool/tests/eviction.rs` — hard bounds, exact fee-rate/tie ordering, subtree eviction, candidate-self-eviction rollback.
- `crates/oregon-mempool/tests/reconciliation.rs` — confirmation, active-chain conflicts, tip changes, reorg full rebuild, non-resurrection and atomic failure.

## Public Interface Contract

Later tasks must use these exact names unless a compiler-required mechanical adjustment is documented in the commit.

```rust
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

pub struct MempoolEntry { /* private fields */ }

impl MempoolEntry {
    pub fn transaction(&self) -> &Transaction;
    pub fn txid(&self) -> Hash256;
    pub fn fee(&self) -> u64;
    pub fn encoded_bytes(&self) -> usize;
    pub fn parents(&self) -> &BTreeSet<Hash256>;
    pub fn children(&self) -> &BTreeSet<Hash256>;
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

pub struct Mempool { /* private state */ }

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

`removed` and `evicted` vectors are externally observable and must be deterministic. `ReconcileReport.removed` is sorted ascending by txid bytes. `AdmissionOutcome.evicted` records capacity-removal roots/subtree members in deterministic commit order; tests must freeze that ordering.

## Test Fixture Contract

All permissive verifiers stay in integration-test code only:

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
```

Chain UTXO fixtures use the production checked restoration API rather than adding a new insertion bypass:

```rust
pub fn state_with(entries: Vec<(OutPoint, UtxoEntry)>) -> UtxoState {
    UtxoState::from_persisted_entries(entries).unwrap()
}
```

---

### Task 1: One Authoritative Normal-Transaction Skeleton Validator

**Files:**
- Modify: `crates/oregon-consensus/src/error.rs`
- Modify: `crates/oregon-consensus/src/block.rs`
- Modify: `crates/oregon-consensus/src/lib.rs`

**Interfaces:**
- Produces: `NormalTransactionError::{TooLarge, EmptyInputs, EmptyOutputs, CoinbaseForm, NullOutpoint}`.
- Produces: `validate_normal_transaction_skeleton(transaction: &Transaction) -> Result<(), NormalTransactionError>`.
- Produces: public `MAX_TRANSACTION_BYTES` re-export.
- Preserves: every existing `validate_non_genesis_block_skeleton()` externally observed `ConsensusError` and block-level ordering.

- [ ] **Step 1: Write RED focused helper tests** in `block.rs`.

```rust
#[test]
fn normal_transaction_helper_rejects_empty_inputs() {
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

#[test]
fn normal_transaction_helper_rejects_coinbase_form() {
    assert_eq!(
        validate_normal_transaction_skeleton(&coinbase(2)),
        Err(NormalTransactionError::CoinbaseForm)
    );
}
```

Add equivalent direct tests for `EmptyOutputs`, `NullOutpoint`, valid normal transaction, and encoded size > `MAX_TRANSACTION_BYTES` -> `TooLarge`.

- [ ] **Step 2: Run RED**.

```bash
cargo +1.85.0 test --locked -p oregon-consensus normal_transaction_helper -- --nocapture
```

Expected: compilation failure because helper/error do not exist. A workflow/toolchain failure does not count as RED.

- [ ] **Step 3: Implement focused error and helper**.

Helper rejection order is exactly:

```rust
pub fn validate_normal_transaction_skeleton(
    transaction: &Transaction,
) -> Result<(), NormalTransactionError> {
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
}
```

- [ ] **Step 4: Refactor block validator without changing error precedence**.

Keep the current block-size check first. Keep the all-transaction maximum-size preflight so an oversized normal transaction continues to beat later Merkle/shape errors. For normal entries, source the size result from the authoritative helper where possible, then run the helper again in the existing post-Merkle normal-transaction loop and map:

```rust
match validate_normal_transaction_skeleton(transaction) {
    Ok(()) => {}
    Err(NormalTransactionError::TooLarge) => {
        return Err(ConsensusError::TransactionTooLarge(index));
    }
    Err(NormalTransactionError::EmptyInputs) => {
        return Err(ConsensusError::EmptyNormalTransactionInputs(index));
    }
    Err(NormalTransactionError::EmptyOutputs) => {
        return Err(ConsensusError::EmptyNormalTransactionOutputs(index));
    }
    Err(NormalTransactionError::CoinbaseForm) => {
        return Err(ConsensusError::MultipleCoinbase);
    }
    Err(NormalTransactionError::NullOutpoint) => {
        return Err(ConsensusError::NullOutpointInNormalTransaction);
    }
}
```

Do not route the first coinbase transaction through this helper.

- [ ] **Step 5: Add parity regression tests** proving the old block errors remain exact for oversized tx, empty inputs, empty outputs, second coinbase and null outpoint.

- [ ] **Step 6: Full gate**.

```bash
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

- [ ] **Step 7: Commit** `refactor: share Oregon normal transaction validation`.

---

### Task 2: Mempool Crate Foundation, Configuration, Base Identity, and CI Gate

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `.github/workflows/oregon-rust.yml`
- Create: `crates/oregon-mempool/Cargo.toml`
- Create: `crates/oregon-mempool/src/lib.rs`
- Create: `crates/oregon-mempool/src/config.rs`
- Create: `crates/oregon-mempool/src/error.rs`
- Create: `crates/oregon-mempool/src/entry.rs`
- Create: `crates/oregon-mempool/src/pool.rs`
- Create: `crates/oregon-mempool/tests/common/mod.rs`
- Create: `crates/oregon-mempool/tests/admission.rs`

**Interfaces:**
- Produces `ChainBase`, `MempoolConfig`, `MempoolEntry`, `AdmissionOutcome`, `ReconcileReport`, `MempoolError`, and read-only `Mempool` accessors from the public contract above.
- Internal live collections: `BTreeMap<Hash256, MempoolEntry>` for entries; `HashMap<OutPoint, Hash256>` only for one-spender lookup; explicit sorted structures for observable order.

- [ ] **Step 1: Add crate manifest and workspace member**.

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

Regenerate `Cargo.lock` using Cargo; do not hand-invent third-party versions.

- [ ] **Step 2: Add M5 branch to CI trigger** while preserving every existing branch, checkout SHA, read-only permissions, RocksDB prerequisite step and locked commands.

- [ ] **Step 3: Write RED config/base tests**.

```rust
#[test]
fn default_limits_are_frozen() {
    let config = MempoolConfig::default();
    assert_eq!(config.max_entries, 50_000);
    assert_eq!(config.max_total_bytes, 64 * 1024 * 1024);
    assert_eq!(config.max_ancestors, 25);
    assert_eq!(config.max_descendants, 25);
}

#[test]
fn zero_hard_capacity_is_rejected() {
    let base = ChainBase { tip_id: hash(1), tip_height: 10 };
    let config = MempoolConfig {
        max_entries: 0,
        ..MempoolConfig::default()
    };
    assert_eq!(Mempool::new(base, config), Err(MempoolError::InvalidConfig));
}
```

`max_ancestors = 0` and `max_descendants = 0` are valid policy settings that intentionally disable unconfirmed chains; only `max_entries == 0` or `max_total_bytes == 0` is invalid.

- [ ] **Step 4: Run RED** with missing crate/types as the intended failure.

- [ ] **Step 5: Implement minimal foundation**.

`lib.rs` begins:

```rust
#![forbid(unsafe_code)]

mod config;
mod entry;
mod error;
mod eviction;
mod graph;
mod pool;
mod reconcile;

pub use config::MempoolConfig;
pub use entry::{AdmissionOutcome, ChainBase, MempoolEntry, ReconcileReport};
pub use error::MempoolError;
pub use pool::Mempool;
```

At this task, `eviction`, `graph`, and `reconcile` may be empty private modules containing no production behavior so module paths are stable for later tasks. Do not add placeholder comments such as TODO.

Define `MempoolError` now with the complete variants later tasks consume:

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
    #[error("mempool parent does not contain referenced output: {0:?}")]
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

- [ ] **Step 6: Verify public read-only accessors and defaults**, then full workspace gate.

- [ ] **Step 7: Commit** `feat: add Oregon mempool foundation`.

---

### Task 3: Atomic Chain-Backed Admission, Conflict Rejection, and Narrow UTXO Validation

**Files:**
- Modify: `crates/oregon-mempool/src/pool.rs`
- Modify: `crates/oregon-mempool/src/entry.rs`
- Modify: `crates/oregon-mempool/tests/common/mod.rs`
- Modify: `crates/oregon-mempool/tests/admission.rs`

**Interfaces:**
- Produces full `Mempool::admit(...)` for transactions whose inputs are chain-backed; dependency support is added in Task 4 without changing the signature.
- Internal helper: `fn next_spend_height(base: ChainBase) -> Result<u64, MempoolError>`.
- Internal helper: `fn seed_chain_inputs(...) -> Result<UtxoState, MempoolError>` using `UtxoState::from_persisted_entries`.
- No live pool mutation occurs until structural, context, conflict, dependency presence, UTXO and verifier checks have passed.

- [ ] **Step 1: RED valid admission test**.

```rust
#[test]
fn valid_chain_backed_transaction_records_exact_fee_and_bytes() {
    let previous = outpoint(0x11, 0);
    let chain = state_with(vec![(previous, entry(100, 1, false))]);
    let tx = spend(vec![previous], &[60, 30], 1);
    let base = base(0x22, 20);
    let mut pool = Mempool::new(base, MempoolConfig::default()).unwrap();

    let outcome = pool
        .admit(tx.clone(), base, &chain, &AcceptTestSpends)
        .unwrap();

    assert_eq!(outcome.txid, tx.txid());
    assert_eq!(outcome.fee, 10);
    assert_eq!(outcome.encoded_bytes, tx.encode().len());
    assert!(outcome.evicted.is_empty());
    assert_eq!(pool.len(), 1);
    assert_eq!(pool.total_bytes(), tx.encode().len());
}
```

- [ ] **Step 2: Add RED failure-atomicity tests** for exact duplicate txid, different tx spending same outpoint, missing input, structural rejection, `RejectTestSpends`, immature coinbase at next height, and stale chain base. For each, snapshot `len`, `total_bytes`, `base`, deterministic order, and any existing entry metadata before call; assert exact equality afterward.

Coinbase boundary test uses creation height `10`: base height `128` validates at spend height `129` and must reject; base height `129` validates at spend height `130` and must accept.

- [ ] **Step 3: Run RED** and verify failures are missing admission behavior, not fixture errors.

- [ ] **Step 4: Implement admission preflight in this exact order**:

```text
1. chain_base == self.base
2. checked next spend height
3. canonical txid + encoded length
4. validate_normal_transaction_skeleton
5. duplicate txid
6. each input: existing spend-index conflict
7. each input must exist in chain UTXO for Task 3
8. build narrow UtxoState from referenced chain entries
9. apply_normal_transaction(candidate, next_height, verifier)
10. prepare entry and all spend claims in local values
11. publish entry/spend claims/total_bytes with no remaining fallible operation
```

Do not clone the whole chain UTXO state. Deduplicate seed outpoints with a lookup-only `HashSet`; `UtxoState::from_persisted_entries` remains the checked constructor.

- [ ] **Step 5: Publish only after validation**.

The final mutation section must contain no validator call and no policy error return after the first live field is changed. Compute `new_total_bytes` with `checked_add` before mutation; arithmetic failure maps to `InvariantViolation`.

- [ ] **Step 6: Focused tests and full gate**.

- [ ] **Step 7: Commit** `feat: admit Oregon mempool transactions atomically`.

---

### Task 4: Parent-Child Dependencies, Orphan Rejection, Graph Limits, and Deterministic Topology

**Files:**
- Modify: `crates/oregon-mempool/src/graph.rs`
- Modify: `crates/oregon-mempool/src/pool.rs`
- Modify: `crates/oregon-mempool/src/entry.rs`
- Create/Modify: `crates/oregon-mempool/tests/dependencies.rs`

**Interfaces:**
- Produces complete ancestor-aware `Mempool::admit(...)`.
- Produces `Mempool::deterministic_order() -> Result<Vec<Hash256>, MempoolError>`.
- Internal graph helpers:

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

- [ ] **Step 1: RED parent-then-child and child-before-parent tests**.

```rust
#[test]
fn parent_then_child_is_valid_but_child_before_parent_is_not_retained() {
    let chain_point = outpoint(0x30, 0);
    let chain = state_with(vec![(chain_point, entry(100, 1, false))]);
    let parent = spend(vec![chain_point], &[90], 1);
    let child_point = OutPoint { txid: parent.txid(), index: 0 };
    let child = spend(vec![child_point], &[80], 2);
    let base = base(0x31, 20);

    let mut first = Mempool::new(base, MempoolConfig::default()).unwrap();
    assert_eq!(
        first.admit(child.clone(), base, &chain, &AcceptTestSpends),
        Err(MempoolError::MissingDependency(child_point))
    );
    assert!(first.is_empty());

    first.admit(parent.clone(), base, &chain, &AcceptTestSpends).unwrap();
    first.admit(child.clone(), base, &chain, &AcceptTestSpends).unwrap();
    assert_eq!(first.deterministic_order().unwrap(), vec![parent.txid(), child.txid()]);
}
```

- [ ] **Step 2: RED invalid parent output test** where parent exists but candidate references index `parent.outputs.len()`; expect `InvalidParentOutput` and no mutation.

- [ ] **Step 3: RED ancestor/descendant exact-boundary tests** using a linear chain and configs `max_ancestors = 2`, `max_descendants = 2`: second ancestor/descendant is allowed; third is rejected with exact typed error and unchanged pool.

- [ ] **Step 4: RED deterministic ordering test**: admit independent transactions in opposite orders into two pools; both `deterministic_order()` results must be identical ascending txid. Include a DAG where parent relations override txid order.

- [ ] **Step 5: Implement graph traversal with cycle detection**.

Use `BTreeSet<Hash256>` for ready sets and closures whose traversal order becomes visible. Kahn topology outline:

```rust
let mut indegree = BTreeMap::<Hash256, usize>::new();
let mut ready = BTreeSet::<Hash256>::new();
// populate indegrees from explicit parent sets
// always pop the smallest txid from ready
// if emitted.len() != entries.len(): DependencyCycle
```

- [ ] **Step 6: Upgrade admission dependency discovery**.

For each input:

1. reject spend-index conflict first;
2. if chain UTXO contains the outpoint, treat it as chain-backed;
3. otherwise, if `entries` contains `input.previous_txid`, verify the output index exists and add that txid to direct parents;
4. otherwise `MissingDependency`.

This chain-first rule makes revalidated confirmed-parent outputs naturally promotable later.

- [ ] **Step 7: Build narrow replay state**.

Collect complete ancestor closure. Seed only chain-backed outpoints used by ancestors/candidate. Replay ancestors in topology order through `apply_normal_transaction`; verify each replayed fee equals the stored `MempoolEntry::fee()`, otherwise return `InvariantViolation`. Then validate candidate and record its fee.

- [ ] **Step 8: Preflight descendant limits**.

Candidate ancestor count must be `<= max_ancestors`. For every ancestor, compute its existing unique descendant closure; adding candidate would increase it by exactly one because candidate is new and has no descendants. Require `existing_descendants + 1 <= max_descendants` with checked arithmetic.

- [ ] **Step 9: Publish graph edges atomically after all checks**. Candidate entry gets parent set; every parent entry gets child link. Preflight every parent existence before mutating any field.

- [ ] **Step 10: Full gate and commit** `feat: add Oregon mempool dependency graph`.

---

### Task 5: Bounded Memory and Deterministic Dependency-Safe Eviction

**Files:**
- Modify: `crates/oregon-mempool/src/eviction.rs`
- Modify: `crates/oregon-mempool/src/pool.rs`
- Modify: `crates/oregon-mempool/src/graph.rs`
- Create/Modify: `crates/oregon-mempool/tests/eviction.rs`

**Interfaces:**
- Internal exact comparator:

```rust
pub(crate) fn eviction_cmp(
    left: &MempoolEntry,
    right: &MempoolEntry,
) -> std::cmp::Ordering;
```

- Internal non-mutating planner returns a complete set/order of txids to remove before candidate publication.
- Internal removal commit removes an already-preflighted set without returning policy/validation errors midway.

- [ ] **Step 1: RED fee-rate comparison tests**.

Freeze exact rational comparison without floating point. For entries A/B compare:

```rust
let left = u128::from(a.fee()) * (b.encoded_bytes() as u128);
let right = u128::from(b.fee()) * (a.encoded_bytes() as u128);
```

Lowest ratio sorts first. Equal ratio -> lower fee first. Equal fee -> smaller txid bytes first.

- [ ] **Step 2: RED entry-count and byte-cap tests** using tiny custom configs. Verify admission remains accepted when exactly equal to a bound and triggers eviction only when `>` the bound.

- [ ] **Step 3: RED subtree eviction test**: low-fee parent with high-fee child must be removed together if parent is selected; no surviving child may reference a removed mempool parent.

- [ ] **Step 4: RED candidate-self-eviction rollback**. Start with a better existing transaction, submit a worse candidate into `max_entries = 1`; expect `CapacityRejected`, and assert exact pre-call pool state including bytes/graph/spend index through public observations.

- [ ] **Step 5: RED insertion-order determinism**. Equivalent pools constructed in different independent admission order must choose identical eviction txid(s).

- [ ] **Step 6: Implement virtual capacity planning without cloning transaction payloads**.

Planner inputs are immutable live entries plus the fully validated prepared candidate. Maintain a `BTreeSet<Hash256>` `planned_removed`. Virtual counts are:

```text
entries = self.entries.len() + 1 - planned_removed.len()
bytes = self.total_bytes + candidate_bytes - sum(bytes of planned_removed)
```

Use checked arithmetic before publication. Candidate participates in score selection virtually. If selected directly, return `CapacityRejected`. If an existing selected root has candidate in its ancestor-descendant relation, candidate would be in that subtree; also return `CapacityRejected` without mutating live state.

When an existing root is selected, add root plus its unique descendant closure to `planned_removed`; iterate until both hard bounds pass.

- [ ] **Step 7: Preflight removal consistency before first mutation**.

For every planned existing txid verify:
- entry exists;
- every input spend claim points back to this txid;
- every parent/child reciprocal edge is present when the counterpart survives/is in the planned set;
- byte subtraction is checked.

Any failure -> `InvariantViolation` before mutation.

- [ ] **Step 8: Commit removals then candidate insertion with no remaining fallible validation**. Remove spend claims and reciprocal edges, subtract bytes, remove entries, then insert candidate and its claims/edges. `AdmissionOutcome.evicted` is deterministic ascending-by-removal-plan order frozen by tests.

- [ ] **Step 9: Full gate and commit** `feat: bound Oregon mempool with deterministic eviction`.

---

### Task 6: Active-Block Reconciliation and Confirmed-Parent Promotion

**Files:**
- Modify: `crates/oregon-mempool/src/reconcile.rs`
- Modify: `crates/oregon-mempool/src/pool.rs`
- Modify: `crates/oregon-mempool/src/graph.rs`
- Create/Modify: `crates/oregon-mempool/tests/reconciliation.rs`

**Interfaces:**
- Produces `Mempool::reconcile_active_block(...)`.
- Internal staged rebuild helper:

```rust
fn rebuild_against_chain<V: SpendVerifier>(
    &self,
    source: &BTreeMap<Hash256, MempoolEntry>,
    new_base: ChainBase,
    chain_utxos: &UtxoState,
    verifier: &V,
) -> Result<Mempool, MempoolError>;
```

The rebuilt pool uses the same `MempoolConfig` and is published with `*self = rebuilt` only after success.

- [ ] **Step 1: RED confirmed-parent child-survival test**.

Construct parent->child in pool. Construct the new chain UTXO snapshot as if parent was mined: parent input absent, parent output present. Pass an active block containing the parent. After reconciliation parent is removed, child remains, child has empty mempool parent set, and base advances.

- [ ] **Step 2: RED active-block conflict test**.

Pool transaction A spends chain outpoint X and has child C. Active block contains different transaction B spending X. Reconciliation must remove A and C. `removed` must be sorted and deterministic.

- [ ] **Step 3: RED ordinary tip-change revalidation test** where an unrelated valid pool transaction survives and an entry invalidated by new chain state is filtered.

- [ ] **Step 4: RED reconciliation invariant atomicity test** using a `#[cfg(test)]` internal hook or crate-unit test that constructs a broken reciprocal graph, then calls staged rebuild and verifies the original pool/base are unchanged on `InvariantViolation`. Do not export a production corruption hook.

- [ ] **Step 5: Stage confirmed/conflict preprocessing without touching live pool**.

Create a source snapshot of transaction objects/metadata needed for rebuild. Determine confirmed txids from active block normal transactions. Determine active-block input conflicts from current spend index. Remove conflicting roots plus descendants from the staged source. Remove confirmed entries from staged source **without** automatically deleting children.

- [ ] **Step 6: Rebuild graph from actual input availability, not stale old edges**.

For each source transaction in deterministic old topology/txid traversal:
- if an input exists in `chain_utxos`, it is chain-backed even if its txid formerly named a mempool parent;
- otherwise dependency may target only a transaction successfully retained in the staged rebuild;
- if dependency is unavailable, filter transaction as invalid/missing rather than publishing an orphan;
- structural validation and `SpendVerifier` are run again at `new_base.tip_height + 1`.

This is the mechanism that promotes a child of a confirmed parent to chain-backed state.

- [ ] **Step 7: Reapply configured ancestor/descendant and capacity policy to rebuilt state** deterministically. Since source entries were previously valid, capacity should normally not increase, but the rebuild must not assume that invariant if configuration/accounting is corrupt.

- [ ] **Step 8: Publish rebuilt pool/base only after complete success**. Build `ReconcileReport` from old txid set minus rebuilt txid set, sorted ascending.

- [ ] **Step 9: Full gate and commit** `feat: reconcile Oregon mempool with active blocks`.

---

### Task 7: Reorg Full Revalidation, Stale Context, Determinism, and Recovery Matrix

**Files:**
- Modify: `crates/oregon-mempool/src/reconcile.rs`
- Modify: `crates/oregon-mempool/src/pool.rs`
- Modify: `crates/oregon-mempool/tests/reconciliation.rs`
- Modify: `crates/oregon-mempool/tests/admission.rs`
- Modify: `crates/oregon-mempool/tests/dependencies.rs`

**Interfaces:**
- Produces `Mempool::reconcile_reorg(...)`.
- No disconnected-block transaction list is accepted; M5 therefore cannot silently resurrect transactions.

- [ ] **Step 1: RED reorg retained-valid test**. Change base id/height and provide a compatible new UTXO snapshot; valid existing pool entries remain and base advances.

- [ ] **Step 2: RED disappeared-confirmed-parent test**. Begin with a child that survived because its parent had become chain-backed after prior active-block reconciliation. Reorg to a chain snapshot where that parent output no longer exists. Child must be removed as missing dependency; no parent transaction is synthesized.

- [ ] **Step 3: RED non-resurrection test**. A disconnected old-chain transaction not currently present in the pool must not appear after `reconcile_reorg` because the API receives no disconnected transaction bodies.

- [ ] **Step 4: RED stale-base admission test around reorg**. Before reconciliation, admission using new base returns `StaleChainContext`; after successful `reconcile_reorg`, same base is accepted for future admission.

- [ ] **Step 5: RED deterministic rebuild test**. Two logically identical pools produced from different independent insertion histories and reconciled against same chain snapshot must have identical `deterministic_order`, `removed`, byte total and entry metadata.

- [ ] **Step 6: Implement reorg as full staged rebuild only**.

```rust
pub fn reconcile_reorg<V: SpendVerifier>(
    &mut self,
    new_base: ChainBase,
    chain_utxos: &UtxoState,
    verifier: &V,
) -> Result<ReconcileReport, MempoolError> {
    let rebuilt = self.rebuild_against_chain(&self.entries, new_base, chain_utxos, verifier)?;
    let removed = sorted_difference(self.entries.keys(), rebuilt.entries.keys());
    let retained = rebuilt.len();
    *self = rebuilt;
    Ok(ReconcileReport { removed, retained })
}
```

The real helper may need internal ownership/borrowing adjustments, but semantics must remain: build first, single publication last.

- [ ] **Step 7: Add recovery/atomicity matrix** covering:
  - verifier rejection during rebuild filters expected-invalid tx without corrupting surviving state;
  - graph cycle/invariant error aborts rebuild and preserves old pool/base;
  - `tip_height == u64::MAX` returns `HeightOverflow` before publication;
  - zero-fee valid transactions remain admissible if capacity permits;
  - witness byte changes alter txid/size and are accounted independently;
  - no observable output changes with `HashMap` insertion order.

- [ ] **Step 8: Full gate and commit** `feat: revalidate Oregon mempool across reorgs`.

---

### Task 8: Security Mutations, Manual M4->M5 Review, and Accepted Checkpoint

**Files:**
- Mutation branches: production/test files only on throwaway branches.
- Create on clean M5 branch after evidence: `docs/checkpoints/OREGON_V1_M5_MEMPOOL.md`.
- Remove any temporary mutation-branch CI triggers before accepted checkpoint.

**Interfaces:**
- No new production behavior.
- Planned accepted recovery branch: `oregon-v1-checkpoint-m5-mempool-accepted-2026-09-04`.

- [ ] **Step 1: Fresh pre-mutation clean gate** on exact reviewed M5 code head.

Record commit SHA and GitHub Actions run/job IDs for:

```bash
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

- [ ] **Step 2: Mutation A — double-spend conflict bypass** on a fresh branch from clean M5 head.

Change admission so an existing outpoint spend claim is ignored/overwritten. Expected killed tests include the direct conflict test and state/index consistency tests. The mutant is accepted as killed only if CI fails for the intended semantic reason.

- [ ] **Step 3: Mutation B — orphan/missing-parent bypass** on a fresh branch from clean M5 head.

Change dependency admission so an input unavailable in both active chain UTXO and accepted mempool parents is retained/accepted. Expected killed tests: child-before-parent and missing-dependency tests.

- [ ] **Step 4: Mutation C — early/partial publication** on a fresh branch from clean M5 head.

Move one live publication action (entry insert, spend claim, graph edge, or byte increment) before verifier/capacity completion. Expected killed tests: rejection atomicity/state-equality and candidate-self-eviction rollback.

- [ ] **Step 5: Optional high-value boundary mutation if needed by review**: change ancestor or descendant `<=` boundary to `<`/`+1`. Exact-boundary tests must kill it. If existing Task 4 tests already demonstrate the off-by-one guard strongly, record that evidence without requiring this as one of the three mandatory mutations.

- [ ] **Step 6: Return to clean branch and run fresh post-mutation gate**. Verify none of the mutation commits/files are ancestors/diffs of the clean M5 head.

- [ ] **Step 7: Manual security review M4 accepted -> M5 clean head**.

Review at minimum:
- shared normal-transaction helper is the sole implementation used by mempool and mapped by block validation;
- block error precedence/regressions remain unchanged;
- no production permissive verifier;
- candidate, ancestor replay and full rebuild all invoke `SpendVerifier`;
- next-spend-height maturity boundary exactness;
- one-spender-per-outpoint and no RBF path;
- child-before-parent is not retained;
- parent output index bounds;
- ancestor/descendant unique closure and exact boundaries;
- no unchecked amount/byte/count arithmetic;
- fee-rate comparison uses integer cross multiplication only;
- deterministic txid tie rules and no `HashMap` iteration leakage;
- capacity planning is non-mutating and candidate-self-eviction rolls back completely;
- planned subtree removal cannot leave dangling parent/child edges or spend claims;
- confirmed-parent promotion keeps chain-backed children valid;
- active-chain conflicts remove conflicting descendants;
- full rebuild is staged and publishes base only at the end;
- reorg does not resurrect unknown/disconnected txs;
- M4 storage/chainstate code has no new dependency on mempool and durability semantics are untouched;
- no unsafe Rust in new crate;
- no unexpected new third-party production dependency beyond `thiserror` already used by workspace.

No known Critical or Important finding may remain open.

- [ ] **Step 8: Write checkpoint using observed evidence only**.

`docs/checkpoints/OREGON_V1_M5_MEMPOOL.md` must record:
- accepted M4 base SHA;
- final reviewed M5 code SHA;
- design and plan paths;
- exact test/fmt/clippy run IDs;
- each mutation branch/SHA/run and intended killed tests;
- post-mutation clean run;
- manual review disposition;
- explicit exclusions (P2P, orphan pool, RBF, package relay, persistence, wallet, mining RPC, production spend cryptography, testnet/mainnet).

No placeholder run IDs or future claims.

- [ ] **Step 9: Commit checkpoint and run one final CI gate on the checkpoint commit**.

- [ ] **Step 10: Create recovery branch** exactly from the final checkpoint commit:

`oregon-v1-checkpoint-m5-mempool-accepted-2026-09-04`

Verify branch SHA is identical to checkpoint commit and `main` remains untouched.

---

## Definition of M5 Accepted

M5 is accepted only when:

- shared normal-transaction validation is authoritative and block/mempool parity tests pass;
- `oregon-mempool` implements atomic admission, conflicts, parent-child dependencies, exact limits, deterministic topology/eviction, active-block reconciliation and reorg full rebuild;
- child-before-parent is rejected with no orphan retention;
- every normal input continues through mandatory `SpendVerifier`;
- capacity is bounded by both entry count and canonical transaction bytes;
- no observable ordering depends on unordered collection iteration;
- all M1-M4 regression suites remain green;
- required mutations A/B/C are each killed by intended tests;
- fresh post-mutation clean CI is green;
- manual M4->M5 review has no known Critical or Important finding open;
- checkpoint evidence contains exact SHAs/runs only;
- accepted M5 recovery branch points exactly to the final checkpoint commit;
- `main` has not been merged or modified.
