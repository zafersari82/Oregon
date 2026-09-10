# Oregon Stage 4B Runtime ABI and Transaction Coordinator V1

**Status:** written from the owner-approved Stage 4B design direction; implementation remains gated on written-spec review.

**Date:** 2026-09-06 UTC

**Base main:** `dd7cdcb566273c39d5a38cf0c0036058b08a7d89`

**Base tree:** `239219eb08157276ce468dd3dd037e129bec0d58`

**Parent contracts:**
- `AGENTS.md`
- `docs/architecture/OREGON_ENGINEERING_CONSTITUTION.md`
- `docs/architecture/OREGON_PLATFORM_ARCHITECTURE_CONTRACT.md`
- `docs/superpowers/specs/2026-09-05-execution-architecture-design.md`
- `docs/superpowers/specs/2026-09-05-execution-envelope-wire-v1.md`
- `docs/superpowers/specs/2026-09-06-execution-resources-fees-v1.md`
- `docs/superpowers/specs/2026-09-06-fee-settlement-reserve-v1.md`
- `docs/superpowers/specs/2026-09-06-runtime-journal-async-v1-design.md`

## 1. Authority, reprioritization and scope

Stage 4A is integrated into `main` as an inactive bounded execution-journal foundation. The queued trust roadmap names formal reserve proof work as the default next task unless the owner reprioritizes. The owner explicitly reprioritized toward **Target 2: the full platform architecture promised by the contract (Multi-VM, hybrid state, EVM/WASM, 1:1 reserve)** and approved proceeding with the Stage 4B VM-neutral runtime/coordinator approach.

This design therefore specifies Stage 4B only. It does not change the accepted trust roadmap; it changes execution order under the roadmap's own owner-reprioritization rule.

Stage 4B delivers:

1. a new dependency-light `oregon-runtime` crate containing the deterministic V1 host/cross-VM ABI;
2. one `oregon-execution` transaction coordinator that owns the fee/meter/journal/call-frame/receipt lifecycle;
3. bounded frame-local events and return data;
4. authoritative fee settlement reuse rather than copied arithmetic;
5. typed execution-accounting value transfers that remain journaled and revertible;
6. one canonical Oregon execution receipt and deterministic effect commitments; and
7. adversarial deterministic test backends exercising the ABI without pretending to be production EVM or WASM engines.

Stage 4B does **not** activate the universal envelope, current block/header bytes, current mempool, RPC, storage schema, EVM, WASM, async delivery, chainstate publication or consensus execution. No production VM engine is selected here.

## 2. Selected approach and rejected alternatives

Three approaches were reviewed:

- **Selected: VM-neutral core first.** Freeze and test one runtime ABI and one transaction coordinator before binding a real VM engine.
- **Rejected: WASM-first runtime design.** Faster visible contract execution but risks permanently shaping the shared ABI around one VM and forcing redesign when EVM arrives.
- **Rejected: EVM + WASM + coordinator in one slice.** Too large an audit surface: VM semantics, fee rollback, cross-VM behavior, state schemes and engine-specific faults would become entangled in one acceptance boundary.

The selected path minimizes architecture rework and preserves the parent contract's rule that EVM and WASM are adapters behind one Oregon execution truth.

## 3. Ownership and dependency direction

### 3.1 `oregon-runtime`

`oregon-runtime` owns only deterministic V1 runtime/call ABI types and traits. It may depend on `oregon-primitives` for canonical typed addresses, execution domains and hashes. It must not depend on `oregon-execution`, contract state, consensus, UTXO, storage, chainstate, networking, node, RPC or a VM implementation.

It owns call request/context/result data, stable deterministic trap discriminants, VM-facing host trait signatures, event input framing, structural bounds and runtime ABI versioning. It does not own fee policy, state persistence, resource budgets, monetary policy, journal frames or dispatch authority.

### 3.2 `oregon-execution`

`oregon-execution` owns pre-execution composition, authenticated funding capability consumption, max-fee escrow, the one Stage 3A `WeightMeter`, Stage 4A journal/frame lifecycle, backend dispatch, cross-VM call-stack coordination, value-transfer accounting, event frame lifecycle, top-level outcome mapping, Stage 3B settlement reuse, receipt assembly and the unpublished transaction proposal.

