# Stage 3B Fee Settlement and Reserve Conservation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the inactive Stage 3B fee escrow/settlement and 1:1 native reserve foundation without changing current Oregon runtime behavior.

**Architecture:** `oregon-primitives` owns canonical bytes/hashes, `oregon-execution` owns fee capability/escrow/settlement, `oregon-contract-state` owns canonical logical accounting/FeeState keys, `oregon-utxo` owns the singleton reserve-pool transition, and `oregon-consensus` exposes only a separate inactive producer-fee validator. Independent Python vectors and named mutation targets gate the whole feature.

**Tech Stack:** Rust 1.85.0, existing Oregon workspace crates, `thiserror`, `proptest`, Python 3, GitHub Actions x86_64/ARM.

**Spec:** `docs/superpowers/specs/2026-09-06-fee-settlement-reserve-v1.md`

## Global Constraints

- Implementation branch starts from the verified design/plan descendant of main `a07eb04be910ff1312a0c2a08aa66affa4fd75b5`.
- Current `Transaction`, block/header encoding, txid, active UTXO/coinbase validation, RocksDB schema, mempool, networking and node behavior remain unchanged.
- All fee products use `u128`; every narrowing conversion is checked.
- `max_fee_per_weight < base_fee_per_weight` is invalid before escrow.
- Deterministic revert/resource exhaustion charges consumed work; pre-escrow invalidity charges zero.
- Execution balances equal the single protocol reserve UTXO value exactly at accepted execution boundaries.
- Reserve locking program is exactly `4f5245474f4e2f455845432f524553455256452f563100`.
- `oregon-consensus` never depends on `oregon-execution`; `oregon-execution` never depends on storage/chainstate/RPC/network/VM crates.
- No TODO/FIXME/HACK, lint suppression, force-push, or accepted-checkpoint ref movement.

---

### Task 1: Canonical fee receipt and reserve hash primitives

**Files:**
- Create: `crates/oregon-primitives/src/fee_settlement.rs`
- Create: `crates/oregon-primitives/src/execution_reserve.rs`
- Modify: `crates/oregon-primitives/src/lib.rs`
- Test: `crates/oregon-primitives/tests/fee_settlement.rs`
- Test: `crates/oregon-primitives/tests/execution_reserve.rs`

**Interfaces:**
- Produces `FeeSourceKind`, `ExecutionOutcome`, `FeeSettlementReceiptV1Parts`, `FeeSettlementReceiptV1`, `EXECUTION_RESERVE_LOCKING_PROGRAM_V1`, `reserve_transition_id`, `reserve_outpoint_txid`.
- Uses existing `execution_address::{ExecutionAddress, ExecutionAddressKind}`, `Hash256`, `domain_hash`.

- [ ] **Step 1: Write failing receipt tests using existing address API**

```rust
use oregon_primitives::execution_address::{ExecutionAddress, ExecutionAddressKind};

fn payer() -> ExecutionAddress {
    ExecutionAddress::new(ExecutionAddressKind::Oregon, [0x33; 32]).unwrap()
}

#[test]
fn canonical_receipt_round_trips() {
    let receipt = FeeSettlementReceiptV1::new(FeeSettlementReceiptV1Parts {
        txid: Hash256::from_bytes([0x11; 32]),
        escrow_id: Hash256::from_bytes([0x22; 32]),
        payer: payer(),
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

Add named tests for unknown source/outcome discriminants, wrong fixed length, `actual_weight == 0`, `actual_weight > max_weight`, wrong effective price/components/charge/refund and max fee below base fee.

- [ ] **Step 2: Run expected red**

```bash
cargo +1.85.0 test --locked -p oregon-primitives --test fee_settlement --test execution_reserve
```

Expected: E0432/E0433 because new modules/types do not exist.

- [ ] **Step 3: Implement exact enums and reserve helpers**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FeeSourceKind { NativeUtxo = 0x01, ExecutionBalance = 0x02 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExecutionOutcome { Committed = 0x00, Reverted = 0x01, ResourceExhausted = 0x02 }

pub const EXECUTION_RESERVE_LOCKING_PROGRAM_V1: &[u8; 23] = b"OREGON/EXEC/RESERVE/V1\0";

pub fn reserve_transition_id(bytes: &[u8]) -> Hash256 {
    domain_hash(b"OREGON/RESERVE/TRANSITION/V1\0", bytes)
}

pub fn reserve_outpoint_txid(transition_id: Hash256) -> Hash256 {
    domain_hash(b"OREGON/RESERVE/OUTPOINT/V1\0", transition_id.as_bytes())
}
```

