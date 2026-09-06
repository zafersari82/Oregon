# Stage 3B Fee Settlement and Reserve Conservation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the inactive, deterministic Stage 3B fee escrow/settlement and 1:1 native reserve foundation without changing accepted M0-M6 or currently active Stage 1/2/3A behavior.

**Architecture:** Canonical bytes and hashes live in `oregon-primitives`; fee lifecycle and capability/escrow semantics live in `oregon-execution`; logical accounting/FeeState key codecs live in `oregon-contract-state`; reserve-pool transitions live in `oregon-utxo`; execution-fee-aware producer validation is a separate inactive helper in `oregon-consensus`. Cross-crate behavior is locked by independent Python vectors and named mutation gates before any later chainstate/persistence activation.

**Tech Stack:** Rust 1.85.0, existing Oregon crates, `thiserror`, `proptest`, Python 3 vector generator, GitHub Actions x86_64/ARM.

**Spec:** `docs/superpowers/specs/2026-09-06-fee-settlement-reserve-v1.md`

## Global Constraints

- Base from verified main `a07eb04be910ff1312a0c2a08aa66affa4fd75b5`; never resume old Stage 1/2/3A work branches.
- Current `Transaction`, block header, txid, UTXO validation, coinbase validation, RocksDB schema, mempool, networking and node runtime behavior remain unchanged.
- No execution-fee burn; execution balances remain exactly 1:1 backed by the protocol reserve pool.
- All fee products use `u128`; every narrowing conversion is checked.
- Deterministic contract revert/resource exhaustion charges consumed work; pre-escrow invalid transactions charge zero.
- The Stage 3B implementation path remains inactive and has no RPC/VM/network/chainstate activation caller.
- `oregon-consensus` must not depend on `oregon-execution`; `oregon-execution` must not depend on storage/chainstate/RPC/network/VM crates.
- Reserve locking program is exactly `4f5245474f4e2f455845432f524553455256452f563100`.
- No TODO/FIXME/HACK, lint suppression, force-push, or accepted-checkpoint ref movement.

---

### Task 1: Canonical fee receipt and reserve identifiers

**Files:**
- Create: `crates/oregon-primitives/src/fee_settlement.rs`
- Create: `crates/oregon-primitives/src/execution_reserve.rs`
- Modify: `crates/oregon-primitives/src/lib.rs`
- Test: `crates/oregon-primitives/tests/fee_settlement.rs`
- Test: `crates/oregon-primitives/tests/execution_reserve.rs`

**Interfaces:**
- Produces: `FeeSourceKind`, `ExecutionOutcome`, `FeeSettlementReceiptV1`, `FeeSettlementReceiptV1Parts`, `EXECUTION_RESERVE_LOCKING_PROGRAM_V1`, `reserve_transition_id(&[u8]) -> Hash256`, `reserve_outpoint_txid(Hash256) -> Hash256`.
- Consumes: existing `ExecutionAddress`, `Hash256`, `Decoder`, `PrimitiveError`, `domain_hash`.

- [ ] **Step 1: Write failing canonical receipt tests**

```rust
#[test]
fn fee_receipt_round_trips_and_hashes_exactly() {
    let receipt = FeeSettlementReceiptV1::new(FeeSettlementReceiptV1Parts {
        txid: Hash256::from_bytes([0x11; 32]),
        escrow_id: Hash256::from_bytes([0x22; 32]),
        payer: ExecutionAddress::from_bytes([0x03; 33]).unwrap(),
        source_kind: FeeSourceKind::ExecutionBalance,
        outcome: ExecutionOutcome::Reverted,
        base_fee_per_weight: 10,
        max_fee_per_weight: 20,
        max_priority_fee_per_weight: 5,
        max_weight: 100,
        actual_weight: 40,
        effective_price: 15,
        base_component: 400,
        priority_component: 200,
        charged: 600,
        refund: 1_400,
    }).unwrap();
    let bytes = receipt.encode();
    assert_eq!(FeeSettlementReceiptV1::decode(&bytes).unwrap(), receipt);
    assert_eq!(receipt.receipt_id(), domain_hash(b"OREGON/FEE/RECEIPT/V1\0", &bytes));
}
```

Also add named failures for unknown source kind/outcome, wrong fixed length, inconsistent arithmetic and noncanonical decode.

- [ ] **Step 2: Run focused tests and verify expected red**

Run:

