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

It owns:

- call request/context/result data;
- stable deterministic trap discriminants;
- VM-facing host trait signatures;
- event input framing and structural bounds;
- runtime ABI versioning.

It does not own fee policy, state persistence, resource budgets, monetary policy, journal frames or dispatch authority.

### 3.2 `oregon-execution`

`oregon-execution` owns:

- pre-execution composition;
- authoritative funding capability consumption;
- max-fee escrow lifecycle;
- the one Stage 3A `WeightMeter` instance;
- Stage 4A journal construction and frame lifecycle;
- execution-domain backend dispatch;
- cross-VM call-stack coordination;
- value-transfer accounting inside journal frames;
- event frame lifecycle;
- top-level outcome mapping;
- Stage 3B settlement reuse;
- execution receipt assembly; and
- unpublished transaction proposal composition.

VM adapters cannot obtain direct references to `WeightMeter`, `EscrowBookV1`, `ExecutionJournalV1` or raw `ExecutionAccounting`/`FeeState` system keys.

### 3.3 Lower owners remain authoritative

- `oregon-primitives` owns canonical execution receipt bytes and receipt/effect identifiers.
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

V1 Oregon-standard nested calls target only EVM or WASM typed execution addresses. Unknown/system targets fail deterministically. Domain-specific same-VM semantics such as EVM `DELEGATECALL` are not silently generalized into the cross-VM ABI. A later EVM adapter design may map its own same-VM semantics through a reviewed internal boundary while preserving this cross-VM contract.

### 4.3 Call context delivered to a backend

A backend receives an immutable context logically containing:

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

The coordinator constructs this context. A backend cannot rewrite it.

### 4.4 Call result

A normal backend result is exactly one of:

- `Success(return_data)`;
- `Revert(return_data)`; or
- `Trap(RuntimeTrapCodeV1)`.

Return data is structurally bounded before coordinator retention/copy to the caller. A backend cannot represent fatal state corruption as an ordinary revert.

### 4.5 Stable deterministic trap classes

V1 trap discriminants are `u16` and frozen as:

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

`0x0000` means no trap and is not a valid trapped result. Resource-meter exhaustion is **not** a catchable trap; it is a transaction-level outcome. State corruption, accounting invariant failure, commitment failure and coordinator invariant failure are fatal candidate errors, not traps.

## 5. Structural runtime ceilings

These V1 values are absolute inactive structural ceilings, not benchmark-backed production economic settings. Activation may select lower limits; raising them requires versioned review.

| Item | V1 ceiling |
| --- | ---: |
| Call depth including top level | 64 |
| Nested call input | 262,144 bytes |
| Return data per call | 262,144 bytes |
| Event data per event | 65,536 bytes |
| Event topics per event | 4 |
| Events per transaction | 256 |
| Aggregate retained event + return-data bytes | 2,097,152 bytes |

Event topics are fixed `Hash256` values. Event emitter identity is coordinator-derived from the current target; a backend may not forge another emitter.

Checks occur before coordinator-owned copies or frame growth. Limit failure does not truncate data and call it success.

## 6. Deterministic host surface

`RuntimeHostV1` is a capability-limited interface implemented by the coordinator for the active frame. V1 host categories are:

- scoped state read;
- scoped state put/delete;
- emit event;
- nested Oregon-standard call;
- charge current VM-native deterministic units;
- charge normalized common host work; and
- immutable context queries.

There is no filesystem, socket/network access, wall clock, process environment, OS randomness, arbitrary database access, raw RocksDB access, generic system-domain key access, fee mutation, receipt mutation, async-consumed mutation or `non_revertible_write` host function.

A VM-facing state operation is automatically scoped to the current target/domain. The backend supplies only its domain-owned local key/value bytes. It does not supply `CommitmentDomainId` and cannot redirect a storage write into `ExecutionAccounting`, `ExecutionReceipts`, `FeeState`, `AsyncOutbox` or `AsyncConsumed`.

Stage 4B production state support through the Stage 4A Oregon-SMT journal is initially only the WASM logical state domain. EVM uses a separately reserved commitment scheme and is not falsely routed through Oregon SMT. Stage 4B may exercise EVM-labelled adversarial backends for call/meter/event semantics, but no such backend is production EVM state support.

## 7. Host charging and the one transaction meter

One Stage 3A `WeightMeter` is constructed by the coordinator and lives for the complete transaction. It is not cloned per call. Rollback never decreases it. Exhaustion remains sticky and consumes the authorized maximum exactly as Stage 3A specifies.

Backends do not choose a `ResourceDomain` when reporting native units. `charge_vm_units(units)` is mapped by the coordinator from the current execution domain to the authoritative Stage 3A resource domain, preventing a backend from relabeling expensive EVM/WASM work as another domain.