Implement fixed-width receipt encode/decode and recompute every duplicated arithmetic field before construction succeeds.

- [ ] **Step 4: Run Task 1 green**

```bash
cargo +1.85.0 test --locked -p oregon-primitives --test fee_settlement --test execution_reserve
cargo +1.85.0 fmt --all -- --check
```

- [ ] **Step 5: Commit**

```bash
git add crates/oregon-primitives
git commit -m "feat: add canonical Stage 3B fee primitives"
```

---

### Task 2: Funding capabilities and one-shot escrow settlement

**Files:**
- Create: `crates/oregon-execution/src/fees.rs`
- Modify: `crates/oregon-execution/src/lib.rs`
- Modify: `crates/oregon-execution/Cargo.toml`
- Test: `crates/oregon-execution/tests/fees.rs`

**Interfaces:**
- Produces `FundingCapabilityV1`, `FeeTermsV1`, `EscrowTicketV1`, `SettlementResultV1`, `EscrowBookV1`, `FeeError`.
- Adds only `oregon-primitives = { path = "../oregon-primitives" }` as a new production dependency.

- [ ] **Step 1: Write expected-red fee arithmetic/state-machine tests**

```rust
#[test]
fn max_fee_below_base_fee_is_rejected() {
    assert_eq!(FeeTermsV1::new(10, 9, 1, 100), Err(FeeError::MaxFeeBelowBaseFee));
}

#[test]
fn reverted_execution_still_charges_consumed_weight_once() {
    let mut book = EscrowBookV1::new(8);
    let capability = execution_capability(2_000, 7);
    let ticket = book.open(Hash256::from_bytes([0x44; 32]), capability, FeeTermsV1::new(10, 20, 5, 100).unwrap()).unwrap();
    let settled = book.settle(ticket.escrow_id(), ExecutionOutcome::Reverted, 40).unwrap();
    assert_eq!(settled.charged(), 600);
    assert_eq!(settled.refund(), 1_400);
    assert_eq!(book.settle(ticket.escrow_id(), ExecutionOutcome::Reverted, 40), Err(FeeError::EscrowAlreadySettled));
}
```

Define the test helper in the same file:

```rust
fn execution_capability(available: u64, sequence: u64) -> FundingCapabilityV1 {
    FundingCapabilityV1::new(
        FeeSourceKind::ExecutionBalance,
        ExecutionAddress::new(ExecutionAddressKind::Oregon, [0x55; 32]).unwrap(),
        Hash256::from_bytes([0x66; 32]),
        available,
        sequence,
        Hash256::from_bytes([0x77; 32]),
    ).unwrap()
}
```

Add tests for `u128` overflow/narrowing, max-weight exact/one-over, insufficient capability, duplicate capability, unknown escrow, stale sequence and resource exhaustion.

- [ ] **Step 2: Run expected red**

```bash
cargo +1.85.0 test --locked -p oregon-execution --test fees
```

Expected: missing fee module/types.

- [ ] **Step 3: Add the downward dependency and core terms type**

```toml
[dependencies]
oregon-primitives = { path = "../oregon-primitives" }
thiserror = "2"
```

`FeeTermsV1::new(base_fee, max_fee, max_priority, max_weight)` validates the gates and stores `max_escrow` after checked `u128` multiplication/narrowing.

- [ ] **Step 4: Implement canonical capability/ticket encoders**

`FundingCapabilityV1::capability_id()` writes spec fields in exact order and hashes with `OREGON/FEE/CAPABILITY/V1\0`; `EscrowTicketV1::escrow_id()` writes exact ticket fields and hashes with `OREGON/FEE/ESCROW/V1\0`.

- [ ] **Step 5: Implement bounded `EscrowBookV1`**

Use `BTreeMap<Hash256, OpenEscrow>` plus bounded consumed-capability/settled sets. `new(max_open)` rejects zero. `open` fails before mutation on duplicate capability, capacity, insufficient available amount or invalid terms. `settle` removes exactly one open escrow, computes checked charge/refund/components, creates `FeeSettlementReceiptV1`, records settled id and cannot settle twice.

- [ ] **Step 6: Run execution suite green**

```bash
cargo +1.85.0 test --locked -p oregon-execution --all-targets
cargo +1.85.0 clippy --locked -p oregon-execution --all-targets -- -D warnings
```

- [ ] **Step 7: Commit**

