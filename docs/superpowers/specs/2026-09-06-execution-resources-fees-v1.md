# Oregon Execution Resources and Fees V1

Date: 2026-09-06 UTC.
Status: selected under the owner's current delegation to continue Oregon without repeated low-level approval prompts. Inactive implementation design; no activation or main integration.
Base: `7cfac99c40de96afb79d222cd5ade9f3a537215a`, Stage 2 PR #14.

## 1. Authority and scope

The Engineering Constitution, Platform Architecture Contract and Execution Architecture V1 remain normative. This document refines Execution Architecture §§9–12 and Stage 3 without amending frozen M0–M6 behavior. The owner has authorized continued implementation and repository recording. This selection does not permit changing any frozen architecture decision.

Stage 3 is decomposed into two independently reviewable deliveries:

- **3A, implemented by the accompanying plan:** inactive deterministic resource conversion/metering, parent-only base-fee arithmetic and bounded block-weight accumulation.
- **3B, next versioned implementation design:** authoritative fee funding/escrow/settlement, fee-state bytes/commitments and authenticated reserve/accounting transitions. The binding equations and integration requirements in §8 constrain that design; this delivery does not pretend aggregate arithmetic alone proves UTXO backing.

No VM, envelope activation, new UTXO locking program, storage schema, wallet, RPC, mempool, issuance, header or transaction bytes change here. There are no production activation constants. Synthetic vectors are examples, never a network profile.

## 2. Owners and dependencies

| Rule | Authoritative owner | Stage 3A surface |
| --- | --- | --- |
| Parent utilization to next base fee; block/transaction limits | `oregon-consensus` | `execution_resources` module |
| VM/native units to normalized consumption; transaction budget | `oregon-execution` | new storage-independent `weight` module |
| Native/EVM/WASM deterministic work quantities | respective native/VM owner | future callers; no interpreters here |
| Canonical envelope fee caps and serialized identifiers | `oregon-primitives` | existing `FeeCaps`, unchanged |
| Fee funding, escrow, settlement and rollback coordination | `oregon-execution` | Stage 3B / runtime composition |
| Protocol reserve outpoints and native spend validity | `oregon-utxo` | Stage 3B design; no alternate ledger |
| Logical fee/accounting state and proofs | `oregon-contract-state` | Stage 2 SMT reused by Stage 3B |
| Physical state/durable block publication | storage / chainstate | later atomic integration |

Consensus does not depend on execution. The execution crate has no Oregon production dependencies in 3A: it owns integer metering only and may reuse the workspace's existing `thiserror` dependency for errors. It will consume lower-level consensus interfaces when the actual execution pipeline needs them. It must never depend on storage representation, networking, RPC or VM implementation crates. Domain adapters supply metered quantities; they never set prices or reset the transaction meter.

## 3. Selected approaches

Per-VM fee markets contradict the shared-market contract and are excluded. A single hard-coded native-unit scale would prematurely freeze unbenchmarked VM parameters. V1 therefore selects a fixed-size three-domain rational schedule, one shared normalized meter, and a separately owned consensus fee update. This preserves exact integer conversion without requiring EVM/WASM implementation choices today.

Charging each chunk with independent rounding makes the result depend on callback/chunk boundaries. V1 instead charges changes in the rounded **cumulative** quantity per resource domain. Integer-only coefficients are a valid special case but are not forced before benchmarks.

No-burn fee markets do not make producer self-filling economically expensive in the same way as fee burning. A deterministic rate bound limits the speed of fee movement; it does not prove manipulation resistance. Explicit producer-sequence vectors, a fee ceiling, and calibration evidence are required before activation.

## 4. Integer units and checked configuration

Normalized weights, native unit counts and fee prices use `u64`. Intermediate products use `u128`. Fee prices are integer OREG base units per normalized weight, using the existing `MAX_SUPPLY_BASE_UNITS` upper envelope. No floating point, machine-word-dependent arithmetic, wall clock or nondeterministic iteration is permitted.

`WeightRatio::new(numerator, denominator)` accepts only positive `u64` components. For a cumulative unit count `u`, conversion is `ceil(u * numerator / denominator)`; zero units cost zero. Compute quotient plus a nonzero-remainder bit, not `product + denominator - 1`. Reject a result above `u64::MAX` as `WeightOverflow` from this pure conversion function.

`MeterScheduleV1::new(version, native, evm, wasm)` accepts only version 1. Each ratio has already passed its constructor. Domain identifiers are closed `ResourceDomain::{Native,Evm,Wasm}`; there are no attacker-sized collections. Native units and WASM units refer to the separately versioned native/WASM work schedule. EVM units are gas consumption from the activated EVM revision, before any mechanism that refunds already consumed Oregon normalized weight. Detailed adapter mapping/refund compatibility requires its own vectors before those adapters activate.