```bash
cargo +1.85.0 test --locked -p oregon-primitives --test fee_settlement --test execution_reserve
```

Expected: compile failure because the new modules/types do not exist.

- [ ] **Step 3: Implement minimal canonical primitives**

Use exact discriminants:

```rust
#[repr(u8)]
pub enum FeeSourceKind { NativeUtxo = 0x01, ExecutionBalance = 0x02 }

#[repr(u8)]
pub enum ExecutionOutcome { Committed = 0x00, Reverted = 0x01, ResourceExhausted = 0x02 }

pub const EXECUTION_RESERVE_LOCKING_PROGRAM_V1: &[u8; 23] = b"OREGON/EXEC/RESERVE/V1\0";
```

`FeeSettlementReceiptV1::new` validates `actual_weight in 1..=max_weight`, `max_fee >= base_fee`, `max_priority <= max_fee`, exact effective price/components/charged/refund using checked `u128`, and rejects any duplicate field set that does not recompute.

- [ ] **Step 4: Run focused tests green**

Run the Task 1 command; expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/oregon-primitives/src crates/oregon-primitives/tests
git commit -m "feat: add canonical Stage 3B fee receipts"
```

---

### Task 2: Fee capability, escrow state machine and settlement arithmetic

**Files:**
- Create: `crates/oregon-execution/src/fees.rs`
- Modify: `crates/oregon-execution/src/lib.rs`
- Modify: `crates/oregon-execution/Cargo.toml`
- Test: `crates/oregon-execution/tests/fees.rs`

**Interfaces:**
- Consumes: `oregon_primitives::{ExecutionAddress, ExecutionOutcome, FeeSettlementReceiptV1, FeeSourceKind, Hash256, MAX_SUPPLY_BASE_UNITS, domain_hash}`.
- Produces: `FundingCapabilityV1`, `FeeTermsV1`, `EscrowTicketV1`, `EscrowBookV1`, `SettlementResultV1`, `FeeError`.

- [ ] **Step 1: Add expected-red fee lifecycle tests**

Tests must name and distinguish:

```rust
assert_eq!(FeeTermsV1::new(9, 10, 1, 100), Err(FeeError::MaxFeeBelowBaseFee));
assert_eq!(book.open(capability.clone(), terms)?, expected_ticket);
assert_eq!(book.open(capability, terms), Err(FeeError::CapabilityAlreadyConsumed));
assert_eq!(book.settle(ticket.escrow_id(), ExecutionOutcome::Reverted, 40)?.charged(), 600);
assert_eq!(book.settle(ticket.escrow_id(), ExecutionOutcome::Reverted, 40), Err(FeeError::EscrowAlreadySettled));
```

Add overflow, exact max-weight, one-over, refund, stale sequence, payer mismatch and resource-exhaustion tests.

- [ ] **Step 2: Run expected red**

```bash
cargo +1.85.0 test --locked -p oregon-execution --test fees
```

Expected: compile failure for missing fee module/types.

- [ ] **Step 3: Add downward primitives dependency**

`crates/oregon-execution/Cargo.toml`:

```toml
oregon-primitives = { path = "../oregon-primitives" }
```

Do not add consensus/storage/chainstate/VM dependencies.

- [ ] **Step 4: Implement capability and ticket IDs**

`FundingCapabilityV1::capability_id()` serializes exactly the spec fields and hashes with `OREGON/FEE/CAPABILITY/V1\0`. `EscrowTicketV1::escrow_id()` uses `OREGON/FEE/ESCROW/V1\0`.

- [ ] **Step 5: Implement `EscrowBookV1` minimally**

Use bounded maps/sets keyed by `Hash256`. `open` validates freshness, payer/source identity, supply envelope and max escrow. `settle` accepts only an open ticket, computes exact arithmetic, creates a canonical receipt, marks the ticket settled and returns source refund/net charge.

- [ ] **Step 6: Run focused execution tests**

```bash
cargo +1.85.0 test --locked -p oregon-execution --all-targets
cargo +1.85.0 clippy --locked -p oregon-execution --all-targets -- -D warnings
```

Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add crates/oregon-execution Cargo.lock
git commit -m "feat: add deterministic Stage 3B fee escrow"
```

---

### Task 3: ExecutionAccounting and FeeState canonical SMT keys