```bash
git add crates/oregon-execution Cargo.lock
git commit -m "feat: add deterministic Stage 3B fee escrow"
```

---

### Task 3: Canonical ExecutionAccounting and FeeState SMT writes

**Files:**
- Create: `crates/oregon-contract-state/src/execution_accounting.rs`
- Create: `crates/oregon-contract-state/src/fee_state.rs`
- Modify: `crates/oregon-contract-state/src/lib.rs`
- Test: `crates/oregon-contract-state/tests/execution_accounting.rs`
- Test: `crates/oregon-contract-state/tests/fee_state.rs`

**Interfaces:**
- Produces exact key builders, reserve-outpoint codec, `ExecutionAccountingWritesV1::write_set()`, `FeeStateValuesV1::write_set()`.
- Uses existing `StateWrite`, `StateWriteSet`, `CommitmentDomainId` and `OutPoint`.

- [ ] **Step 1: Write exact key/value tests**

```rust
let address = ExecutionAddress::new(ExecutionAddressKind::Oregon, [0x21; 32]).unwrap();
let mut expected = b"acct/v1/balance/".to_vec();
expected.extend_from_slice(&address.to_bytes());
assert_eq!(balance_key(&address), expected);
assert_eq!(encode_accounting_u64(42), 42u64.to_le_bytes().to_vec());
assert_eq!(encode_reserve_outpoint(None), vec![0x00]);
```

For a present outpoint assert exactly `0x01 || txid[32] || u32_le`. Assert all six FeeState raw key constants byte-for-byte.

- [ ] **Step 2: Run expected red**

```bash
cargo +1.85.0 test --locked -p oregon-contract-state --test execution_accounting --test fee_state
```

- [ ] **Step 3: Implement accounting writes**

`ExecutionAccountingWritesV1` accepts final balance writes `(ExecutionAddress, Option<u64>)`, sequence writes, total execution balance and reserve outpoint. `write_set()` maps `None`/zero balance to `StateWrite::delete`, nonzero balance to 8-byte LE `put`, and always writes total/reserve keys under `CommitmentDomainId::ExecutionAccounting`.

- [ ] **Step 4: Implement FeeState writes**

```rust
pub struct FeeStateValuesV1 {
    pub height: u64,
    pub base_fee_per_weight: u64,
    pub block_weight_used: u64,
    pub execution_fee_total: u64,
    pub producer_coinbase_txid: Hash256,
    pub reserve_transition_id: Hash256,
}
```

`write_set()` creates exactly six puts under `CommitmentDomainId::FeeState` using fixed LE/hash bytes.

- [ ] **Step 5: Add deterministic SMT root tests**

Implement a tiny test-only `StateSource` backed by `BTreeMap<Hash256, StateNode>`/values, apply the write sets with existing `apply_write_set`, prove identical writes produce identical roots and substituting `FeeState` with `ExecutionAccounting` changes the root.

- [ ] **Step 6: Run state suite green and commit**

```bash
cargo +1.85.0 test --locked -p oregon-contract-state --all-targets
git add crates/oregon-contract-state
git commit -m "feat: add Stage 3B accounting state writes"
```

---

### Task 4: Inactive singleton reserve-pool transition and undo

**Files:**
- Create: `crates/oregon-utxo/src/reserve.rs`
- Modify: `crates/oregon-utxo/src/lib.rs`
- Modify: `crates/oregon-utxo/src/state.rs` only to expose crate-private insertion/removal helpers required by `reserve.rs`; do not change normal/block validation behavior.

**Interfaces:**
- Produces `ReservePoolSnapshotV1`, `ReserveTransitionV1Parts`, `ReserveTransitionV1`, `ReserveTransitionError`, `ReserveUndoV1`.

- [ ] **Step 1: Write reserve transition tests inside `reserve.rs`**

Define the helper completely:

```rust
fn parts(previous: u64, deposits: u64, withdrawals: u64, fees: u64, new_total: u64) -> ReserveTransitionV1Parts {
    ReserveTransitionV1Parts {
        chain_id: 7,
        height: 100,
        parent_block_hash: Hash256::from_bytes([0x10; 32]),
        previous: if previous == 0 { None } else { Some(ReservePoolSnapshotV1::test(previous)) },
        native_deposit_total: deposits,
        execution_withdrawal_total: withdrawals,
        execution_fee_total: fees,
        new_execution_balance_total: new_total,
        producer_coinbase_txid: Hash256::from_bytes([0x20; 32]),
    }
}
```