VM adapters cannot obtain direct references to `WeightMeter`, `EscrowBookV1`, `ExecutionJournalV1` or raw `ExecutionAccounting`/`FeeState` system keys.

### 3.3 Lower owners remain authoritative

- `oregon-primitives` owns canonical execution receipt/event/effect bytes and identifiers.
- `oregon-contract-state` owns accounting key encoding, logical state reads/write sets and Oregon SMT transitions.
- `oregon-utxo` remains native funding/reserve authority.
- `oregon-consensus` remains base-fee/block-budget/activation authority.
- `oregon-storage` and `oregon-chainstate` remain physical durability and accepted publication authorities.

No upward dependency is introduced into those crates merely to support Stage 4B.

## 4. Runtime ABI V1

### 4.1 ABI version

The runtime ABI version is exactly `1`. Unknown versions fail closed. ABI versioning is independent from envelope version, VM revision and state commitment scheme version.

### 4.2 VM-facing nested call request

The VM-facing request is logically:

```text
RuntimeCallSpecV1 {
    target: ExecutionAddress,
    value: u64,
    read_only: bool,
    input: bounded bytes,
}
```

The caller does **not** supply caller identity, chain id, txid, height, call depth, current execution domain, principal, payer or journal frame identity. The coordinator derives those from the current trusted frame.

V1 Oregon-standard nested calls target only `ExecutionAddressKind::Evm` or `ExecutionAddressKind::Wasm`. `Oregon`, `System`, unknown and domain-incompatible targets fail deterministically. Domain-specific same-VM semantics such as EVM `DELEGATECALL` are not silently generalized into the cross-VM ABI.

Read-only authority is monotonic down the stack. The child's effective flag is:

`effective_read_only = parent_read_only || requested_read_only`.

A child cannot clear a read-only restriction by requesting `false`.

### 4.3 Call context delivered to a backend

A backend receives immutable coordinator-derived context:

```text
RuntimeCallContextV1 {
    chain_id: u64,
    height: u64,
    parent_block_hash: Hash256,
    txid: Hash256,
    principal: ExecutionAddress,
    caller: ExecutionAddress,
    target: ExecutionAddress,
    execution_domain: ExecutionDomain,
    depth: u16,
    read_only: bool,
    transferred_value: u64,
}
```

### 4.4 Call result

A normal backend result is exactly one of:

- `Success(return_data)`;
- `Revert(return_data)`; or
- `Trap(RuntimeTrapCodeV1)`.

Return data is bounded before coordinator retention/copy to the caller. A backend cannot represent fatal state corruption as an ordinary revert.

### 4.5 Stable deterministic trap classes

V1 `u16` trap discriminants are:

- `0x0001` backend deterministic trap;
- `0x0002` invalid/unsupported call target;
- `0x0003` state access denied;
- `0x0004` write attempted in read-only call;
- `0x0005` call-depth limit exceeded;
- `0x0006` call-input limit exceeded;
- `0x0007` return-data limit exceeded;
- `0x0008` event structural/aggregate limit exceeded;
- `0x0009` attached-value transfer failed;
- `0x000a` unsupported deterministic runtime operation;
- `0x000b` invalid deterministic host input.

`0x0000` means no trap. Resource-meter exhaustion is **not** a catchable trap; it is a transaction-level outcome. State corruption, accounting invariant failure, commitment failure and coordinator invariant failure are fatal candidate errors.

## 5. Structural runtime ceilings

These are absolute inactive V1 structural ceilings, not benchmark-backed production economic settings. Activation may select lower limits; raising them requires versioned review.

| Item | V1 ceiling |
| --- | ---: |
| Call depth including top level | 64 |
| Nested call input | 262,144 bytes |
| Return data per call | 262,144 bytes |
| Event data per event | 65,536 bytes |
| Event topics per event | 4 |
| Events per transaction | 256 |
| Aggregate live retained event + return-data bytes | 2,097,152 bytes |

Stage 4A's monotonic total-frame ceiling of 4,096 remains the transaction-wide upper bound on created journal frames. The coordinator does not add an unbounded second call-frame counter.