**Files:**
- Create: `crates/oregon-contract-state/src/execution_accounting.rs`
- Create: `crates/oregon-contract-state/src/fee_state.rs`
- Modify: `crates/oregon-contract-state/src/lib.rs`
- Test: `crates/oregon-contract-state/tests/execution_accounting.rs`
- Test: `crates/oregon-contract-state/tests/fee_state.rs`

**Interfaces:**
- Produces: `balance_key(&ExecutionAddress) -> Vec<u8>`, `sequence_key(&ExecutionAddress) -> Vec<u8>`, `TOTAL_EXECUTION_BALANCE_KEY`, `RESERVE_OUTPOINT_KEY`, `encode_accounting_u64`, `encode_reserve_outpoint`, FeeState key constants and `FeeStateValuesV1::write_set()`.
- Consumes: existing `StateWrite`, `StateWriteSet`, `CommitmentDomainId`, `OutPoint`.

- [ ] **Step 1: Write exact-key and exact-value tests**

```rust
assert_eq!(balance_key(&address), [b"acct/v1/balance/".as_slice(), address.as_bytes()].concat());
assert_eq!(encode_accounting_u64(42), 42u64.to_le_bytes());
assert_eq!(encode_reserve_outpoint(None), vec![0x00]);
```

For a present reserve outpoint assert exact `0x01 || txid || index_le`. FeeState tests assert the six exact raw keys and fixed 8/32-byte values.

- [ ] **Step 2: Verify expected red**

```bash
cargo +1.85.0 test --locked -p oregon-contract-state --test execution_accounting --test fee_state
```

Expected: compile failure for missing modules.

- [ ] **Step 3: Implement key/value codecs and write-set builders**

`FeeStateValuesV1::write_set()` must produce `StateWriteSet::new(CommitmentDomainId::FeeState, writes)` and never write the values under EVM/WASM/ExecutionAccounting domains.

Accounting helpers use `CommitmentDomainId::ExecutionAccounting`; zero balances are represented by `StateWrite::delete` in the higher-level builder, not `0u64` leaf values.

- [ ] **Step 4: Add SMT-root characterization tests**

Use a small in-test `StateSource` and existing `apply_write_set` to prove identical writes yield identical roots and a key/domain substitution changes the root.

- [ ] **Step 5: Run focused state tests green**

```bash
cargo +1.85.0 test --locked -p oregon-contract-state --all-targets
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/oregon-contract-state
git commit -m "feat: add Stage 3B accounting state keys"
```

---

### Task 4: Inactive singleton native reserve-pool transition

**Files:**
- Create: `crates/oregon-utxo/src/reserve.rs`
- Modify: `crates/oregon-utxo/src/lib.rs`
- Test: `crates/oregon-utxo/tests/reserve.rs` if integration-test visibility is appropriate; otherwise keep focused `#[cfg(test)]` tests in `reserve.rs`.

**Interfaces:**
- Produces: `ReservePoolSnapshotV1`, `ReserveTransitionV1Parts`, `ReserveTransitionV1`, `ReserveTransitionError`, `ReserveUndoV1`.
- Consumes: primitive reserve constant/hash helpers, `Amount`, `Hash256`, `OutPoint`, `TxOutput`.

- [ ] **Step 1: Write expected-red reserve arithmetic/id tests**

Cover zero->deposit, increase, fee decrease, withdrawal, zero removal, stale previous outpoint, wrong program, underflow, overflow and exact transition/outpoint hashes.

```rust
let transition = ReserveTransitionV1::new(parts_with(0, 1_000, 0, 0, 1_000)).unwrap();
assert_eq!(transition.new_reserve_amount(), 1_000);
assert_eq!(transition.new_reserve_outpoint().unwrap().index, 0);
```

Add a regression test proving `transition_id` is unchanged when only post-state root fixture values change; roots are not ID inputs.

- [ ] **Step 2: Run expected red**

```bash
cargo +1.85.0 test --locked -p oregon-utxo reserve
```

Expected: missing reserve module/types.

- [ ] **Step 3: Implement pure transition construction first**

Build canonical transition-id bytes exactly in spec order; validate previous flag/amount, checked reserve equation and exact equality to new execution total. Derive new outpoint only after the transition id.

- [ ] **Step 4: Add crate-private reserve apply/undo against `UtxoState`**

Do not change `apply_normal_transaction` or `connect_block`. The new inactive API validates exact old reserve entry, removes it, inserts at most one exact new reserve entry, rejects collision/multiple-live reserve state, and returns exact undo.

- [ ] **Step 5: Prove forward/undo/forward equivalence**