Add zero->deposit, increase, decrease, zero removal, underflow, overflow, wrong previous program/outpoint, stale state, multiple-live reserve and exact transition-id/outpoint tests. Add a regression asserting no post-state root is accepted as an ID input because `ReserveTransitionV1Parts` has no root field.

- [ ] **Step 2: Run expected red**

```bash
cargo +1.85.0 test --locked -p oregon-utxo reserve
```

- [ ] **Step 3: Implement canonical transition preimage**

Encode exactly: version, chain id, height, parent hash, optional previous flag/outpoint, previous amount, deposits, withdrawals, execution fees, new execution total, producer txid. Validate checked reserve equation and exact equality to new total before hashing. Derive new outpoint with primitive helper and index `0` only after transition id exists.

- [ ] **Step 4: Implement crate-private apply/undo**

`apply_reserve_transition_v1(&mut UtxoState, &ReserveTransitionV1)` validates the exact old reserve entry, removes it, inserts at most one new exact reserve entry and returns `ReserveUndoV1`. `undo_reserve_transition_v1` verifies current state matches the forward result before restoring old state. Neither function is called from `apply_normal_transaction` or `connect_block`.

- [ ] **Step 5: Prove forward/undo/forward equivalence**

Clone state before forward, apply, clone result, undo and assert original equality; apply same transition again and assert first-forward equality.

- [ ] **Step 6: Run UTXO suite green and commit**

```bash
cargo +1.85.0 test --locked -p oregon-utxo --all-targets
git add crates/oregon-utxo
git commit -m "feat: add inactive execution reserve pool"
```

---

### Task 5: Inactive execution-fee-aware producer validator

**Files:**
- Modify: `crates/oregon-consensus/src/coinbase.rs`
- Modify: `crates/oregon-consensus/src/lib.rs`
- Test: `crates/oregon-consensus/tests/execution_fee_coinbase.rs`

**Interfaces:**
- Produces `validate_coinbase_with_execution_fees_v1(&Transaction, u64, Amount, Amount, &ConsensusParams) -> Result<(), ConsensusError>`.

- [ ] **Step 1: Write distinguishing expected-red tests**

Prove current `validate_coinbase` still accepts its existing underclaim case. New helper tests reject fee underclaim, missing/wrong final execution-fee output, reserve program as payout and total overclaim; zero execution fees require no dedicated final output.

- [ ] **Step 2: Run expected red**

```bash
cargo +1.85.0 test --locked -p oregon-consensus --test execution_fee_coinbase
```

- [ ] **Step 3: Implement the helper without touching active callers**

Use checked base-unit arithmetic:

```rust
let total_fee_units = native_fees.base_units()
    .checked_add(execution_fees.base_units())
    .ok_or(ConsensusError::ArithmeticOverflow)?;
let total_fees = Amount::from_base_units(total_fee_units)
    .map_err(|_| ConsensusError::ArithmeticOverflow)?;
validate_coinbase(tx, height, total_fees, params)?;
```

Compute `miner_start` exactly as current coinbase logic (`1` only at height 1, else `0`), sum miner outputs checked, require `miner_claim >= total_fee_units`, and when execution fees are nonzero require `tx.outputs.last()` exists in the miner slice, has exact execution-fee value and `locking_program.as_slice() != EXECUTION_RESERVE_LOCKING_PROGRAM_V1`.

Do not call this helper from `validate_non_genesis_block_structure` or `UtxoState::connect_block`.

- [ ] **Step 4: Run all consensus tests and commit**

```bash
cargo +1.85.0 test --locked -p oregon-consensus --all-targets
git add crates/oregon-consensus
git commit -m "feat: add inactive execution fee producer rules"
```

---

### Task 6: Independent vectors, cross-crate consumers and mutation gate

**Files:**
- Create: `scripts/generate_fee_settlement_vectors.py`
- Create: `tests/vectors/fee-settlement-v1.json`
- Create: `crates/oregon-primitives/tests/fee_settlement_vectors.rs`
- Create: `crates/oregon-execution/tests/fee_vectors.rs`
- Create: `crates/oregon-consensus/tests/execution_fee_vectors.rs`
- Add vector consumption to `oregon-utxo` reserve tests.
- Create: `scripts/verify_fee_settlement_mutations.py`
- Create: `.github/workflows/oregon-fee-settlement.yml`
- Modify: `.github/workflows/oregon-rust.yml`