Event topics are fixed `Hash256` values. Event emitter identity is coordinator-derived from the current target. Checks occur before coordinator-owned copies or frame growth. Limit failure does not truncate data and call it success.

The aggregate retained-byte budget counts bytes currently retained by live effect frames plus committed transaction events/top-level return data. Discarding a frame releases its retained-byte allocation; the transaction meter still retains the charged work, and Stage 4A's monotonic total-frame count prevents free sibling-frame churn.

## 6. Deterministic host surface

`RuntimeHostV1` is a capability-limited interface implemented by the coordinator for the active frame. V1 host categories are scoped state read, scoped state put/delete, emit event, nested Oregon-standard call, charge current VM-native deterministic units, charge normalized common host work and immutable context queries.

There is no filesystem, socket/network access, wall clock, process environment, OS randomness, arbitrary database access, raw RocksDB access, generic system-domain key access, fee mutation, receipt mutation, async-consumed mutation or `non_revertible_write` host function.

A VM-facing state operation is scoped to the current target/domain. The backend supplies only its domain-owned local key/value bytes. It does not supply a `CommitmentDomainId` and cannot redirect a storage write into `ExecutionAccounting`, `ExecutionReceipts`, `FeeState`, `AsyncOutbox` or `AsyncConsumed`.

Stage 4B production state support through the Stage 4A Oregon-SMT journal is initially only the WASM logical state domain. EVM uses a separately reserved commitment scheme and is not falsely routed through Oregon SMT. Stage 4B may exercise EVM-labelled adversarial backends for call/meter/event semantics, but no such backend is production EVM state support.

## 7. Host charging and the one transaction meter

One Stage 3A `WeightMeter` lives for the complete transaction. It is not cloned per call. Rollback never decreases it. Exhaustion remains sticky and consumes the authorized maximum exactly as Stage 3A specifies.

Backends do not choose a `ResourceDomain`. `charge_vm_units(units)` is mapped by the coordinator from the current execution domain, preventing a backend from relabeling expensive work as another domain.

`HostChargeScheduleV1` uses checked integer base and per-byte/per-item coefficients for the fixed V1 operation categories. Stage 4B freezes the algorithm and categories but not production coefficient values before benchmark evidence. Test schedules use explicit synthetic values and cannot be described as activation constants.

Charging order:

1. validate structural lengths/counts without copying;
2. charge fixed/input-size host work;
3. perform the bounded operation;
4. for returned state bytes, charge the boundary-copy component after authoritative read size is known but before those bytes are copied into VM-visible return memory;
5. any exhaustion stops transaction execution immediately as `ResourceExhausted`.

The authoritative state-read base work is never free merely because returned length is unknown before lookup.

## 8. Coordinator pre-execution authority

The coordinator does not accept a caller-constructed `FundingCapabilityV1` as sufficient spending authority.

The trusted composition boundary supplies a production source validator binding chain/height, exact txid/domain, principal/effective payer, authorization commitment, domain replay state, fee caps/base fee, authoritative payer source commitment/sequence and sufficient available amount. Only after that validation is the Stage 3B capability supplied to the coordinator.

Test validators/backends are compile-time test-only and visibly named as such. RPC or VM code cannot manufacture production funding authority. The coordinator additionally checks capability payer/source-kind/context consistency before `EscrowBookV1::open`.

## 9. Root settlement frame and revertible execution frame

A Stage 4B transaction uses Stage 4A's journal as a two-level authority model:

1. the **root frame** is coordinator-owned settlement/accounting state and is never VM-visible;
2. the coordinator opens a **top-level execution child frame** before backend invocation;
3. contract storage, attached-value transfers and nested call effects live in that child/descendants;
4. success commits the top-level child into the root;
5. top-level revert/trap/resource exhaustion reverts the child while preserving root settlement effects;
6. fee settlement/refund and final accounting complete in the root;
7. the root finalizes only after deterministic outcome is known.

This makes fees survive contract revert without a generic non-revertible host write.

## 10. Execution-funded escrow visibility and accounting

For an execution-funded payer, opening escrow makes `max_escrow` unavailable before the top-level child starts.

