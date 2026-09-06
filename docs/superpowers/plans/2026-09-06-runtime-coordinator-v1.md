# Oregon Stage 4B Runtime ABI and Transaction Coordinator V1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the inactive Stage 4B VM-neutral runtime ABI and single Oregon transaction coordinator that composes Stage 3A metering, Stage 3B escrow/settlement, Stage 4A journal rollback, bounded events/return data, typed execution-balance movement, and byte-exact execution receipts without activating a production VM or protocol path.

**Architecture:** `oregon-runtime` owns only deterministic ABI data/traits and depends downward on `oregon-primitives`. `oregon-execution` remains the sole transaction coordinator and owns the meter, escrow book, journal/effect-frame lifecycle, dispatch, accounting composition, settlement and two-phase unpublished proposal. EVM/WASM production engines, chainstate publication, mempool/RPC integration and protocol activation remain outside this plan.

**Tech Stack:** Rust 1.85.0 / edition 2024, `thiserror`, existing Oregon BLAKE3/domain-hash primitives, existing Stage 2 Oregon SMT APIs, Stage 3A `WeightMeter`, Stage 3B `EscrowBookV1`, Stage 4A `ExecutionJournalV1`, Python 3 independent vector/mutation scripts, GitHub Actions x86_64 + ARM.

**Spec:** `docs/superpowers/specs/2026-09-06-runtime-coordinator-v1-design.md`

## Global Constraints

- Base approved design head: `f59b6f2b0382e191cd6555bd06c8bde33e77f104`; execution starts from the eventual plan successor head, never from an older main checkout.
- Work on isolated branch `work/runtime-coordinator-v1-2026-09-06`; do not alter `main` without a later separate owner integration decision.
- Rust version is exactly `1.85.0`; all crates remain `#![forbid(unsafe_code)]` except the existing `oregon-pow` RandomX boundary.
- `oregon-runtime` may depend on `oregon-primitives` and `thiserror`; it must not depend on execution, contract-state, consensus, UTXO, storage, chainstate, networking, node, RPC or VM implementation crates.
- `oregon-execution` may add `oregon-runtime`; it must not depend on storage, chainstate, RPC, `oregon-vm-evm` or `oregon-vm-wasm`.
- Stage 4B production state host support is only WASM logical state through `CommitmentDomainId::Wasm` / `OregonSmtV1`; EVM-labelled test backends never claim production EVM state support.
- One transaction owns one Stage 3A `WeightMeter`; nested rollback never reduces consumption and exhaustion remains sticky at `max_weight`.
- VM-facing code never receives `WeightMeter`, `EscrowBookV1`, `ExecutionJournalV1`, `CommitmentDomainId`, raw `ExecutionAccounting`, `FeeState`, receipt or async-system key authority.
- `FundingCapabilityV1::new` alone is not production spending authority; the coordinator consumes capabilities only after the trusted source-validation boundary binds transaction/source context.
- Root journal frame is coordinator-owned settlement/accounting state; VM-visible execution begins in a child frame. Fee effects survive deterministic contract revert but remain unpublished until candidate acceptance.
- Call depth including top level: 64. Nested call input: 262,144 bytes. Return data per call: 262,144 bytes. Event data: 65,536 bytes. Event topics: 4. Events per transaction: 256. Aggregate live retained event + return bytes: 2,097,152.
- Nonzero generic V1 call value is allowed only when caller and target kinds are EVM or WASM. `Oregon`/`System` kinds cannot originate or receive generic call value.
- Read-only authority is monotonic: `effective_read_only = parent_read_only || requested_read_only`.
- Stage 4B exposes no VM-visible async host API. Execution receipt outbox count is zero and uses the canonical empty outbox-effect root.
- `ExecutionReceiptV1` is exactly 259 bytes. Phase-A state-effect descriptors exclude `ExecutionReceipts`; Phase B inserts fee and execution receipts under the exact V1 receipt keys.
- Fatal state/accounting/commitment/coordinator errors never become catchable VM reverts; resource exhaustion cannot be converted to success even if a backend ignores an abort signal.
- No production EVM/WASM engine, no current block/header/transaction byte change, no mempool/RPC/storage/chainstate activation, and no protocol activation in this slice.
- Each implementation task follows RED -> minimal implementation -> focused PASS -> broader regression -> commit. No lint suppression, task-numbered production modules, placeholder production paths or duplicate authoritative fee/state arithmetic.

---

## File Structure

Create or modify only these responsibility-oriented surfaces unless an implementation detail forces a documented adjustment:

- `crates/oregon-primitives/src/execution_event.rs` — canonical event bytes/id/root.
- `crates/oregon-primitives/src/execution_effect.rs` — state-effect descriptor validation/bytes/root.
- `crates/oregon-primitives/src/execution_receipt.rs` — fixed 259-byte execution receipt, cross-field validation and ID.
- `crates/oregon-primitives/src/lib.rs` — export those canonical modules.
- `crates/oregon-primitives/tests/runtime_receipts.rs` — byte-exact primitive contracts.
- `crates/oregon-primitives/tests/runtime_receipt_vectors.rs` — independent JSON vector consumer.
- `crates/oregon-runtime/Cargo.toml`, `src/lib.rs`, `src/types.rs`, `src/host.rs` — ABI-only crate.
- `crates/oregon-runtime/tests/runtime_abi.rs` — constructor/limit/discriminant/trait contract tests.
- `crates/oregon-contract-state/src/execution_accounting.rs` — add authoritative accounting decoders needed by coordinator; do not move key ownership.
- `crates/oregon-contract-state/tests/execution_accounting.rs` — malformed-width/round-trip decoder tests.
- `crates/oregon-execution/src/coordinator.rs` — module root only.
- `crates/oregon-execution/src/coordinator/types.rs` — coordinator input/result/error/terminal state.
- `crates/oregon-execution/src/coordinator/effects.rs` — frame-local event/return byte accounting.
- `crates/oregon-execution/src/coordinator/host.rs` — capability-limited host adapter and latching abort behavior.
- `crates/oregon-execution/src/coordinator/accounting.rs` — journal-based balance/total reads and checked staged deltas.
- `crates/oregon-execution/src/coordinator/calls.rs` — nested frame creation, read-only propagation, value transfer and dispatch.
- `crates/oregon-execution/src/coordinator/settlement.rs` — pre-exec authority, escrow reservation, outcome mapping and Stage 3B settlement composition.
- `crates/oregon-execution/src/coordinator/proposal.rs` — Phase A effect root + Phase B receipt journal + all-or-nothing unpublished proposal.
- `crates/oregon-execution/src/lib.rs`, `Cargo.toml` — internal coordinator module and runtime dependency.
- `crates/oregon-execution/src/coordinator/tests.rs` — white-box adversarial coordinator tests/test-only backends.
- `scripts/generate_runtime_coordinator_vectors.py` — independent canonical byte/hash and simple outcome oracle.
- `tests/vectors/runtime-coordinator-v1.json` — frozen independent corpus.
- `scripts/verify_runtime_coordinator_mutations.py` — 16 required compiled/killed mutations with source restoration.
- `.github/workflows/oregon-runtime-coordinator.yml` — x86_64/ARM independent vector + focused test workflow.
- `.github/workflows/oregon-rust.yml` — architecture contract presence, runtime/coordinator contract step and mutation gate.
- `docs/checkpoints/OREGON_RUNTIME_COORDINATOR_PROGRESS.md`, `HANDOFF.md` — exact-head closure evidence only after implementation verification.

---

### Task 1: Canonical Event, Effect and Execution Receipt Primitives

**Files:**
- Create: `scripts/generate_runtime_coordinator_vectors.py`
- Create: `tests/vectors/runtime-coordinator-v1.json`
- Create: `crates/oregon-primitives/src/execution_event.rs`
- Create: `crates/oregon-primitives/src/execution_effect.rs`
- Create: `crates/oregon-primitives/src/execution_receipt.rs`
- Create: `crates/oregon-primitives/tests/runtime_receipts.rs`
- Create: `crates/oregon-primitives/tests/runtime_receipt_vectors.rs`
- Modify: `crates/oregon-primitives/src/lib.rs`

**Interfaces:**
- Consumes: `ExecutionAddress`, `ExecutionDomain`, `CommitmentDomainId`, `CommitmentSchemeId`, `FeeSettlementReceiptV1`, `Hash256`, `domain_hash`.
- Produces:
  - `ExecutionEventV1::new(emitter: ExecutionAddress, topics: Vec<Hash256>, data: Vec<u8>) -> Result<Self, ExecutionReceiptError>`
  - `ExecutionEventV1::encode(&self) -> Vec<u8>` and `event_id(&self) -> Hash256`
  - `events_root(events: &[ExecutionEventV1]) -> Result<Hash256, ExecutionReceiptError>`
  - `StateEffectDescriptorV1::new(domain_id, scheme_id, old_root, new_root) -> Result<Self, ExecutionReceiptError>`
  - `state_effect_root(descriptors: &[StateEffectDescriptorV1]) -> Result<Hash256, ExecutionReceiptError>`
  - `ExecutionReceiptOutcomeV1::{Committed, Reverted, Trapped, ResourceExhausted}`
  - `ExecutionReceiptV1::new(parts: ExecutionReceiptV1Parts, fee_receipt: &FeeSettlementReceiptV1) -> Result<Self, ExecutionReceiptError>`
  - `encode(&self) -> [u8; 259]`, `decode(&[u8])`, `receipt_id(&self)`.

- [ ] **Step 1: Write the independent oracle and freeze RED vectors before Rust production code exists**

Implement Python using only `struct`, `json`, `pathlib` and an independent BLAKE3 implementation/copy already proven by `generate_journal_vectors.py`; do not import Rust outputs. The corpus must include one event root, a multi-event ordered root, one Oregon-SMT effect set, a synthetic EVM `EvmCommitmentV1` descriptor, duplicate/noncanonical/receipt-domain negative metadata, empty outbox root, return-data hashes and all four 259-byte receipt outcomes.

```python
RECEIPT_BYTES = 259
EVENT_DOMAIN = b"OREGON/EXEC/EVENT/V1\0"
EVENTS_DOMAIN = b"OREGON/EXEC/EVENTS/V1\0"
STATE_EFFECT_DOMAIN = b"OREGON/EXEC/STATE-EFFECT/V1\0"
RETURN_DOMAIN = b"OREGON/EXEC/RETURN/V1\0"
OUTBOX_EFFECT_DOMAIN = b"OREGON/EXEC/OUTBOX-EFFECT/V1\0"
EXEC_RECEIPT_DOMAIN = b"OREGON/EXEC/RECEIPT/V1\0"

def empty_outbox_root() -> bytes:
    return domain_hash(OUTBOX_EFFECT_DOMAIN, struct.pack("<I", 0))
```

Run: `python3 scripts/generate_runtime_coordinator_vectors.py`
Expected: writes `tests/vectors/runtime-coordinator-v1.json`, self-checks independent hash fixtures, and exits 0.

- [ ] **Step 2: Write Rust vector/contract tests and verify RED**

Tests must assert literal 259-byte lengths, literal IDs/roots from JSON, exact outcome/trap-code rules, fee payer/weight/charge cross-checks, event limits, descriptor sort/duplicate rejection, `ExecutionReceipts` descriptor rejection and EVM scheme mismatch rejection.

