# Oregon Execution Resources V1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the inactive Stage 3A resource meter, parent-derived base fee and bounded block-weight arithmetic with independent vectors and security evidence.

**Architecture:** Consensus owns fee parameters, next-base-fee arithmetic and block limits. A new `oregon-execution` crate owns a fixed-size shared resource meter; it does not own VM-native semantics or persistence. Stage 3B fee escrow and authenticated backing are a separate next design.

**Tech Stack:** Rust 1.85.0, edition 2024, existing thiserror/proptest/serde/serde_json versions; Python 3 standard library for vectors and mutation orchestration.

**Spec:** `docs/superpowers/specs/2026-09-06-execution-resources-fees-v1.md`

## Global Constraints

- Inactive Stage 3A only; no M0–M6 behavior/encoding changes, reserve outputs, active VM, runtime, persistence, mempool or network integration.
- All new Rust crates declare `#![forbid(unsafe_code)]`; no lint suppression or unfinished production placeholders.
- Prices/weights/native counters use `u64`, products use `u128`, integer-only deterministic calculations.
- Version exactly 1; positive ratios; positive target with representable double; transaction limit in `1..=2*target`; denominator at least 2; `1 <= min <= max <= MAX_SUPPLY_BASE_UNITS`.
- No production activation defaults; JSON profiles are explicitly synthetic.
- Accepted base `7cfac99c40de96afb79d222cd5ade9f3a537215a`; branch `work/execution-resources-v1-2026-09-06`; stacked draft PR against `work/contract-state-v1-2026-09-05`; no main integration.

## Task 1: Checked fee arithmetic and one bounded resource meter

**Files:**
- Modify `Cargo.toml`, `Cargo.lock`, `crates/oregon-consensus/src/lib.rs`.
- Create `crates/oregon-consensus/src/execution_resources.rs`.
- Create `crates/oregon-consensus/tests/execution_resources.rs`.
- Create `crates/oregon-execution/Cargo.toml`, `crates/oregon-execution/src/lib.rs`, `crates/oregon-execution/src/weight.rs`, `crates/oregon-execution/tests/weight.rs`.

**Interfaces:**

```rust
// oregon_consensus::execution_resources
pub enum ResourceFeeError {
    UnsupportedVersion(u16), InvalidParameters, ParentFeeOutOfRange,
    ParentWeightExceeded, InvalidTransactionWeight, BlockWeightExceeded,
}
pub struct FeeParametersV1 { /* private validated fields */ }
impl FeeParametersV1 {
    pub fn new(version: u16, target_weight: u64, max_transaction_weight: u64,
        adjustment_denominator: u64, min_base_fee: u64, max_base_fee: u64)
        -> Result<Self, ResourceFeeError>;
    // same-named getters for five numeric arguments (excluding version)
    pub fn block_weight_limit(&self) -> u64;
}
pub fn next_base_fee(p: &FeeParametersV1, parent_base_fee: u64,
    parent_weight: u64) -> Result<u64, ResourceFeeError>;
pub struct BlockWeightBudget { /* private fields */ }
impl BlockWeightBudget {
    pub fn new(p: &FeeParametersV1) -> Self;
    pub fn include(&mut self, actual_weight: u64) -> Result<(), ResourceFeeError>;
    pub fn consumed(&self) -> u64;
    pub fn remaining(&self) -> u64;
}
// oregon_execution (re-export from lib.rs)
pub enum ResourceError { UnsupportedVersion(u16), InvalidRatio, InvalidBudget,
    WeightOverflow, WeightExhausted }
pub enum ResourceDomain { Native, Evm, Wasm }
pub struct WeightRatio { /* private fields, Copy */ }
impl WeightRatio {
    pub fn new(numerator: u64, denominator: u64) -> Result<Self, ResourceError>;
    pub fn normalized_weight(self, units: u64) -> Result<u64, ResourceError>;
}
pub struct MeterScheduleV1 { /* private fields, Copy */ }
impl MeterScheduleV1 {
    pub fn new(version: u16, native: WeightRatio, evm: WeightRatio,
        wasm: WeightRatio) -> Result<Self, ResourceError>;
}
pub struct WeightMeter { /* private fields, not Clone/Copy */ }
impl WeightMeter {
    pub fn new(schedule: MeterScheduleV1, max_weight: u64, intrinsic_weight: u64)
        -> Result<Self, ResourceError>;
    pub fn charge(&mut self, domain: ResourceDomain, units: u64) -> Result<(), ResourceError>;
    pub fn charge_common(&mut self, weight: u64) -> Result<(), ResourceError>;
    pub fn consumed(&self) -> u64;
    pub fn remaining(&self) -> u64;
    pub fn max_weight(&self) -> u64;
    pub fn is_exhausted(&self) -> bool;
}
```