**Interfaces:**
- Python is the independent oracle and never calls Rust.
- Required mutation names: `max_fee_below_base_fee_bypass`, `fee_product_unchecked`, `refund_off_by_one`, `revert_fee_zeroed`, `duplicate_settlement_allowed`, `stale_capability_allowed`, `reserve_program_bypass`, `reserve_underflow_allowed`, `reserve_backing_equality_bypass`, `fee_accumulator_double_count`, `producer_payout_omitted`, `fee_state_domain_substitution`, `transition_root_cycle_regression`, `reserve_undo_mismatch`.

- [ ] **Step 1: Implement the Python domain hash and arithmetic oracle**

```python
import hashlib

def domain_hash(domain: bytes, payload: bytes) -> bytes:
    return hashlib.sha256(domain + payload).digest()

def checked_fee(max_weight: int, max_fee: int, base_fee: int, priority: int, actual_weight: int):
    if max_fee < base_fee or priority > max_fee or not (1 <= actual_weight <= max_weight):
        raise ValueError("invalid fee terms")
    max_escrow = max_weight * max_fee
    effective = min(max_fee, base_fee + priority)
    charged = actual_weight * effective
    base = actual_weight * base_fee
    return max_escrow, effective, charged, base, charged - base, max_escrow - charged
```

Before committing vectors, compare this domain-hash construction against an already committed Oregon Python generator and reuse that established construction exactly if its framing differs.

- [ ] **Step 2: Generate literal vectors and add `--check`**

Vectors cover fee arithmetic, canonical receipt bytes/hash, reserve transition preimage/id/outpoint, reserve deltas and producer boundaries. `--check` regenerates in memory and byte-compares with committed JSON.

```bash
python3 scripts/generate_fee_settlement_vectors.py
python3 scripts/generate_fee_settlement_vectors.py --check
```

- [ ] **Step 3: Add Rust vector consumers**

Each consumer reads only committed JSON and compares literal expected bytes/hashes/amounts. No Rust test shells out to Python.

- [ ] **Step 4: Implement mutation runner with exact replacements**

Use a table of `(name, file, exact_old, exact_new, test_command, expected_test_name)`. Before mutation require `exact_old` occurs once; after the intended test fails, restore the original bytes in `finally`; hash the restored file and run the clean focused test. Compile failure is rejected as a kill unless the intended test itself executed and failed.

- [ ] **Step 5: Add x86_64/ARM vector workflow and Rust CI gate**

`oregon-fee-settlement.yml` uses matrix `[ubuntu-24.04, ubuntu-24.04-arm]`, pinned checkout matching current repo practice, Rust 1.85.0, native build prerequisites, Python `--check`, and focused Rust vector tests. `oregon-rust.yml` adds Stage 3B spec/dependency scans, focused tests and mutation runner before docs/fmt/clippy.

- [ ] **Step 6: Run focused/full verification and commit**

```bash
python3 scripts/generate_fee_settlement_vectors.py --check
python3 scripts/verify_fee_settlement_mutations.py
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
git diff --check
git add scripts tests/vectors crates .github/workflows
git commit -m "test: gate Stage 3B fee settlement security"
```

---

### Task 7: Exact-head verification, checkpoint and draft PR

**Files:**
- Create: `docs/checkpoints/OREGON_FEE_SETTLEMENT_RESERVE_PROGRESS.md`
- Modify: `HANDOFF.md`

**Interfaces:**
- Checkpoint records exact head/tree, vector count, mutation count, CI run/job IDs, limitations and next action.

- [ ] **Step 1: Run the complete local gate**

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

Record any genuine local native-toolchain limitation; do not substitute a partial check for the CI result.

- [ ] **Step 2: Push implementation branch and open draft PR against current main**

PR body names the design/plan, explicitly says inactive/no activation, and lists exact local evidence. Do not merge.

- [ ] **Step 3: Require exact-head CI**

The exact PR head must pass Oregon Rust CI and Stage 3B x86_64/ARM vectors. A green ancestor is not evidence for a changed head.

- [ ] **Step 4: Write checkpoint and update HANDOFF**

Checkpoint records exact successful SHA/tree/run/job IDs and all mutation kills. HANDOFF points to the verified Stage 3B branch/checkpoint and names the next separate design: runtime/journal/async execution core. State that VM backends, durable chainstate composition and protocol activation are still future work.

- [ ] **Step 5: Commit documentation and stop before main**

```bash
git add docs/checkpoints/OREGON_FEE_SETTLEMENT_RESERVE_PROGRESS.md HANDOFF.md
git commit -m "docs: checkpoint Stage 3B fee settlement foundation"
```

Main integration remains a separate explicit owner decision.