```rust
#[test]
fn trapped_receipt_requires_nonzero_trap_code() {
    let err = ExecutionReceiptV1::new(parts(ExecutionReceiptOutcomeV1::Trapped, 0), &fee()).unwrap_err();
    assert_eq!(err, ExecutionReceiptError::InvalidTrapCode);
}

#[test]
fn receipt_domain_is_forbidden_from_phase_a_effects() {
    let err = state_effect_root(&[descriptor(CommitmentDomainId::ExecutionReceipts)]).unwrap_err();
    assert!(matches!(err, ExecutionReceiptError::ReceiptDomainInStateEffects));
}
```

Run: `cargo +1.85.0 test --locked -p oregon-primitives --test runtime_receipts --test runtime_receipt_vectors`
Expected: FAIL because the new modules/types do not exist.

- [ ] **Step 3: Implement the minimal canonical primitives**

Use fixed discriminants from the spec, checked lengths/counts, strict descriptor order and protocol-approved `(domain, scheme)` pairs. `ExecutionReceiptV1::new` must recompute/cross-check fee payer, actual weight and charge from the supplied `FeeSettlementReceiptV1`; do not trust duplicated receipt fields independently.

```rust
pub const EXECUTION_RECEIPT_BYTES_V1: usize = 259;

#[repr(u8)]
pub enum ExecutionReceiptOutcomeV1 {
    Committed = 0x00,
    Reverted = 0x01,
    Trapped = 0x02,
    ResourceExhausted = 0x03,
}
```

- [ ] **Step 4: Verify focused and primitive regression tests**

Run:
```bash
python3 scripts/generate_runtime_coordinator_vectors.py --check
cargo +1.85.0 test --locked -p oregon-primitives --test runtime_receipts --test runtime_receipt_vectors
cargo +1.85.0 test --locked -p oregon-primitives --all-targets
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add scripts/generate_runtime_coordinator_vectors.py tests/vectors/runtime-coordinator-v1.json crates/oregon-primitives
git commit -m "feat(stage4b): add canonical execution receipt primitives"
```

---

### Task 2: `oregon-runtime` ABI-Only Crate

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/oregon-runtime/Cargo.toml`
- Create: `crates/oregon-runtime/src/lib.rs`
- Create: `crates/oregon-runtime/src/types.rs`
- Create: `crates/oregon-runtime/src/host.rs`
- Create: `crates/oregon-runtime/tests/runtime_abi.rs`

**Interfaces:**
- Consumes: primitive `ExecutionAddress`, `ExecutionAddressKind`, `ExecutionDomain`, `Hash256`.
- Produces:
  - `RuntimeCallSpecV1::new(target, value, read_only, input) -> Result<Self, RuntimeAbiError>`
  - `RuntimeCallContextV1` with coordinator-only constructor kept `pub(crate)` impossible across crates; therefore expose `RuntimeCallContextV1Parts` only to execution through a deliberately named `RuntimeCallContextV1::from_trusted_parts` public constructor that validates target/domain/depth but grants no state authority.
  - `RuntimeTrapCodeV1` exact `u16` discriminants `0x0001..=0x000b`.
  - `RuntimeCallResultV1::{Success(Vec<u8>), Revert(Vec<u8>), Trap(RuntimeTrapCodeV1)}` with return-data ceiling enforcement.
  - `RuntimeHostSignalV1::{Trap(RuntimeTrapCodeV1), Abort}`; `Abort` is opaque and coordinator-latched.
  - `RuntimeHostV1` trait with `context`, `state_get`, `state_put`, `state_delete`, `emit_event`, `call`, `charge_vm_units`, `charge_common`.
  - `RuntimeBackendV1` trait returning `Result<RuntimeCallResultV1, RuntimeBackendFailureV1>` where backend failure is fatal to candidate composition.

- [ ] **Step 1: Write ABI RED tests**

Cover exact/one-over call input and return data, only EVM/WASM call targets, unknown `u16` trap conversion, read-only/context field preservation and trait object safety.

```rust
#[test]
fn call_input_one_over_is_rejected_before_retention() {
    let input = vec![0u8; MAX_RUNTIME_CALL_INPUT_BYTES + 1];
    let err = RuntimeCallSpecV1::new(evm(), 0, false, input).unwrap_err();
    assert_eq!(err, RuntimeAbiError::CallInputTooLarge);
}

fn assert_host_object_safe(_: &mut dyn RuntimeHostV1) {}
```

Run: `cargo +1.85.0 test --locked -p oregon-runtime`
Expected: FAIL because workspace member/crate is absent.

- [ ] **Step 2: Create crate and exact type/trait surface**

`crates/oregon-runtime/Cargo.toml` dependencies are only:

```toml
[dependencies]
oregon-primitives = { path = "../oregon-primitives" }
thiserror = "2"
```

`src/lib.rs` begins with `#![forbid(unsafe_code)]` and re-exports only ABI types/traits, not implementation adapters.

- [ ] **Step 3: Implement validation with no VM/state logic**

`RuntimeCallSpecV1` accepts only EVM/WASM targets. `RuntimeCallContextV1::from_trusted_parts` validates depth `1..=64`, target kind/domain correspondence and `transferred_value` without attempting authorization. `RuntimeCallResultV1` constructors reject return data over 262,144 bytes.

- [ ] **Step 4: Verify runtime and architecture dependency direction**