Types derive Debug/Eq/PartialEq where relevant, error types implement Display/Error via thiserror. No serialization or unnecessary public methods. Integration tests are behavioral consumers, no source-text assertions.

- [ ] **Step 1: Add test consumers before production code.** Create the execution crate manifest and an empty forbid-unsafe lib to register the tests. The consensus test imports the not-yet-existing module. Write table-driven tests for every spec §7 boundary plus invalid configuration and atomic rejection. Representative assertions:

```rust
let p = FeeParametersV1::new(1, 100, 120, 8, 1, 1000).unwrap();
for (used, want) in [(0,88),(99,100),(100,100),(101,101),(200,112)] {
    assert_eq!(next_base_fee(&p, 100, used), Ok(want));
}
let r = WeightRatio::new(2, 3).unwrap();
assert_eq!(r.normalized_weight(1), Ok(1));
assert_eq!(r.normalized_weight(3), Ok(2));
let s = MeterScheduleV1::new(1, r, r, r).unwrap();
let mut m = WeightMeter::new(s, 4, 2).unwrap();
for _ in 0..3 { m.charge(ResourceDomain::Evm, 1).unwrap(); }
assert_eq!(m.consumed(), 4);
assert_eq!(m.charge_common(1), Err(ResourceError::WeightExhausted));
assert_eq!(m.charge_common(0), Err(ResourceError::WeightExhausted));
```

Use named tests `conversion_rounds_up`, `split_charges_share_cumulative_rounding`, `common_work_consumes_budget`, `exhaustion_is_sticky`, `exact_budget_succeeds_then_overrun_exhausts`, `empty_parent_lowers_fee`, `tiny_upward_change_costs_one_unit`, `fee_never_exceeds_ceiling`, `invalid_parent_utilization_rejected`, `block_budget_rejects_transaction_and_block_overruns_atomically` as mutation anchors. Separate tests cover malformed versions/parameters, wide products, mixed domains, zero charges, native counter overflow, conversion overflow and producer sequences. Proptest compares split/combined schedules, domain reordering, fee direction/rate/range.

- [ ] **Step 2: Observe the missing-API red.** Run `cargo +1.85.0 test --locked -p oregon-consensus --test execution_resources` and `cargo +1.85.0 test --locked -p oregon-execution --test weight`; update Cargo.lock only for the new package if --locked requires it. Save commands/output and test-first commit. Missing-API compilation is acceptable initial evidence; mutation acceptance later requires assertion failures.
- [ ] **Step 3: Implement spec §§4–6.** One cumulative `[u64;3]` domain counter array, one private budget acceptance helper and one exhaustion helper. Pure conversion uses `product / denominator + u128::from(product % denominator != 0)` and checked narrowing. The meter converts old/new cumulative counts, charges their difference, and makes count/converted-value overflow terminal exhaustion. Zero work cannot revive an exhausted meter. Ratio arithmetic is owned only once. Fee arithmetic uses u128 both above and below target with explicit clipping; configuration validation protects subtraction/division. Block budget validates before assigning its total.
- [ ] **Step 4: Run focused tests, format and Clippy.** Commands: `cargo +1.85.0 test --locked -p oregon-consensus --test execution_resources`; `cargo +1.85.0 test --locked -p oregon-execution --all-targets`; `cargo +1.85.0 fmt --all -- --check`; `cargo +1.85.0 clippy --locked -p oregon-consensus -p oregon-execution --all-targets -- -D warnings`. Fix faults with covering tests. Record exact results.
- [ ] **Step 5: Commit only this task's files and report.** A task reviewer checks spec compliance and quality before Task 2 acceptance. No agent changes the spec's economic choices or touches the frozen active parameter object.