Host operations use a versioned `HostChargeScheduleV1` of checked integer base and per-byte/per-item charges. Stage 4B freezes the algorithm and operation categories, but does not invent production coefficients before benchmark evidence.

Charging order:

1. validate structural lengths/counts without copying;
2. charge deterministic fixed/input-size host work;
3. perform the bounded operation;
4. for returned state bytes, charge the boundary-copy component after authoritative read size is known but before those bytes are copied into VM-visible return memory;
5. if any charge exhausts the meter, stop transaction execution immediately as `ResourceExhausted`.

The bounded authoritative state-read base work is never free merely because returned length is not known before lookup.

## 8. Coordinator pre-execution authority

The coordinator does not accept a caller-constructed `FundingCapabilityV1` as sufficient spending authority.

The trusted composition boundary supplies a production source validator that binds:

- chain id and current height;
- exact Oregon txid and domain;
- principal and effective fee payer;
- authorization result/commitment;
- domain-native replay state;
- current fee caps/base fee;
- exact authoritative payer source commitment and sequence; and
- sufficient available amount.

The validator then produces the Stage 3B capability used by the coordinator. Test validators/backends are compile-time test-only and must be visibly named as such. RPC or VM code cannot manufacture production funding authority.

The coordinator additionally verifies capability payer/source-kind/txid-context consistency before calling `EscrowBookV1::open`.

## 9. Root settlement frame and revertible execution frame

A Stage 4B transaction uses the Stage 4A journal deliberately as a two-level authority model:

1. the **root journal frame** is coordinator-owned settlement/accounting state and is never exposed to a VM;
2. the coordinator opens a **top-level execution child frame** before invoking the backend;
3. all contract storage, attached-value transfers and nested call effects live in that child and its descendants;
4. success commits the top-level child into the root;
5. top-level revert/trap/resource exhaustion reverts the top-level child while preserving coordinator-owned root settlement effects;
6. fee settlement/refund and final accounting are completed in the root;
7. the root is finalized only after the deterministic transaction outcome is known.

This makes fee effects survive contract revert without introducing a generic non-revertible host write.

## 10. Execution-funded escrow visibility and accounting

For an execution-funded payer, opening escrow must make `max_escrow` unavailable to contract execution before the top-level child starts.

The coordinator stages a root-frame debit of the payer's spendable `ExecutionAccounting` balance by `max_escrow`. The corresponding amount remains an unpublished in-memory escrow liability; `total_execution_balance` is not reduced merely by reservation because native reserve backing is still unchanged.

The child execution frame sees the root debit and therefore cannot spend escrowed value.

After execution:

- refund is added back to the then-current payer balance in the root;
- actual execution-funded charge remains deducted;
- `total_execution_balance` decreases by exactly the charged execution-funded fee;
- native-funded fees do not reduce execution balance/reserve accounting; and
- Stage 3B settlement arithmetic remains the sole price/charge/refund authority.

At the transaction proposal boundary no live escrow remains. The later block reserve transition consumes the accumulated execution-funded fee total under the existing Stage 3B reserve equation.

Any underflow, overflow, malformed accounting value or total-balance inconsistency is fatal to the candidate, not a catchable contract failure.

## 11. Revertible attached-value transfers

An Oregon-standard nested call may attach backed OREG value.

For a nonzero value:

1. coordinator validates caller/target are execution identities allowed for V1 call value;
2. read the caller and target balances through the current authoritative journal overlay;
3. reject insufficient spendable caller value as deterministic trap `0x0009`;
4. checked-debit caller and checked-credit target in the newly opened child call frame;
5. leave `total_execution_balance` unchanged because this is execution-to-execution movement; and
6. invoke the child backend only after the staged transfer succeeds.

Child success merges the transfer; child revert/trap discards it. A propagated top-level revert discards all such contract value transfers while fee settlement remains.

System/protocol identities cannot receive or originate generic call-value transfers in V1.

## 12. Call frames and frame-local effects

The coordinator, not a VM backend, owns begin/commit/revert.

Every nested call atomically opens:

- one Stage 4A journal child frame; and
- one frame-local effects record containing events and retained return-data accounting.

On child success both are merged into the parent. On child revert/trap both are discarded. The caller receives the bounded return result according to VM semantics. Meter consumption is never merged/reverted because it is transaction-global.

If a fatal candidate error occurs in any descendant, no parent may catch it and convert the transaction into success.

## 13. Top-level outcome mapping

Stage 4B has four canonical execution-receipt outcomes:

- `Committed`;
- `Reverted`;
- `Trapped`;
- `ResourceExhausted`.

Fee settlement maps them onto the existing Stage 3B fee outcomes:

- `Committed -> ExecutionOutcome::Committed`;
- `Reverted -> ExecutionOutcome::Reverted`;
- `Trapped -> ExecutionOutcome::Reverted`;
- `ResourceExhausted -> ExecutionOutcome::ResourceExhausted`.

Thus deterministic trap and explicit revert pay for the same consumed work; sticky resource exhaustion charges the full authorized meter maximum under existing Stage 3A/3B semantics.

Pre-escrow invalidity produces no execution receipt and no fee. Fatal candidate errors after escrow produce no publishable transaction proposal at all; any provisional local escrow/journal objects are dropped with the rejected candidate overlay.

## 14. Canonical events and event commitment

`oregon-primitives` owns canonical execution-event bytes for receipt commitment. V1 event bytes are:

```text
version:      u16 LE = 1
emitter:      33-byte ExecutionAddress
topic_count:  u8
topics:       topic_count * Hash256
data_len:     u32 LE
data:         data_len bytes
```

`topic_count <= 4`, `data_len <= 65,536`, and the transaction total event count/retained-byte limits apply before copies.

`event_id = H("OREGON/EXEC/EVENT/V1\0", event_bytes)`.

The ordered transaction event commitment is:

`events_root = H("OREGON/EXEC/EVENTS/V1\0", u32_le(event_count) || event_id_0 || ... || event_id_n)`.

Order is exact execution order after rollback filtering. Discarded-frame events do not appear.

## 15. First-phase state-effect commitment

The canonical execution receipt must commit the resulting execution/accounting effects without becoming circular with the receipt-state root that stores the receipt itself.

Stage 4B therefore uses **two unpublished journal finalizations**.

### Phase A — execution/settlement journal

The first journal contains contract/accounting effects but **not** `ExecutionReceipts` insertion. It finalizes locally and returns a Stage 4A result. Nothing is published.

`state_effect_root` is computed from that Phase-A result in numeric domain order:

```text
version: u16 LE = 1
count:   u16 LE
for each participating domain:
    domain_id: u16 LE
    old_root:  Hash256
    new_root:  Hash256
```

`state_effect_root = H("OREGON/EXEC/STATE-EFFECT/V1\0", canonical_bytes)`.

Because `ExecutionReceipts` is excluded from Phase A, the execution receipt does not hash a state root that depends on its own bytes.

### Phase B — receipt journal

Only after Phase A succeeds does the coordinator construct the canonical fee/execution receipts and stage them in a second unpublished Stage 4A journal configured for `ExecutionReceipts`.

V1 raw receipt keys are:

- `b"receipt/v1/fee/" || txid[32]` -> canonical `FeeSettlementReceiptV1` bytes;
- `b"receipt/v1/execution/" || txid[32]` -> canonical `ExecutionReceiptV1` bytes.

Existing values at either key are a fatal duplicate-finalization error.

If Phase B fails, the Phase-A bundle is discarded. The coordinator returns one combined unpublished `TransactionExecutionProposalV1` only after both phases succeed. The proposal carries both transition sets and must later be published atomically by the block/chainstate owner; Stage 4B itself does not persist either phase.

## 16. Canonical `ExecutionReceiptV1`

`oregon-primitives` owns a fixed-width V1 execution receipt with exactly these fields:

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

The encoded length is exactly **259 bytes**.

Outcome discriminants:

- `0x00` committed;
- `0x01` reverted;
- `0x02` trapped;
- `0x03` resource exhausted.

`trap_code` must be nonzero only for `Trapped`; it must be zero for all other outcomes.

`return_data_hash = H("OREGON/EXEC/RETURN/V1\0", top_level_return_data)` and `return_data_len` is the exact retained top-level length.

Stage 4B has no async emission API. Therefore `outbox_count` is exactly zero and `outbox_effect_root` is the canonical empty value:

`H("OREGON/EXEC/OUTBOX-EFFECT/V1\0", u32_le(0))`.

Stage 4C may later populate the same receipt fields from its separately reviewed canonical message ids without changing the V1 receipt layout.

`execution_receipt_id = H("OREGON/EXEC/RECEIPT/V1\0", receipt_bytes)`.

Construction validates the fee charged/payer/actual weight against the already-built Stage 3B fee settlement receipt; duplicated fields are not accepted as independent truth.

## 17. Backend dispatch and production boundaries

The coordinator owns a closed dispatch table for activated execution domains. Stage 4B implementation includes deterministic adversarial **test-only** backends that exercise:

- nested same-domain calls;
- cross-labelled EVM/WASM calls;
- success/revert/trap;
- attached-value rollback;
- shared-meter exhaustion;
- read-only violation;
- event rollback/order/bounds;
- return-data bounds; and
- fatal state/corruption propagation.

These test backends are not exported as production executors and must not be described as EVM/WASM compatibility.