Run:
```bash
cargo +1.85.0 test --locked -p oregon-runtime --all-targets
cargo +1.85.0 tree -p oregon-runtime
```
Expected: tests PASS; dependency tree contains `oregon-primitives`/its transitive dependencies and `thiserror`, with no execution/state/consensus/storage/network/VM crate.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/oregon-runtime
git commit -m "feat(stage4b): add deterministic runtime ABI"
```

---

### Task 3: Coordinator Frame and Effect-Stack Foundation

**Files:**
- Modify: `crates/oregon-execution/Cargo.toml`
- Modify: `crates/oregon-execution/src/lib.rs`
- Create: `crates/oregon-execution/src/coordinator.rs`
- Create: `crates/oregon-execution/src/coordinator/types.rs`
- Create: `crates/oregon-execution/src/coordinator/effects.rs`
- Create: `crates/oregon-execution/src/coordinator/tests.rs`

**Interfaces:**
- Consumes: `ExecutionJournalV1`, `JournalContextV1`, `JournalLimitsV1`, runtime call context/results, primitive events.
- Produces internal types:
  - `CoordinatorLimitsV1::new(max_call_depth, max_events, max_effect_bytes) -> Result<Self, CoordinatorError>` bounded by spec ceilings.
  - `EffectFrameV1 { events, retained_return_bytes }` private.
  - `EffectStackV1::begin/commit/revert` kept in lock-step with journal frames.
  - `CoordinatorTerminalV1::{Running, ResourceExhausted, Fatal}` private latch.

- [ ] **Step 1: Write RED white-box tests for frame/effect atomicity**

Tests live under `#[cfg(test)] mod tests` so production coordinator APIs need not be exported prematurely. Cover root misuse, child success merge, child revert discarding events, ancestor revert discarding committed descendant effects, exact/one-over event count and aggregate retained bytes, and frame-stack mismatch becoming fatal.

```rust
#[test]
fn reverted_child_releases_effect_bytes_but_not_frame_history() {
    let mut effects = EffectStackV1::new(limits());
    effects.begin().unwrap();
    effects.push_event(event_with_data(1024)).unwrap();
    effects.revert().unwrap();
    assert_eq!(effects.live_retained_bytes(), 0);
    assert_eq!(effects.total_frames_created(), 2);
}
```

Run: `cargo +1.85.0 test --locked -p oregon-execution coordinator::tests::effects`
Expected: FAIL because coordinator modules do not exist.

- [ ] **Step 2: Add `oregon-runtime` dependency and module skeleton**

Add only `oregon-runtime = { path = "../oregon-runtime" }`. Keep `mod coordinator;` private in `lib.rs` during this slice unless a later task demonstrates a real cross-crate consumer.

- [ ] **Step 3: Implement effect frames with pre-copy bounds**

Event count and event data are validated before cloning. Aggregate live retained event + return bytes is checked with `checked_add`; child commit moves ownership, child revert subtracts live bytes, and root cannot be ended through child APIs.

- [ ] **Step 4: Verify focused plus journal regression**

Run:
```bash
cargo +1.85.0 test --locked -p oregon-execution coordinator::tests::effects
cargo +1.85.0 test --locked -p oregon-execution --test journal --test journal_security --test journal_vectors
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.lock crates/oregon-execution
git commit -m "feat(stage4b): add coordinator effect frame foundation"
```

---

### Task 4: Capability-Limited Host and Shared Meter Composition

**Files:**
- Create: `crates/oregon-execution/src/coordinator/host.rs`
- Modify: `crates/oregon-execution/src/coordinator.rs`
- Modify: `crates/oregon-execution/src/coordinator/types.rs`
- Modify: `crates/oregon-execution/src/coordinator/tests.rs`

**Interfaces:**
- Consumes: one mutable `WeightMeter`, `MeterScheduleV1`, `RuntimeHostV1`, active runtime context, active journal/effect frame.
- Produces:
  - `HostChargeScheduleV1` validated synthetic/inactive integer coefficients.
  - `CoordinatorHostV1<'a, ...>` private implementation of `RuntimeHostV1`.
  - Latched abort state checked by coordinator after every backend return.

- [ ] **Step 1: Write RED tests for one-meter and abort semantics**

Cover backend attempts to choose a cheaper resource domain, rollback not refunding meter, common host charging, exact budget, one-over sticky exhaustion, backend catching `Abort` then returning `Success` still yielding terminal `ResourceExhausted`, and host input-size charge before copy.

```rust
#[test]
fn backend_success_cannot_mask_latched_exhaustion() {
    let result = run_backend(BackendThatCatchesAbortAndReturnsSuccess, max_weight_10());
    assert_eq!(result.outcome(), CoordinatorOutcomeV1::ResourceExhausted);
    assert_eq!(result.actual_weight(), 10);
}
```

Run: `cargo +1.85.0 test --locked -p oregon-execution coordinator::tests::meter`
Expected: FAIL.

- [ ] **Step 2: Implement `HostChargeScheduleV1` checked arithmetic**

Use `u64` coefficients and `u128` intermediates. Fixed categories include state-read base/input/copy, state-write base/key/value, delete, event base/topic/data, nested-call base/input/return-copy and context query. Synthetic test schedules are explicit constructor inputs; no network constants are introduced.

- [ ] **Step 3: Implement domain-derived VM charging and latched abort**

Map active `ExecutionDomain::Evm` to `ResourceDomain::Evm` and `ExecutionDomain::Wasm` to `ResourceDomain::Wasm`; unsupported domains latch fatal/abort. `RuntimeHostSignalV1::Abort` is never sufficient to clear coordinator terminal state.

- [ ] **Step 4: Verify meter regressions**