## Task 2: Independent evidence, CI boundaries and durable continuation

**Files:**
- Create `scripts/generate_execution_resource_vectors.py`, `tests/vectors/execution-resources-v1.json`.
- Create `crates/oregon-consensus/tests/execution_resource_vectors.rs`, `crates/oregon-execution/tests/resource_vectors.rs`.
- Create `scripts/verify_execution_resource_mutations.py`.
- Modify `.github/workflows/oregon-rust.yml`; create `.github/workflows/oregon-execution-resources.yml`.
- Update `HANDOFF.md`, this plan; create `docs/checkpoints/OREGON_EXECUTION_RESOURCES_PROGRESS.md`.

**Interfaces:** consumes Task 1 public APIs above and named assertion tests. Produces reproducible synthetic JSON, compiled killed-mutant evidence, exact-head CI links and an unambiguous next Stage 3B action.

- [ ] **Step 1: Build independent literal JSON using Python integer arithmetic.** Conversion oracle uses `divmod` of arbitrary precision products and checks u64 range. Fee oracle computes the rational fraction with `fractions.Fraction`, truncates toward zero for the nonnegative magnitude, and applies §6. Include cases listed in spec §7, maximal products, full/empty/alternating producer sequences and an eventual ceiling. Commit literal values and the generating script; it must have a `--check` mode that compares regenerated content without rewriting the file.

```python
q, remainder = divmod(units * numerator, denominator)
weight = q + bool(remainder)
change = int(Fraction(base * abs(used - target), target * denominator))
next_fee = min(ceiling, base + max(1, change)) if used > target else max(floor, base - change)
```

The target-equality case keeps the fee unchanged. Python consumers never call Rust for expected outputs. Rust tests deserialize explicit fields with serde and exercise the public APIs. They verify nonempty vector sets and all sequence elements, not merely metadata.

- [ ] **Step 2: Implement and execute the thirteen mutation targets in spec §7.** Run in a disposable clean checkout. Each mutation requires exactly one source-site match, executes its named focused test, expects exit 101 plus the named `... FAILED` result, rejects compilation errors, restores source in finally and checks a clean final diff. Baseline runs must pass before and after the mutation loop. Do not treat a surviving mutant as a reason to weaken assertions.
- [ ] **Step 3: Add CI gates without rewriting inherited scans.** Add this branch to Rust CI, required design presence, no upward dependencies from execution and no execution dependency in consensus. Run new focused vectors/tests and the mutation script. New architecture workflow uses the same pinned checkout action, Rust 1.85 and x86/ARM runner style as existing workflows, installs required native compiler dependencies for consensus, runs only focused resource tests/vectors and the generator check. Trigger this branch and matching PRs; no RandomX workflow edits.
- [ ] **Step 4: Run final verification and independent whole-branch review.** Required commands: full workspace all-target test, fmt, Clippy warnings denied, workspace docs, architecture scan, new reference-vector check and mutations plus inherited CI gates. Capture exact SHA and workflows, not a stale green ancestor. New architecture-vector workflow must finish on x86 and ARM.
- [ ] **Step 5: Persist the checkpoint and handoff, push via connector and update the stacked draft PR.** Record delivered 3A versus unimplemented 3B, independent review, test-first/mutation evidence and exact next action. Inspect the final descendant's own CI. Keep `main` and PR #14 unchanged. Do not claim escrow, authenticated reserve transitions or production activation are implemented.