A production WASM backend is a later slice using the same ABI and the Stage 4B scoped WASM state host. A production EVM backend requires its own explicit EVM state commitment/overlay adapter design and cannot be routed through the Stage 4A Oregon-SMT journal as a shortcut.

## 18. Async boundary

Stage 4B does not emit or consume async messages. No VM-visible async host API exists yet.

The receipt carries the fixed empty outbox effect commitment only so Stage 4C can populate the already-planned receipt field. Stage 4C remains responsible for canonical message bytes, ids, outbox keys, consumed keys, expiry, authorization, replay policy and delivery-failure semantics.

## 19. Fatal versus deterministic failures

The following are deterministic execution outcomes when caused by contract/backend behavior within validated state:

- explicit contract revert;
- stable runtime trap;
- insufficient attached call value;
- read-only write attempt;
- structural call/event/return limit violation; and
- full transaction resource exhaustion, with its distinct top-level outcome.

The following are fatal candidate errors and invalidate the unpublished transaction/block overlay:

- corrupt/missing authoritative state records where the lower owner requires them;
- accounting encoding/invariant violation;
- fee arithmetic/settlement invariant failure;
- duplicate settlement/receipt insertion;
- state commitment failure;
- coordinator/journal context mismatch;
- impossible frame/effect-stack mismatch; and
- any error the lower state owner classifies as corruption rather than normal absence.

Fatal errors cannot be caught by a contract or translated into a successful receipt.

## 20. Verification requirements

Implementation is test-first and must include distinguishing tests for at least:

### Runtime/call behavior

- derived caller/context cannot be forged by a backend;
- exact call-depth limit and one-over;
- nested success merges state/events;
- child revert/trap discards state/value/events but not weight;
- ancestor revert discards previously committed descendants;
- read-only child cannot write;
- invalid/system target traps deterministically;
- return/input/event exact limits and one-over;
- event order after rollback filtering.

### Metering

- one meter instance across all frames/domains;
- VM unit charge maps from current domain rather than backend-selected domain;
- host charge before state write/call/event copy;
- sticky exhaustion cannot be caught by child;
- exhaustion maps to max authorized weight and top-level `ResourceExhausted`.

### Escrow/accounting

- execution-funded max escrow is unavailable to child execution;
- successful contract writes plus refund produce `base - transfers - actual_charge` for payer;
- top-level revert restores contract transfers but still charges actual fee;
- execution-to-execution value transfer preserves total execution balance;
- execution-funded fee reduces total execution balance exactly by charge;
- native-funded fee leaves execution total unchanged;
- malformed/overflowing accounting is fatal;
- same capability/escrow cannot settle twice.

### Two-phase proposal/receipt

- Phase A succeeds and Phase B fails -> no combined proposal escapes;
- state effect commitment excludes receipt self-dependency;
- duplicate receipt keys fail closed;
- fee receipt id/charge/payer/weight must match execution receipt;
- canonical event/state/return/empty-outbox roots have independent golden vectors;
- success/revert/trap/resource-exhausted receipt bytes have byte-exact golden vectors.

### Adversarial mutations

At minimum mutation gates must kill:

- child rollback refunds meter;
- backend chooses a cheaper resource domain;
- event survives reverted frame;
- attached value survives reverted frame;
- escrow reservation hidden from contract balance view;
- execution-funded fee fails to reduce execution total;
- trap incorrectly maps to committed fee outcome;
- resource exhaustion made catchable;
- generic system-domain write exposed to backend;
- receipt stores a self-referential receipt-state root;
- Phase-A partial result escapes when Phase B fails;
- duplicate receipt finalization accepted;
- trap code accepted on a non-trapped receipt; and
- event/return structural limit truncates instead of failing.

Independent reference vectors must not call Rust production helpers for expected roots/hashes/bytes. New canonical vectors run on x86_64 and ARM.

Full acceptance also requires workspace tests, architecture scan, rustdoc/docs, rustfmt, Clippy with warnings denied, relevant inherited mutation gates, exact-head CI and a new Stage 4B checkpoint.

## 21. Non-goals and activation boundary

Stage 4B completion must not be reported as:

- production WASM execution;
- production EVM execution;
- Ethereum RPC compatibility;
- universal-envelope activation;
- async messaging;
- durable execution persistence;
- active execution fee-aware coinbase settlement;
- active aggregate block state commitment;
- completed hybrid-state chainstate publication; or
- completed Target 2.

Those remain later gated slices.

Stage 4B output is an **inactive, unpublished transaction execution proposal**. Only a later chainstate/block-integration design may compose native UTXO/reserve transitions, Phase-A execution transitions, Phase-B receipt transitions, block fee totals, commitments and undo into one WAL+sync durable accepted operation.

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