Run:
```bash
cargo +1.85.0 test --locked -p oregon-execution coordinator::tests::meter
cargo +1.85.0 test --locked -p oregon-execution --test resource_meter --test resource_meter_properties
```
If existing resource tests use different exact filenames, use `cargo +1.85.0 test --locked -p oregon-execution --all-targets` and record the discovered stable names in the checkpoint; do not rename inherited tests merely for this plan.
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/oregon-execution
git commit -m "feat(stage4b): compose runtime host with shared meter"
```

---

### Task 5: Authoritative Accounting Decode and Escrow Reservation Boundary

**Files:**
- Modify: `crates/oregon-contract-state/src/execution_accounting.rs`
- Create or modify: `crates/oregon-contract-state/tests/execution_accounting.rs`
- Create: `crates/oregon-execution/src/coordinator/accounting.rs`
- Create: `crates/oregon-execution/src/coordinator/settlement.rs`
- Modify: `crates/oregon-execution/src/coordinator/types.rs`
- Modify: `crates/oregon-execution/src/coordinator/tests.rs`

**Interfaces:**
- Consumes: existing accounting key builders, journal `read/put/delete`, `FundingCapabilityV1`, `FeeTermsV1`, `EscrowBookV1`.
- Produces:
  - `decode_accounting_u64(bytes: &[u8]) -> Result<u64, StateError>` in contract-state as authoritative inverse of `encode_accounting_u64`.
  - Private coordinator `read_balance`, `write_balance`, `read_total_execution_balance` using contract-state key/codec owners.
  - `FundingSourceValidatorV1` trait at execution composition boundary.
  - `FundingValidationRequestV1` binding chain/height/txid/domain/principal/payer/authorization/fee terms.
  - Root-frame execution-funded escrow reservation before execution child frame.

- [ ] **Step 1: Write RED decoder and escrow-visibility tests**

```rust
#[test]
fn accounting_u64_rejects_non_eight_byte_values() {
    assert!(decode_accounting_u64(&[0u8; 7]).is_err());
    assert!(decode_accounting_u64(&[0u8; 9]).is_err());
}

#[test]
fn max_escrow_is_not_spendable_inside_execution_child() {
    let observed = backend_observed_balance(payer_with_100(), max_escrow_40());
    assert_eq!(observed, 60);
}
```

Run:
```bash
cargo +1.85.0 test --locked -p oregon-contract-state --test execution_accounting
cargo +1.85.0 test --locked -p oregon-execution coordinator::tests::escrow
```
Expected: FAIL on missing decoder/coordinator composition.

- [ ] **Step 2: Implement strict accounting decoder in owner crate**

```rust
pub fn decode_accounting_u64(bytes: &[u8]) -> Result<u64, StateError> {
    let array: [u8; 8] = bytes.try_into().map_err(|_| StateError::InvalidAccountingValueLength(bytes.len()))?;
    Ok(u64::from_le_bytes(array))
}
```

Add the exact descriptive `StateError` variant in the existing owner error enum instead of mapping malformed state to a generic string.

- [ ] **Step 3: Implement trusted validator trait and capability consistency checks**

The coordinator checks returned capability payer, source kind, source commitment/sequence context and amount against the validated request before `EscrowBookV1::open`. Unit-test validators are `#[cfg(test)]` only. There is no public constructor that treats arbitrary capability data as fresh authority.

- [ ] **Step 4: Implement root reservation for execution-funded payer**

Read current payer balance through journal, require `balance >= max_escrow`, stage `balance - max_escrow` in root, keep total execution balance unchanged during reservation, then open top-level execution child. Native-funded escrow stages no execution-accounting debit.

- [ ] **Step 5: Verify focused and inherited fee/accounting tests, then commit**

Run:
```bash
cargo +1.85.0 test --locked -p oregon-contract-state --all-targets
cargo +1.85.0 test --locked -p oregon-execution coordinator::tests::escrow
cargo +1.85.0 test --locked -p oregon-execution --test fees --test fee_vectors
```
Expected: PASS.

```bash
git add crates/oregon-contract-state crates/oregon-execution
git commit -m "feat(stage4b): bind escrow to authoritative accounting"
```

---

### Task 6: Scoped WASM State, Nested Calls, Read-Only Propagation and Revertible Call Value

**Files:**
- Create: `crates/oregon-execution/src/coordinator/calls.rs`
- Modify: `crates/oregon-execution/src/coordinator/host.rs`
- Modify: `crates/oregon-execution/src/coordinator/accounting.rs`
- Modify: `crates/oregon-execution/src/coordinator/tests.rs`

**Interfaces:**
- Consumes: runtime call spec/context/result, journal frame lifecycle, effect frames, accounting helpers, active dispatch table.
- Produces:
  - Scoped WASM state key mapping owned by execution coordinator, e.g. `b"wasm/v1/storage/" || target[33] || local_key` before Stage 2 path hashing; the backend never supplies a domain id.
  - Nested-call coordinator that opens journal/effect frames together and derives caller/target/domain/depth/read-only.
  - Checked call-value debit/credit in child frame with unchanged total execution balance.

- [ ] **Step 1: Write RED tests for state scope, read-only and call value**

Cover two WASM targets using identical local keys without collision, EVM-labelled backend denied production state access, parent read-only + child requested false remains read-only, exact depth 64 succeeds and 65 traps, EVM/WASM value transfer success, insufficient value trap `0x0009`, System/Oregon value rejection, child revert/trap rolling back value and events, and ancestor revert discarding committed descendants.

```rust
#[test]
fn read_only_is_monotonic_across_nested_calls() {
    let child = derive_child_context(parent_read_only(), call_spec(false));
    assert!(child.read_only());
}
```

Run: `cargo +1.85.0 test --locked -p oregon-execution coordinator::tests::calls`
Expected: FAIL.

- [ ] **Step 2: Implement WASM-scoped state host only**