The coordinator stages a root-frame debit of the payer's spendable `ExecutionAccounting` balance by `max_escrow`. That amount remains an unpublished in-memory escrow liability; `total_execution_balance` is not reduced merely by reservation because native reserve backing is still unchanged.

The execution child sees the root debit and therefore cannot spend escrowed value.

After execution:

- refund is added to the then-current payer balance in the root;
- actual execution-funded charge remains deducted;
- `total_execution_balance` decreases by exactly the charged execution-funded fee;
- native-funded fees do not reduce execution balance/reserve accounting; and
- Stage 3B remains the only price/charge/refund arithmetic owner.

At proposal completion no live escrow remains. The later block reserve transition consumes accumulated execution-funded fee totals under the existing Stage 3B reserve equation.

Any underflow, overflow, malformed accounting value or total-balance inconsistency is fatal.

## 11. Revertible attached-value transfers

An Oregon-standard nested call may attach backed OREG value.

For nonzero value, **both caller and target must be `ExecutionAddressKind::Evm` or `ExecutionAddressKind::Wasm`**. `Oregon` and `System` kinds cannot originate or receive generic V1 call value.

The coordinator:

1. validates the exact allowed kinds;
2. reads caller/target balances through the current journal overlay;
3. treats insufficient spendable caller value as deterministic trap `0x0009`;
4. checked-debits caller and checked-credits target in the newly opened child call frame;
5. leaves `total_execution_balance` unchanged; and
6. invokes the child backend only after the staged transfer succeeds.

Child success merges the transfer; child revert/trap discards it. A propagated top-level revert discards all contract value transfers while fee settlement remains.

## 12. Call frames and frame-local effects

The coordinator owns begin/commit/revert. Every nested call atomically opens one Stage 4A journal child frame and one frame-local effects record containing events and retained return-data accounting.

On child success both merge into the parent. On child revert/trap both are discarded. Meter consumption is transaction-global and never merged/reverted. Fatal descendant errors cannot be caught and converted to success.

## 13. Top-level outcome mapping

Canonical execution receipt outcomes are `Committed`, `Reverted`, `Trapped`, `ResourceExhausted`.

Fee settlement maps them to the existing Stage 3B outcomes:

- `Committed -> ExecutionOutcome::Committed`;
- `Reverted -> ExecutionOutcome::Reverted`;
- `Trapped -> ExecutionOutcome::Reverted`;
- `ResourceExhausted -> ExecutionOutcome::ResourceExhausted`.

Thus trap and explicit revert pay consumed work; sticky exhaustion charges the full authorized maximum. Pre-escrow invalidity produces no executed receipt/fee. Fatal post-escrow candidate errors produce no publishable proposal.

## 14. Canonical events and event commitment

`oregon-primitives` owns canonical execution-event bytes:

```text
version:      u16 LE = 1
emitter:      33-byte ExecutionAddress
topic_count:  u8
topics:       topic_count * Hash256
data_len:     u32 LE
data:         data_len bytes
```

`topic_count <= 4`, `data_len <= 65,536`, and transaction aggregate limits apply before copies.

`event_id = H("OREGON/EXEC/EVENT/V1\0", event_bytes)`.

`events_root = H("OREGON/EXEC/EVENTS/V1\0", u32_le(event_count) || event_id_0 || ... || event_id_n)`.

Order is exact execution order after rollback filtering. Discarded-frame events do not appear.

## 15. State-effect commitment without receipt self-reference

The execution receipt must commit execution/accounting effects without hashing a receipt-state root that depends on the receipt itself.

Stage 4B therefore uses two unpublished phases.

### Phase A — execution/settlement effects

Phase A contains contract/accounting effects but no `ExecutionReceipts` insertion. Oregon-SMT effects are finalized through Stage 4A. The resulting effect descriptors are canonicalized together with any future scheme-specific effect descriptor supplied by a separately approved backend adapter.

A canonical `StateEffectDescriptorV1` is:

```text
domain_id:  u16 LE
scheme_id:  u16 LE
old_root:   Hash256
new_root:   Hash256
```

Descriptors are strictly ordered by numeric `domain_id`, duplicates are rejected, and `(domain_id, scheme_id)` must be protocol-approved. Stage 4A Oregon-SMT results use `CommitmentSchemeId::OregonSmtV1`. A future real EVM adapter may contribute `CommitmentDomainId::Evm + CommitmentSchemeId::EvmCommitmentV1` without changing the receipt format or pretending EVM uses Oregon SMT.