Snapshot state, apply transition, undo, assert exact original state, reapply and assert exact first forward state.

- [ ] **Step 6: Run UTXO suite green**

```bash
cargo +1.85.0 test --locked -p oregon-utxo --all-targets
```

Expected: current tests plus reserve tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/oregon-utxo
git commit -m "feat: add inactive execution reserve pool"
```

---

### Task 5: Inactive execution-fee-aware producer claim helper

**Files:**
- Modify: `crates/oregon-consensus/src/coinbase.rs`
- Modify: `crates/oregon-consensus/src/lib.rs`
- Test: `crates/oregon-consensus/tests/execution_fee_coinbase.rs`

**Interfaces:**
- Produces: `validate_coinbase_with_execution_fees_v1(tx, height, native_fees, execution_fees, params)`.
- Consumes: current `validate_coinbase`, `block_subsidy`, primitive reserve locking constant.

- [ ] **Step 1: Write distinguishing expected-red tests**

Tests must prove current `validate_coinbase` behavior is unchanged while the new helper rejects:

- miner claim below `native_fee_total + execution_fee_total`;
- missing final execution-fee output;
- wrong final execution-fee amount;
- reserve locking program as producer payout;
- total claim above subsidy + all fees.

Also prove `execution_fee_total == 0` needs no dedicated final execution-fee output.

- [ ] **Step 2: Run expected red**

```bash
cargo +1.85.0 test --locked -p oregon-consensus --test execution_fee_coinbase
```

Expected: missing helper.

- [ ] **Step 3: Implement the separate helper**

Pseudo-shape:

```rust
pub fn validate_coinbase_with_execution_fees_v1(
    tx: &Transaction,
    height: u64,
    native_fees: Amount,
    execution_fees: Amount,
    params: &ConsensusParams,
) -> Result<(), ConsensusError> {
    let total = checked_amount(native_fees, execution_fees)?;
    validate_coinbase(tx, height, total, params)?;
    let miner_outputs = miner_outputs(tx, height)?;
    require_claim_at_least_fees(miner_outputs, total)?;
    require_final_execution_fee_output(miner_outputs, execution_fees)?;
    Ok(())
}
```

Do not wire this helper into `validate_non_genesis_block_structure` or `UtxoState::connect_block`.

- [ ] **Step 4: Run current + new consensus tests**

```bash
cargo +1.85.0 test --locked -p oregon-consensus --all-targets
```

Expected: pass, including unchanged current underclaim characterization.

- [ ] **Step 5: Commit**

```bash
git add crates/oregon-consensus
git commit -m "feat: add inactive execution fee producer rules"
```

---

### Task 6: Independent Stage 3B vectors and cross-crate consumers

**Files:**
- Create: `scripts/generate_fee_settlement_vectors.py`
- Create: `tests/vectors/fee-settlement-v1.json`
- Create: `crates/oregon-primitives/tests/fee_settlement_vectors.rs`
- Create: `crates/oregon-execution/tests/fee_vectors.rs`
- Create: `crates/oregon-utxo/tests/reserve_vectors.rs` if public test interface exists after Task 4; otherwise consume vectors in crate-local reserve tests.
- Create: `crates/oregon-consensus/tests/execution_fee_vectors.rs`
- Create: `.github/workflows/oregon-fee-settlement.yml`

**Interfaces:**
- Python generator is independent and must not invoke Rust.
- JSON contains literal expected bytes/hashes/arithmetic/reserve transitions used by Rust consumers.

- [ ] **Step 1: Write generator with `--check` mode**

The generator uses Python integers for wide arithmetic and `hashlib.sha256` only according to Oregon's already established domain-hash construction. Include vectors for arithmetic, receipts, transition IDs/outpoints, reserve deltas and producer-claim boundaries.

- [ ] **Step 2: Generate committed literal vectors**

```bash
python3 scripts/generate_fee_settlement_vectors.py
python3 scripts/generate_fee_settlement_vectors.py --check
```

Expected: second command exits 0 and produces no diff.

- [ ] **Step 3: Add Rust consumers and run focused vector tests**

```bash
cargo +1.85.0 test --locked -p oregon-primitives --test fee_settlement_vectors
cargo +1.85.0 test --locked -p oregon-execution --test fee_vectors
cargo +1.85.0 test --locked -p oregon-consensus --test execution_fee_vectors
```

Add the UTXO vector consumer to the command if Task 4 exposes a stable test surface.

- [ ] **Step 4: Add x86_64/ARM workflow**

Workflow runs on `ubuntu-24.04` and `ubuntu-24.04-arm`, pins checkout to the repository's current SHA-pinned action, installs Rust 1.85.0/native prerequisites, runs Python `--check` and the focused Rust consumers.

- [ ] **Step 5: Commit**

```bash
git add scripts tests/vectors crates .github/workflows/oregon-fee-settlement.yml
git commit -m "test: add independent Stage 3B fee vectors"
```

---

### Task 7: Stage 3B named security mutation gate

**Files:**
- Create: `scripts/verify_fee_settlement_mutations.py`
- Modify: `.github/workflows/oregon-rust.yml`
- Test: mutation script itself restores source exactly after every mutant.

**Interfaces:**
- Produces one mutation gate with named target -> named expected failing test mapping.
- Must restore source even after compile/test failure and run the clean focused suites afterward.

- [ ] **Step 1: Define named mutants**

At minimum:

```text
max_fee_below_base_fee_bypass
fee_product_unchecked
refund_off_by_one
revert_fee_zeroed
duplicate_settlement_allowed
stale_capability_allowed
reserve_program_bypass
reserve_underflow_allowed
reserve_backing_equality_bypass
fee_accumulator_double_count
producer_payout_omitted
fee_state_domain_substitution
transition_root_cycle_regression
reserve_undo_mismatch
```

- [ ] **Step 2: Implement target-specific kill verification**

For every target, mutate one exact source fragment, run only the intended named test, require that test to fail for the intended semantic reason, restore source, and verify clean hash/content before the next target. Compile failure alone is not a kill.

- [ ] **Step 3: Run mutation gate locally where toolchain permits**

```bash
python3 scripts/verify_fee_settlement_mutations.py
```

Expected: all targets killed and clean source restored. If native RandomX linking is unavailable locally, CI is the authoritative full linked run; Python syntax and source-restoration checks still run locally.

- [ ] **Step 4: Wire gate into Oregon Rust CI**

Add architecture checks for the Stage 3B spec and dependency direction, focused Stage 3B tests, then the mutation gate before docs/fmt/clippy.

- [ ] **Step 5: Commit**

```bash
git add scripts/verify_fee_settlement_mutations.py .github/workflows/oregon-rust.yml
git commit -m "ci: enforce Stage 3B fee settlement mutations"
```

---

### Task 8: Full verification, checkpoint and continuation record

**Files:**
- Create: `docs/checkpoints/OREGON_FEE_SETTLEMENT_RESERVE_PROGRESS.md`
- Modify: `HANDOFF.md`

**Interfaces:**
- Checkpoint records exact branch/head/tree, vector/mutation counts, CI run/job IDs, limitations and next action.
- HANDOFF points only to the verified current branch/checkpoint; it does not claim main integration.

- [ ] **Step 1: Run full local verification**

```bash
python3 scripts/generate_fee_settlement_vectors.py --check
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 rustdoc --locked -p oregon-chainstate -- -D warnings
cargo +1.85.0 doc --locked --workspace --no-deps
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
python3 scripts/verify_fee_settlement_mutations.py
git diff --check
```

Expected: all available local commands pass; any environment-only native limitation is recorded rather than disguised.

- [ ] **Step 2: Push/open a draft PR against current main and require CI**

The exact PR head must pass Oregon Rust CI and the Stage 3B x86_64/ARM vector workflow. RandomX special workflows remain required only if RandomX-relevant files changed.

- [ ] **Step 3: Record exact evidence in checkpoint**

Do not use a green ancestor for a changed tree. Record exact source SHA/tree and exact successful run/job IDs.

- [ ] **Step 4: Update HANDOFF**

Next action after accepted Stage 3B is the separately versioned runtime/journal/async execution core. State explicitly that VM execution, chainstate persistence integration and protocol activation remain future work.

- [ ] **Step 5: Commit checkpoint/handoff**

```bash
git add docs/checkpoints/OREGON_FEE_SETTLEMENT_RESERVE_PROGRESS.md HANDOFF.md
git commit -m "docs: checkpoint Stage 3B fee settlement foundation"
```

- [ ] **Step 6: Stop before main integration**

Keep the verified implementation branch/PR intact. `main` integration requires the separate explicit owner decision mandated by `AGENTS.md` and the Engineering Constitution.