State `get/put/delete` map to `CommitmentDomainId::Wasm`; EVM state host calls return deterministic `StateAccessDenied` because no EVM overlay exists in Stage 4B. Read-only put/delete return `WriteInReadOnlyCall`. No host path can construct ExecutionAccounting/FeeState/receipt/async keys.

- [ ] **Step 3: Implement nested frame creation and context derivation**

Validate target/input/depth before frame growth. Open journal child and effect child together. Use `effective_read_only = parent || requested`. On result `Success` commit both; on `Revert`/`Trap` revert both. On latched abort or fatal mismatch unwind to transaction-level handling.

- [ ] **Step 4: Implement checked attached-value transfer**

Both caller/target kinds must be EVM/WASM. Read current overlay balances, checked debit/credit within the newly opened child frame and leave `total_execution_balance` unchanged. Any malformed accounting is fatal; insufficient clean balance is deterministic trap.

- [ ] **Step 5: Verify focused/journal tests and commit**

Run:
```bash
cargo +1.85.0 test --locked -p oregon-execution coordinator::tests::calls
cargo +1.85.0 test --locked -p oregon-execution --test journal --test journal_security --test journal_vectors
```
Expected: PASS.

```bash
git add crates/oregon-execution
git commit -m "feat(stage4b): add scoped nested runtime calls"
```

---

### Task 7: Top-Level Outcome and Stage 3B Settlement Mapping

**Files:**
- Modify: `crates/oregon-execution/src/coordinator/settlement.rs`
- Modify: `crates/oregon-execution/src/coordinator/types.rs`
- Modify: `crates/oregon-execution/src/coordinator/accounting.rs`
- Modify: `crates/oregon-execution/src/coordinator/tests.rs`

**Interfaces:**
- Consumes: `EscrowTicketV1`, `EscrowBookV1::settle`, meter actual weight, top-level runtime result, root accounting reservation.
- Produces `CoordinatorOutcomeV1::{Committed, Reverted, Trapped(RuntimeTrapCodeV1), ResourceExhausted}` and one settled fee receipt for every executed nonfatal transaction.

- [ ] **Step 1: Write RED settlement matrix tests**

Use one fixed fee schedule/weight and assert committed, reverted and trapped outcomes charge equal amounts for equal consumed weight; trapped maps to fee `Reverted`; resource exhaustion uses exactly `max_weight`; pre-escrow invalidity creates no receipt; double settle fails; fatal after escrow leaks no publishable result.

```rust
#[test]
fn deterministic_trap_maps_to_reverted_fee_outcome() {
    let result = execute_with_top_level_trap();
    assert_eq!(result.fee_receipt().outcome(), ExecutionOutcome::Reverted);
    assert!(matches!(result.outcome(), CoordinatorOutcomeV1::Trapped(_)));
}
```

Run: `cargo +1.85.0 test --locked -p oregon-execution coordinator::tests::settlement`
Expected: FAIL.

- [ ] **Step 2: Implement top-level child commit/revert mapping**

Committed backend result commits top-level child. Explicit revert/trap reverts it. Latched resource exhaustion always reverts it even if backend returned success. Fatal state prevents settlement result from escaping.

- [ ] **Step 3: Apply refund and actual execution-funded charge in root**

For execution-funded payer after `settle`: add `refund` to current root-visible payer balance; reduce `total_execution_balance` by exactly `charged`; keep the charge deducted from payer spendable balance. Native-funded settlement makes no execution-accounting total change. Use checked arithmetic only.

- [ ] **Step 4: Verify fee/resource/accounting regressions**

Run:
```bash
cargo +1.85.0 test --locked -p oregon-execution coordinator::tests::settlement
cargo +1.85.0 test --locked -p oregon-execution --test fees --test fee_vectors
python3 scripts/generate_execution_resource_vectors.py --check
python3 scripts/generate_fee_settlement_vectors.py --check
```
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/oregon-execution
git commit -m "feat(stage4b): settle runtime outcomes exactly once"
```

---

### Task 8: Two-Phase State Effects and Receipt Proposal

**Files:**
- Create: `crates/oregon-execution/src/coordinator/proposal.rs`
- Modify: `crates/oregon-execution/src/coordinator.rs`
- Modify: `crates/oregon-execution/src/coordinator/types.rs`
- Modify: `crates/oregon-execution/src/coordinator/tests.rs`

**Interfaces:**
- Consumes: Phase-A `JournalResultV1`, canonical `StateEffectDescriptorV1`, fee receipt, `ExecutionReceiptV1`, fresh `ExecutionReceipts` snapshot/journal.
- Produces private `TransactionExecutionProposalV1 { phase_a: JournalResultV1, phase_b: JournalResultV1, fee_receipt, execution_receipt, execution_fee_total_delta }` only after both phases succeed.

- [ ] **Step 1: Write RED proposal tests**

Cover Phase-A success + Phase-B corrupt source returns error/no proposal; `ExecutionReceipts` absent from state-effect descriptors; Oregon-SMT descriptors use `OregonSmtV1`; synthetic EVM descriptor requires `EvmCommitmentV1`; duplicate/noncanonical descriptors fail; existing fee/execution receipt key causes fatal duplicate; receipt payer/weight/charge/fee-id cross-checks; all four top-level outcomes produce exact vector bytes.

```rust
#[test]
fn phase_b_failure_discards_phase_a_result() {
    let err = execute_with_corrupt_receipt_snapshot().unwrap_err();
    assert!(matches!(err, CoordinatorError::ReceiptState(_)));
    assert_eq!(published_proposals(), 0);
}
```

Run: `cargo +1.85.0 test --locked -p oregon-execution coordinator::tests::proposal`
Expected: FAIL.

- [ ] **Step 2: Build Phase-A descriptors from finalized roots**

Convert Stage 4A roots to descriptors only for participating Oregon-SMT domains, omit `ExecutionReceipts`, strictly sort and validate through primitive constructor. Stage 4B contributes no real EVM descriptor; synthetic EVM descriptor injection exists only in tests for scheme validation.

- [ ] **Step 3: Build canonical receipts and Phase-B journal**

Receipt keys are exactly:

```rust
fn fee_receipt_key(txid: Hash256) -> Vec<u8> { b"receipt/v1/fee/" + txid }
fn execution_receipt_key(txid: Hash256) -> Vec<u8> { b"receipt/v1/execution/" + txid }
```

Implement with explicit `Vec` assembly, read each key first, fail if present, then put the canonical fee receipt bytes and 259-byte execution receipt into an `ExecutionReceipts`-only journal and finalize committed.

- [ ] **Step 4: Return proposal only after both immutable finalizations succeed**

No callback/shared output exposes Phase A early. Any Phase-B error drops local Phase-A result, receipts and settlement proposal. This remains unpublished and persistence-neutral.

- [ ] **Step 5: Verify focused/vector tests and commit**

Run:
```bash
python3 scripts/generate_runtime_coordinator_vectors.py --check
cargo +1.85.0 test --locked -p oregon-primitives --test runtime_receipts --test runtime_receipt_vectors
cargo +1.85.0 test --locked -p oregon-execution coordinator::tests::proposal
```
Expected: PASS.

```bash
git add crates/oregon-execution
git commit -m "feat(stage4b): compose atomic execution receipt proposal"
```

---

### Task 9: Adversarial Test-Only Backends and Cross-Call Scenarios

**Files:**
- Modify: `crates/oregon-execution/src/coordinator/tests.rs`
- Optional create if test file becomes unwieldy: `crates/oregon-execution/src/coordinator/test_backends.rs` under `#[cfg(test)]` only.