The canonical effect preimage is:

```text
version: u16 LE = 1
count:   u16 LE
repeated StateEffectDescriptorV1
```

`ExecutionReceipts` is forbidden in this Phase-A descriptor list.

`state_effect_root = H("OREGON/EXEC/STATE-EFFECT/V1\0", canonical_bytes)`.

### Phase B — receipt state

Only after Phase A succeeds does the coordinator construct fee/execution receipts and stage them in a second unpublished Stage 4A journal configured for `ExecutionReceipts`.

V1 raw keys are:

- `b"receipt/v1/fee/" || txid[32]` -> canonical `FeeSettlementReceiptV1` bytes;
- `b"receipt/v1/execution/" || txid[32]` -> canonical `ExecutionReceiptV1` bytes.

Existing values at either key are fatal duplicate-finalization errors.

If Phase B fails, Phase A is discarded. One combined unpublished `TransactionExecutionProposalV1` escapes only after both phases succeed. Later chainstate publication must persist both atomically; Stage 4B persists neither.

## 16. Canonical `ExecutionReceiptV1`

`oregon-primitives` owns a fixed-width V1 execution receipt:

```text
version:                    u16 LE = 1
txid:                       Hash256
execution_domain:           u8
outcome:                    u8
trap_code:                  u16 LE
fee_payer:                  33-byte ExecutionAddress
actual_weight:              u64 LE
fee_charged:                u64 LE
fee_settlement_receipt_id:  Hash256
state_effect_root:           Hash256
events_root:                Hash256
event_count:                u32 LE
return_data_hash:            Hash256
return_data_len:             u32 LE
outbox_effect_root:          Hash256
outbox_count:                u32 LE
```

Encoded length is exactly **259 bytes**.

Outcome discriminants:

- `0x00` committed;
- `0x01` reverted;
- `0x02` trapped;
- `0x03` resource exhausted.

`trap_code` is nonzero only for `Trapped` and zero otherwise.

`return_data_hash = H("OREGON/EXEC/RETURN/V1\0", top_level_return_data)` with exact bounded `return_data_len`.

Stage 4B has no async emission API. Therefore `outbox_count = 0` and:

`outbox_effect_root = H("OREGON/EXEC/OUTBOX-EFFECT/V1\0", u32_le(0))`.

Stage 4C may populate these already-reserved fields from separately reviewed canonical message ids without changing the V1 receipt layout.

`execution_receipt_id = H("OREGON/EXEC/RECEIPT/V1\0", receipt_bytes)`.

Construction cross-checks payer, actual weight and charged amount against the built Stage 3B fee settlement receipt; duplicated fields are not independent truth.

## 17. Backend dispatch and production boundaries

The coordinator owns a closed dispatch table for activated domains. Stage 4B implementation includes deterministic adversarial **test-only** backends exercising nested same-domain calls, cross-labelled EVM/WASM calls, success/revert/trap, value rollback, shared-meter exhaustion, read-only propagation, event rollback/order/bounds, return-data bounds and fatal propagation.

They are not exported as production executors and are not EVM/WASM compatibility claims.

A production WASM backend is a later slice using this ABI and scoped WASM state host. A production EVM backend requires an explicit EVM state commitment/overlay adapter and cannot be routed through Stage 4A Oregon SMT.

## 18. Async boundary

Stage 4B does not emit or consume async messages and exposes no VM-visible async host API. The receipt carries only the canonical empty outbox effect. Stage 4C remains responsible for message bytes/ids, outbox/consumed keys, sequence, expiry, authorization, replay and delivery-failure semantics.

## 19. Fatal versus deterministic failures

Deterministic execution outcomes include explicit contract revert, stable runtime trap, insufficient attached call value, read-only write attempt, structural call/event/return limit violation and resource exhaustion with its distinct top-level outcome.

Fatal candidate errors include corrupt/missing authoritative records where the lower owner requires them, accounting encoding/invariant failure, fee invariant failure, duplicate settlement/receipt insertion, state commitment failure, coordinator/journal context mismatch, impossible frame/effect-stack mismatch and lower-state corruption errors.