`FeeParametersV1::new(version, target_weight, max_transaction_weight, adjustment_denominator, min_base_fee, max_base_fee)` requires:

- version exactly 1;
- target weight nonzero and `2 * target_weight` representable as `u64`;
- transaction maximum in `1..=2 * target_weight`;
- adjustment denominator at least 2;
- `1 <= min_base_fee <= max_base_fee <= MAX_SUPPLY_BASE_UNITS`.

The block maximum is exactly twice the target. These objects have private fields and getters. They are immutable validated configurations, not runtime settings or evidence of activation. Unknown versions and invalid configurations fail closed with distinct descriptive errors.

## 5. Shared transaction metering

`WeightMeter::new(schedule, max_weight, intrinsic_weight)` requires a nonzero budget and `intrinsic_weight <= max_weight`. Intrinsic weight is computed by the future authoritative execution validator for bytes, decoding, authorization and common processing; this library neither authenticates inputs nor invents its cost constants. An intrinsic weight of zero is permitted in this arithmetic library; activation must specify positive common processing costs.

The meter contains three fixed cumulative counters, the schedule, total consumed weight and an exhaustion flag. It is not `Clone` or `Copy`, has no reset/refund API, and exposes only `consumed`, `remaining`, `max_weight`, and `is_exhausted` views.

For `charge(domain, added_units)`:

1. An exhausted meter returns `WeightExhausted`, including for a zero charge.
2. Checked-add `added_units` to the domain's cumulative `u64` counter.
3. Obtain the domain's new cumulative converted weight, then subtract its old cumulative converted weight.
4. If the counter overflows, conversion exceeds `u64`, or the increment exceeds remaining normalized budget: mark exhausted, consume the full transaction maximum, and return `WeightExhausted`. No partially updated counters are exposed. The `u64` cumulative native-unit ceiling is an additional explicit safety bound.
5. Otherwise update the counter and consumed weight exactly once.

`charge_common(weight)` directly charges additional already-normalized common/host work through the same budget check. At exactly the maximum, a charge succeeds; remaining is zero, but the exhaustion flag stays false until an attempted positive overrun. A zero charge at the exact boundary succeeds. Any later overrun becomes sticky.

The execution/runtime caller must charge before work, keep this same meter across nested calls, and keep resource consumption outside the revertible state journal. Creating fresh child meters or returning consumed weight after a revert violates this design. No refund cap or VM refund convention can decrease already consumed Oregon weight in 3A.

## 6. Parent-only dynamic fee

`next_base_fee(&parameters, parent_base_fee, parent_weight)` validates parent fee within configured bounds and parent weight no greater than the block limit. It takes no current-block transaction input. Chainstate integration must obtain these arguments from the authenticated accepted parent; this arithmetic function cannot authenticate caller-supplied state.

Let `b` be the parent base fee, `u` the parent consumed normalized weight, `T` the target, and `D` the adjustment denominator.

- If `u == T`: return `b`.
- Otherwise let `q = floor(b * abs(u - T) / (T * D))`, with both products in `u128`.
- If `u > T`: return `min(max_base_fee, b + max(1, q))`.
- If `u < T`: return `max(min_base_fee, b - q)`.

The upward one-base-unit rounding minimum is explicit. Thus the per-block upward bound is `max(1, floor(b / D))`, with that one-unit exception at small prices; the downward bound is `floor(b / D)`. Clipping to price bounds can only reduce the movement. Evaluate addition in `u128` before narrowing so clamping cannot conceal integer wraparound.

`BlockWeightBudget::new(&parameters)` starts at zero. `include(actual_weight)` requires a positive weight not above the transaction maximum and an accumulated sum not above the block maximum. Invalid input leaves the budget unchanged. Inclusion uses consumed weight from the one execution meter, including consumption on revert/trap, never the sender's declared unused maximum. Exact limits succeed. Capacity is shared across all domains.

First-block seed, height/version schedule, fee-state encoding and parent commitment verification are Stage 3B/activation decisions and cannot be inferred from this API.

## 7. Literal examples and adversarial acceptance

The checked-in JSON contains synthetic data with provenance from a Python arbitrary-precision reference script independent of the Rust implementation. Rust consumers compare literal fields. Required examples include:

- conversion `1 * 2/3 -> 1`, `2 * 2/3 -> 2`, `3 * 2/3 -> 2`, plus zero and `u64::MAX` products;
- with intrinsic weight 2, EVM ratio 2/3: three charges of one unit leave total consumed 4, exactly matching one charge of three units;
- mixed Native/EVM/WASM/common charges share one budget; exact limit, one over, sticky exhaustion and counter/conversion overflow;
- `T=100, D=8, min=1, max=1000, b=100`: utilizations 0/99/100/101/200 produce 88/100/100/101/112;
- small-price upward minimum, downward floor, configured floor/ceiling, invalid utilization and unsupported version;
- full-parent producer sequence from 100: 112, 126, 141, 158, 177; empty-parent sequence from 100: 88, 77, 68, 60, 53;
- repeated full parents stop at the configured ceiling; no sequence exceeds the per-block movement bound;
- max-transaction and max-block boundaries reject atomically.

Property tests cover split/combined metering equivalence within native-counter bounds; monotone consumption; fee direction/range/rate bounds; and mixed-domain ordering equivalence below budget. Independent vectors must not call Rust production helpers for expected values.

Required security mutations: floor in place of conversion ceiling; per-chunk instead of cumulative conversion; skipped common charge; non-sticky exhaustion; budget off-by-one; downward update changed to upward; omitted upward minimum; omitted price ceiling; accepted excess parent utilization; accepted excess transaction/block weight. A killed mutant must compile and fail its named assertion test; a compiler error is not evidence.

## 8. Binding Stage 3B accounting requirements

These requirements constrain the next design and are not implementation claims:

- Validate structure, replay, authorization, fee caps and authoritative payer availability before escrow. A rejected pre-execution transaction has no state changes or charge.
- Let `M=max_weight`, `P=max_fee_per_weight`, `B=base_fee`, and `Q=max_priority_fee_per_weight`. Require `P >= B`; compute `effective = B + min(Q, P-B)` to avoid overflowing `B+Q`. The reserve is `M*P`, charge is `consumed*effective`, refund is reserve minus charge. All amounts pass the existing checked supply envelope.
- Base component `consumed*B` plus priority component equals the full charge. All of that value goes to producer entitlement; none is burned. Stage 3B must bind payout/refund destinations and payer identity to authoritative authorization/transaction context, never caller-selected unsigned bytes.
- Fee reserve is inaccessible to revertible contract writes. Success, revert and trap pay the same charge for the same consumed weight. Exhaustion consumes the full authorized meter budget. Account nonce/sequence consumption after inclusion survives top-level revert; native outpoint replay remains with the native owner.
- Native-funded escrow reserves spend-verified UTXO value. Execution-funded escrow deducts spendable claims while retaining a backed escrow liability. During execution, spendable claims plus live escrow liabilities equal native reserved backing. At settled block boundaries, no escrow remains and total execution claims equal native reserve outputs.
- Execution-funded producer fees reduce execution claims and reserve by exactly the charge while crediting equal native producer value. A native-funded fee does not reduce execution reserve. Deposits increase reserve and claims equally; withdrawals decrease both equally; cross-domain transfers preserve their sum.
- Claims are never added to native UTXO amounts when counting issued supply. A balance sum supplied by an untrusted caller is not evidence of backing. UTXO outpoints, reserve classification, authorization, duplicate detection, pre-state binding and resulting account roots must all be validated by their owners.
- Frozen M1 currently permits coinbase underclaim. Universal execution activation must explicitly specify exact producer execution-fee settlement and distinguish foregone subsidy from destruction of previously represented value; it cannot assert a stronger active M0–M6 supply rule retroactively.
- Journal commit/revert, fees, reserve outputs, receipts and undo become one unpublished block transition; publication follows atomic WAL+sync storage. Reorg disconnect reverses all these domains together. No standalone Stage 3 arithmetic library may claim this property is already integrated.

The 3B design must select exact fee-state and accounting keys/bytes, authenticated funding capabilities, atomic delta interfaces and stale/double-settlement rejection before writing its code. It must not introduce a second account truth alongside EVM/WASM domain balances.

## 9. Verification and continuation

3A requires focused tests and independent vectors, the mutation targets above, full workspace tests, architecture scan, rustdoc/docs, rustfmt and Clippy with warnings denied. New integer vectors run on x86_64 and ARM. RandomX is unchanged; its existing acceptance evidence remains applicable, while inherited Rust CI gates continue to run.

The checkpoint names exact source SHA, executed commands, mutation results, CI runs, review result and the exact next action. No success claim is based only on a green ancestor with different code.

After 3A, continue with the Stage 3B versioned fee-funding/escrow/state/backing design constrained by §8. Benchmark profiles, production constants, runtime/VM adapters, block/UTXO/persistence integration, activation and main integration remain separate explicitly gated work.