**Interfaces:**
- Consumes: runtime backend/host traits and completed private coordinator.
- Produces compile-time test-only scripted backends; no production export.

- [ ] **Step 1: Add scripted backend vocabulary under test cfg**

```rust
enum ScriptOp {
    Read(Vec<u8>),
    Put(Vec<u8>, Vec<u8>),
    Emit(Vec<Hash256>, Vec<u8>),
    ChargeVm(u64),
    ChargeCommon(u64),
    Call(RuntimeCallSpecV1),
    Return(Vec<u8>),
    Revert(Vec<u8>),
    Trap(RuntimeTrapCodeV1),
    IgnoreAbortThenReturnSuccess,
}
```

The backend is deterministic and does not inspect wall clock/environment/filesystem.

- [ ] **Step 2: Add nested same-domain and cross-labelled EVM/WASM traces**

Cross-labelled EVM calls exercise context/meter/event/value semantics only; any EVM state read/write must deterministically fail. Include WASM -> EVM label -> WASM nested trace to prove shared meter/read-only/caller propagation without claiming real EVM compatibility.

- [ ] **Step 3: Add failure-propagation traces**

Cover child revert handled by parent, child trap handled by parent, ancestor top-level revert, resource exhaustion ignored by backend but forced by coordinator, and injected fatal journal/accounting error that no parent can convert to success.

- [ ] **Step 4: Run focused adversarial suite repeatedly**

Run:
```bash
cargo +1.85.0 test --locked -p oregon-execution coordinator::tests -- --nocapture
cargo +1.85.0 test --locked -p oregon-execution --all-targets
```
Expected: PASS deterministically on repeated runs.

- [ ] **Step 5: Commit**

```bash
git add crates/oregon-execution
git commit -m "test(stage4b): add adversarial runtime backends"
```

---

### Task 10: Runtime Coordinator Mutations and Dedicated x86_64/ARM CI

**Files:**
- Create: `scripts/verify_runtime_coordinator_mutations.py`
- Create: `.github/workflows/oregon-runtime-coordinator.yml`
- Modify: `.github/workflows/oregon-rust.yml`

**Interfaces:**
- Consumes: completed Stage 4B source/tests/vectors.
- Produces 16 compiled/killed mutation gates and dedicated architecture/vector workflow.

- [ ] **Step 1: Implement mutation runner with clean baseline/restoration checks**

Follow `verify_journal_mutations.py`: disposable checkout, source-hash capture, one mutation at a time, required named test failure, restore source, clean baseline after each mutation. The required mutations are exactly:

1. rollback refunds meter;
2. backend chooses cheaper resource domain;
3. read-only child clears parent restriction;
4. event survives reverted frame;
5. attached value survives reverted frame;
6. escrow reservation remains spendable;
7. execution-funded charge fails to reduce execution total;
8. trap maps to committed fee outcome;
9. resource exhaustion can be masked by backend success;
10. generic system-domain state write is exposed;
11. EVM effect accepted as Oregon SMT;
12. receipt state root is inserted into Phase-A effect list/self-reference;
13. Phase-A result escapes after Phase-B failure;
14. duplicate receipt finalization succeeds;
15. non-trapped receipt accepts nonzero trap code;
16. event/return overflow truncates instead of rejecting.

Each mutation must compile; compiler failure is not a killed mutation.

- [ ] **Step 2: Verify mutation runner RED controls**

Add an explicit negative-control mode that changes one expected runtime receipt root/byte in a disposable corpus and proves both the Python `--check` and Rust vector consumer reject it.

Run: `python3 scripts/verify_runtime_coordinator_mutations.py`
Expected before mutation targets are all wired: runner reports any surviving mutation and exits nonzero; fix tests/implementation rather than weakening the runner.