Fatal errors cannot be caught by a contract or translated into success.

## 20. Verification requirements

Implementation is test-first and must distinguish at least:

### Runtime/calls

- caller/context cannot be forged by a backend;
- exact depth and one-over;
- parent read-only cannot be cleared by child;
- nested success merges state/events;
- child revert/trap discards state/value/events but not weight;
- ancestor revert discards committed descendants;
- invalid/Oregon/System target traps;
- input/return/event exact limits and one-over;
- event order after rollback filtering.

### Metering

- one meter across frames/domains;
- backend cannot choose cheaper resource domain;
- host work charged at required boundary;
- sticky exhaustion cannot be caught;
- exhaustion maps to max authorized weight and `ResourceExhausted`.

### Escrow/accounting

- max escrow unavailable to execution child;
- success + refund gives correct payer balance after transfers and actual charge;
- top-level revert restores contract transfers but charges fee;
- execution-to-execution value preserves total execution balance;
- execution-funded fee reduces total by charge;
- native-funded fee leaves execution total unchanged;
- malformed/overflowing accounting is fatal;
- same capability/escrow cannot settle twice.

### Two-phase receipt proposal

- Phase A success + Phase B failure leaks no proposal;
- receipt self-dependency is impossible;
- Oregon-SMT descriptors carry `OregonSmtV1`;
- a synthetic EVM-labelled descriptor cannot masquerade as Oregon SMT;
- duplicate/noncanonical effect descriptors fail;
- duplicate receipt keys fail;
- fee receipt id/charge/payer/weight match execution receipt;
- event/state/return/empty-outbox roots have independent golden vectors;
- all four receipt outcomes have byte-exact 259-byte vectors.

### Required mutation gates

At minimum kill mutations where:

- child rollback refunds meter;
- backend chooses cheaper domain;
- read-only child clears parent restriction;
- event survives reverted frame;
- attached value survives reverted frame;
- escrow reservation is hidden from contract balance view;
- execution fee does not reduce execution total;
- trap maps to committed fee outcome;
- resource exhaustion is catchable;
- generic system-domain write is exposed;
- EVM effect is mislabeled Oregon SMT;
- receipt stores a self-referential receipt-state root;
- Phase-A partial result escapes after Phase-B failure;
- duplicate receipt finalization succeeds;
- trap code is accepted on non-trapped receipt; or
- event/return over-limit data is truncated instead of rejected.

Independent vectors must not call Rust production helpers for expected bytes/hashes. New canonical vectors run on x86_64 and ARM.

Acceptance also requires workspace tests, architecture scan, rustdoc/docs, rustfmt, warnings-denied Clippy, inherited mutation gates, exact-head CI and a Stage 4B checkpoint.

## 21. Non-goals and activation boundary

Stage 4B completion is not production WASM, production EVM, Ethereum RPC, universal-envelope activation, async messaging, durable execution persistence, active execution-fee coinbase settlement, active aggregate block commitment, completed hybrid-state chainstate publication or completed Target 2.

Stage 4B returns an **inactive, unpublished transaction execution proposal**. Only a later chainstate/block-integration design may compose native UTXO/reserve transitions, Phase-A execution transitions, Phase-B receipt transitions, block fee totals, commitments and undo into one WAL+sync durable operation.

## 22. Implementation decomposition after written-spec approval

The implementation plan should preserve this order:

1. add byte-exact primitive receipt/event/effect commitments with expected-red vectors;
2. add `oregon-runtime` ABI types/traits and structural tests;
3. add coordinator frame/effect-stack foundation over Stage 4A;
4. add shared-meter/host charging composition;
5. add authenticated pre-execution/escrow root reservation composition;
6. add typed accounting/value-transfer composition;
7. add top-level outcome/settlement mapping;
8. add Phase-A/Phase-B receipt proposal composition;
9. add adversarial test-only backends and cross-call scenarios;
10. add independent vectors and mutation gates;
11. run exact-head full CI/security review;
12. write Stage 4B checkpoint; and
13. stop for a separate owner integration decision before `main`.

No step may silently add a production VM engine or protocol activation merely because the coordinator is ready for one.