- [ ] **Step 3: Add dedicated workflow**

`.github/workflows/oregon-runtime-coordinator.yml` runs on `work/runtime-coordinator-v1-2026-09-06`, PRs to main and main pushes, matrix `ubuntu-24.04` / `ubuntu-24.04-arm`, pinned checkout, Rust 1.85.0, required design/plan presence, Python oracle `--check`, primitive vector tests, `oregon-runtime --all-targets`, focused coordinator tests.

- [ ] **Step 4: Extend Oregon Rust CI architecture and mutation gates**

Add work branch to push list. Required-contract scan includes:

```text
docs/superpowers/specs/2026-09-06-runtime-coordinator-v1-design.md
docs/superpowers/plans/2026-09-06-runtime-coordinator-v1.md
```

Add dependency scan proving `oregon-runtime` has no upward/state/VM dependencies and execution still has no VM/storage/chainstate/RPC dependency. Add `Runtime coordinator contracts` before full workspace test and `Runtime coordinator mutation gates` after inherited journal mutations.

- [ ] **Step 5: Run all local gate commands available and commit**

Run:
```bash
python3 scripts/generate_runtime_coordinator_vectors.py --check
python3 scripts/verify_runtime_coordinator_mutations.py
cargo +1.85.0 test --locked -p oregon-primitives --test runtime_receipts --test runtime_receipt_vectors
cargo +1.85.0 test --locked -p oregon-runtime --all-targets
cargo +1.85.0 test --locked -p oregon-execution --all-targets
```
Expected: PASS and all 16/16 mutations killed.

```bash
git add scripts/verify_runtime_coordinator_mutations.py .github/workflows/oregon-runtime-coordinator.yml .github/workflows/oregon-rust.yml
git commit -m "ci(stage4b): gate runtime coordinator invariants"
```

---

### Task 11: Exact-Head Verification, Checkpoint and Integration Stop

**Files:**
- Create after implementation-head CI: `docs/checkpoints/OREGON_RUNTIME_COORDINATOR_PROGRESS.md`
- Modify after implementation-head CI: `HANDOFF.md`
- Modify after implementation-head CI: `docs/superpowers/plans/2026-09-06-runtime-coordinator-v1.md` checkboxes/status only.
- Draft PR: `Stage 4B: runtime ABI and transaction coordinator foundation`.

**Interfaces:**
- Consumes: exact implementation source SHA/tree and CI evidence.
- Produces durable continuation evidence; does not merge to main.

- [ ] **Step 1: Run full exact implementation-head verification**

Run locally where supported:

```bash
cargo +1.85.0 test --locked --workspace --all-targets
python3 scripts/generate_execution_resource_vectors.py --check
python3 scripts/generate_fee_settlement_vectors.py --check
python3 scripts/generate_journal_vectors.py --check
python3 scripts/generate_runtime_coordinator_vectors.py --check
python3 scripts/verify_execution_address_mutations.py
python3 scripts/verify_execution_envelope_mutations.py
python3 scripts/verify_contract_state_mutations.py
python3 scripts/verify_execution_resource_mutations.py
python3 scripts/verify_fee_settlement_mutations.py
python3 scripts/verify_journal_mutations.py
python3 scripts/verify_runtime_coordinator_mutations.py
cargo +1.85.0 doc --locked --workspace --no-deps
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
```

If local environment cannot execute a gate, record the limitation and use the exact-head GitHub Actions run as authoritative; never claim a skipped local gate passed locally.

- [ ] **Step 2: Verify dedicated CI on exact implementation SHA**

Required evidence:
- `Oregon Rust CI` exact head: completed SUCCESS including full workspace, inherited mutation gates, runtime coordinator 16/16, docs, Format, Clippy.
- `Oregon Runtime Coordinator Vectors` exact head: x86_64 SUCCESS and ARM SUCCESS.
- No success claim from an ancestor SHA.

- [ ] **Step 3: Perform focused source review before checkpoint**

Review dependency direction, public API necessity, no system-domain host write, no EVM-as-Oregon-SMT path, no duplicated fee arithmetic, no meter reset/refund, no early Phase-A escape, and no production backend export. Search production source for forbidden placeholders/lint suppression/task-named modules and remove any finding.

- [ ] **Step 4: Commit checkpoint/HANDOFF/plan state together**

`OREGON_RUNTIME_COORDINATOR_PROGRESS.md` records base main, design/plan paths, implementation head/tree, exact CI run/job IDs, 16/16 mutation list, x86/ARM vector evidence, inactive/non-activation scope and remaining production VM/chainstate work. `HANDOFF.md` points to Stage 4B branch/PR and says implementation is checkpointed but not integrated/activated.

```bash
git add docs/checkpoints/OREGON_RUNTIME_COORDINATOR_PROGRESS.md HANDOFF.md docs/superpowers/plans/2026-09-06-runtime-coordinator-v1.md
git commit -m "docs(stage4b): checkpoint verified runtime coordinator"
```

- [ ] **Step 5: Verify the checkpoint successor exact head**

Wait for GitHub Actions on the checkpoint commit and require Oregon Rust CI + dedicated runtime coordinator x86/ARM workflow SUCCESS. Update PR body with both implementation-head and checkpoint-successor evidence. Do not create another repository commit solely to embed the successor run IDs if that would create an infinite self-invalidating verification cycle; PR body may record the external successor evidence.

- [ ] **Step 6: Stop before main integration**

PR remains isolated until the owner makes a separate explicit integration decision. Do not mark Stage 4B as production WASM/EVM, durable execution, protocol activation or completed Target 2. After any later merge, verify the exact new main head again before calling Stage 4B integrated.
