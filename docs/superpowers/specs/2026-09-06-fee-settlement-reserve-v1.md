# Oregon Stage 3B Fee Settlement and Reserve Conservation V1

**Status:** versioned inactive design; no execution activation

**Date:** 2026-09-06

**Base main:** `a07eb04be910ff1312a0c2a08aa66affa4fd75b5`

**Parent contracts:**
- `docs/architecture/OREGON_ENGINEERING_CONSTITUTION.md`
- `docs/architecture/OREGON_PLATFORM_ARCHITECTURE_CONTRACT.md`
- `docs/superpowers/specs/2026-09-05-execution-architecture-design.md`
- `docs/superpowers/specs/2026-09-06-execution-resources-fees-v1.md`

## 1. Purpose and scope

Stage 3B fixes the inactive fee-funding, escrow, settlement, fee-state and native-reserve accounting contract required before any VM or universal-envelope activation. It preserves all accepted M0-M6 runtime behavior and all integrated Stage 1, Stage 2 and Stage 3A foundations.

This design does not activate the universal envelope, change current transaction/block/header bytes, change the current RocksDB schema, route EVM/WASM execution, alter the active mempool, expose wallet/RPC APIs, or replace the current native UTXO/coinbase validation path. Activation remains a separate versioned migration and integration decision.

The inherited requirements remain mandatory: one Oregon fee truth, parent-derived dynamic base fee, optional priority fee, no execution-fee burn, consumed-work charging after deterministic revert, native UTXO ownership of OREG issuance, and exact 1:1 backing between execution balances and protocol-reserved native UTXO value.

## 2. Authoritative owners and dependency direction

- `oregon-primitives` owns canonical Stage 3B receipt encodings, discriminants and domain-separated identifiers. It never mutates balances or UTXOs.
- `oregon-consensus` continues to own Stage 3A base-fee/weight rules and future execution-fee-aware producer-claim validation. The accepted M0-M6 coinbase path remains unchanged until explicit activation.
- `oregon-execution` owns the single fee lifecycle: funding-capability consumption, max-fee escrow, effective-price arithmetic, actual charge, deterministic refund, settlement status, fee receipts and block execution-fee totals.
- `oregon-utxo` owns native funding handoff, reserve-output classification, reserve-pool transitions and reserve undo.
- `oregon-contract-state` owns logical ExecutionAccounting and FeeState SMT transitions/snapshots. Physical persistence remains `oregon-storage` owned.
- a later `oregon-chainstate` integration atomically composes UTXO, reserve, execution-accounting, fee-state and receipt deltas and publishes only after WAL-enabled synchronous durability.

Dependency direction remains downward. `oregon-consensus` does not depend on `oregon-execution`; `oregon-execution` does not depend on RocksDB, chainstate, RPC, networking or VM implementations.

## 3. Fee payer and funding-source policy

The effective payer is explicit `fee_payer` when present, otherwise `principal`. Existing Stage 1 authorization rules remain unchanged: a distinct payer requires its own authorization proof.

Stage 3B introduces no envelope field. An internal authenticated `FundingCapabilityV1` is produced only by the authoritative source validator after authorization succeeds.

V1 fee sources are deliberately narrow:

| Execution domain | Fee source |
| --- | --- |
| Native | authenticated native UTXO funding |
| EVM | backed execution balance |
| WASM | backed execution balance |
| System | none; fail closed in Stage 3B |

Cross-domain fee sponsorship is not activated in V1. A future design may extend capability production without changing the outer envelope if it preserves payer authorization and the one settlement truth.

## 4. Authenticated funding capability

Canonical test encoding of `FundingCapabilityV1` is:

```
version:                  u16 LE = 1
source_kind:              u8
payer:                    33-byte ExecutionAddress
source_commitment:        Hash256
available_amount:         u64 LE
source_sequence:          u64 LE
authorization_commitment: Hash256
```

Source kinds:

- `0x01` = `NativeUtxo`
- `0x02` = `ExecutionBalance`

`capability_id = H("OREGON/FEE/CAPABILITY/V1\0", canonical_bytes)`.

For native funding, `source_commitment` commits the authorized funding set in ascending `OutPoint` order, including each outpoint, amount and locking-program commitment. `source_sequence` is zero. Staleness is detected by exact outpoint existence and lock ownership in the authoritative overlay.

For execution funding, `source_commitment` binds the current ExecutionAccounting root and payer key. `source_sequence` is the payer's exact current accounting sequence. A capability with a stale root/sequence, payer mismatch or authorization mismatch fails closed.

Capabilities are transaction-scoped authority tokens. RPC, a VM backend or an arbitrary caller cannot construct one and thereby gain spending authority.

## 5. Fee arithmetic and pre-execution gate

The envelope supplies `max_fee_per_weight`, `max_priority_fee_per_weight` and `max_weight`; consensus supplies the current block `base_fee_per_weight`.

Pre-execution validation requires:

1. `max_weight` is nonzero and within the activated transaction-weight limit;
2. `max_fee_per_weight >= base_fee_per_weight`;
3. `max_priority_fee_per_weight <= max_fee_per_weight`;
4. `max_escrow = max_weight * max_fee_per_weight`, evaluated in `u128`, fits `u64` and is within the Oregon supply envelope;
5. the authenticated capability exposes at least `max_escrow` available value;
6. the capability is fresh and unconsumed in the canonical overlay.

Price and settlement arithmetic is:

`effective_price = min(max_fee_per_weight, base_fee_per_weight + max_priority_fee_per_weight)`

`actual_charge = actual_weight * effective_price`

`base_component = actual_weight * base_fee_per_weight`

`priority_component = actual_charge - base_component`

`refund = max_escrow - actual_charge`

The addition and all products use `u128`. Every narrowing conversion is checked. `actual_weight` must be in `1..=max_weight`. Overflow, impossible subtraction, charge above escrow or out-of-range amount invalidates the candidate transition; no fee arithmetic saturates silently.

## 6. Escrow lifecycle and rollback boundary

`oregon-execution` owns a transaction-scoped state machine:

`Open -> Settled`

The lifecycle is:

1. structural/domain/replay/authorization validation;
2. funding capability validation;
3. lock/reserve `max_escrow` in the authoritative source overlay;
4. execute using a transaction-scoped journal and normalized meter;
5. commit or revert revertible execution writes according to the deterministic execution outcome;
6. charge `actual_charge`;
7. return `refund` to the same source;
8. append one settlement receipt;
9. add the execution-funded charge exactly once to the block execution-fee accumulator.

A deterministic top-level revert, explicit VM revert or resource exhaustion does not undo escrow or consumed-work fees. Revertible contract writes roll back; the fee charge survives.

A structural/authentication/funding failure before escrow is an invalid transaction and charges nothing. A fatal invariant/runtime fault, arithmetic failure, commitment mismatch or storage failure invalidates the whole candidate block overlay and publishes nothing.

## 7. Escrow identity and stale/double-settlement rejection

Canonical `EscrowTicketV1` bytes are:

```
version:          u16 LE = 1
txid:             Hash256
payer:            33-byte ExecutionAddress
source_kind:      u8
capability_id:    Hash256
base_fee:         u64 LE
max_fee:          u64 LE
max_priority_fee: u64 LE
max_weight:       u64 LE
max_escrow:       u64 LE
```

`escrow_id = H("OREGON/FEE/ESCROW/V1\0", canonical_bytes)`.

Opening an existing escrow id, settling an unknown/already-settled id, mutating ticket identity between open and settle, or reusing a consumed capability fails closed. Native funding additionally requires the exact referenced outpoints to remain locked by that capability. Execution funding additionally requires the exact accounting sequence.

Reorg replay restores the parent snapshot from undo and recreates capabilities from that authoritative state; forward-run in-memory tickets are never reused.

## 8. Canonical fee settlement receipt

`FeeSettlementReceiptV1` is inactive canonical data owned by `oregon-primitives`.

Encoding:

```
version:                     u16 LE = 1
txid:                        Hash256
escrow_id:                   Hash256
payer:                       33-byte ExecutionAddress
source_kind:                 u8
execution_outcome:           u8
base_fee_per_weight:         u64 LE
max_fee_per_weight:          u64 LE
max_priority_fee_per_weight: u64 LE
max_weight:                  u64 LE
actual_weight:               u64 LE
effective_price:             u64 LE
base_component:              u64 LE
priority_component:          u64 LE
charged:                     u64 LE
refund:                      u64 LE
```

`execution_outcome`:

- `0x00` committed
- `0x01` reverted
- `0x02` resource-exhausted/reverted

Unknown values fail closed.

`receipt_id = H("OREGON/FEE/RECEIPT/V1\0", canonical_bytes)`.

Receipt decoding/reconstruction must recompute all arithmetic from caps, base fee and actual weight; duplicated arithmetic fields are not trusted independently. Fee receipts later live under the already reserved `ExecutionReceipts` commitment domain, not a second receipt root.

## 9. ExecutionAccounting state keys

Execution balances use `CommitmentDomainId::ExecutionAccounting` with `OregonSmtV1`.

Raw key bytes before the existing domain-separated `path_key` hash are:

- `b"acct/v1/balance/" || address[33]`
- `b"acct/v1/sequence/" || address[33]`
- `b"acct/v1/total_execution_balance"`
- `b"acct/v1/reserve_outpoint"`

Values:

- balance/total/sequence: exactly 8-byte `u64` LE;
- reserve outpoint: `0x00` when absent, otherwise `0x01 || txid[32] || index_u32_le`.

Zero account balance is key absence, not an encoded zero leaf. Account sequence is monotonic and advances on each accepted authorized accounting transition that consumes the account capability. Total execution balance changes atomically with account deltas.

## 10. FeeState commitment keys

Fee state uses `CommitmentDomainId::FeeState` with `OregonSmtV1`.

Raw keys are exactly:

- `b"fee/v1/height"`
- `b"fee/v1/base_fee_per_weight"`
- `b"fee/v1/block_weight_used"`
- `b"fee/v1/execution_fee_total"`
- `b"fee/v1/producer_coinbase_txid"`
- `b"fee/v1/reserve_transition_id"`

Values:

- height/base fee/block weight/execution fee total: 8-byte `u64` LE;
- producer coinbase txid/reserve transition id: 32 bytes.

The base fee and block weight stored for block `H` are the parent-only inputs used by Stage 3A to derive block `H+1`. No transaction in `H+1` affects its own base fee.

## 11. Protocol reserve UTXO representation

Stage 3B selects one canonical protocol reserve-pool UTXO instead of fragmented per-account reserve outputs. It remains native UTXO value; execution balances are only claims against it.

The reserve `TxOutput.locking_program` is exactly 23 bytes:

`b"OREGON/EXEC/RESERVE/V1\0"`

hex:

`4f5245474f4e2f455845432f524553455256452f563100`

At future activation, ordinary native paths may neither create nor spend this exact program. Only the authoritative inactive-then-activated reserve transition API in `oregon-utxo` can manipulate it.

At an accepted execution boundary there is at most one live reserve output. Its value equals `acct/v1/total_execution_balance` exactly. If total execution balance is zero, no reserve output exists and `acct/v1/reserve_outpoint` is `0x00`.

## 12. Reserve transition and acyclic derivation order

A block-level `ReserveTransitionV1` applies the net result after transaction settlements in canonical block order.

The **transition-id preimage** is deliberately independent of post-state roots and the new reserve outpoint. This prevents circular commitment dependencies.

Canonical transition-id bytes are:

```
version:                     u16 LE = 1
chain_id:                    u64 LE
height:                      u64 LE
parent_block_hash:           Hash256
previous_reserve_flag:       u8
previous_reserve_outpoint:   36 bytes when flag = 1
previous_reserve_amount:     u64 LE
native_deposit_total:        u64 LE
execution_withdrawal_total:  u64 LE
execution_fee_total:         u64 LE
new_execution_balance_total: u64 LE
producer_coinbase_txid:      Hash256
```

`previous_reserve_flag` is only `0x00`/`0x01`. If absent, previous amount is zero. If present, the outpoint must resolve to exactly one UTXO with the exact reserve program and recorded amount.

Checked arithmetic must satisfy:

`new_reserve = previous_reserve_amount + native_deposit_total - execution_withdrawal_total - execution_fee_total`

`new_reserve == new_execution_balance_total`.

`reserve_transition_id = H("OREGON/RESERVE/TRANSITION/V1\0", transition_id_bytes)`.

If `new_reserve > 0`, the new reserve outpoint is derived next:

- `txid = H("OREGON/RESERVE/OUTPOINT/V1\0", reserve_transition_id)`
- `index = 0`

The new reserve output value is exactly `new_reserve` and uses the exact reserve program. If `new_reserve == 0`, no new reserve UTXO exists.

Only **after** `reserve_transition_id` and the resulting reserve outpoint are known does the implementation compute post-state commitments:

1. apply all execution-accounting deltas and write the resulting reserve outpoint key;
2. compute the `ExecutionAccounting` SMT root;
3. write FeeState including `reserve_transition_id` and the already known producer coinbase txid;
4. compute the `FeeState` SMT root;
5. validate the native reserve UTXO/value against the ExecutionAccounting total/outpoint;
6. place the independent descriptors into the existing canonical aggregate commitment set.

No transition id hashes a root that itself contains that transition id/outpoint. No post-state root is an input to reserve outpoint derivation.

The old reserve UTXO is consumed and the new one is created atomically; the implementation never edits one UTXO in place.

## 13. Deposits, withdrawals, internal transfers and reserve delta

A native-to-execution deposit is valid only after authoritative native authorization produces a capability for the exact native value diverted. The native value is consumed in the same block overlay in which the destination execution balance and reserve pool increase by the same amount.

An execution-to-native withdrawal debits the authorized execution balance, decreases the reserve by the same amount and creates authenticated normal native output value in the same overlay. Exact future native-envelope deposit/withdraw payload bytes are not invented by Stage 3B.

Execution-to-execution transfers, including EVM/WASM cross-domain balance movement through a later approved runtime boundary, move backed claims only and have zero reserve delta.

Native-domain fees are funded from the native overlay and do not enter the reserve. EVM/WASM fees are execution-funded: max escrow is locked in the execution balance, the refund is restored, and the net actual charge reduces both total execution balance and reserve at block settlement.

A deterministic reverted execution therefore still reduces execution payer balance and reserve by the actual consumed fee.

## 14. Producer fee assignment and no-burn rule

Execution-funded fees leaving the reserve must reappear as native producer value in the same accepted block transition. They may not be burned, left in an unowned account or credited only inside a VM.

Stage 3B defines a separate **inactive** execution-fee-aware coinbase helper for future activation; it does not replace current `validate_coinbase`.

At activation:

1. authoritative native and execution transitions compute `native_fee_total` and `execution_fee_total` independently;
2. miner outputs must claim at least `native_fee_total + execution_fee_total`; any permitted underclaim is therefore attributable only to not-yet-issued subsidy, not transaction fees;
3. when `execution_fee_total > 0`, the final miner output after any mandatory founder output has value exactly `execution_fee_total` and must not use the reserve locking program;
4. FeeState commits the full canonical coinbase txid;
5. reserve value decreases by exactly `execution_fee_total`.

This makes execution-fee payout observable and preserves the no-burn rule without silently changing the accepted pre-activation coinbase path.

## 15. Canonical block settlement order

Transactions settle in canonical block order. A block owns one normalized `BlockWeightBudget`, one execution-fee accumulator and one ordered fee-receipt list.

Each included envelope opens escrow once, produces one deterministic outcome, settles once, appends one receipt and adds its execution-funded charge once. After all transactions, the block derives the reserve transition id/outpoint in the acyclic order above, computes ExecutionAccounting and FeeState roots, verifies 1:1 backing and validates producer fee assignment. No ingress or VM backend gets a second settlement path.

## 16. Persistence, undo and reorg composition

Stage 3B logical code returns explicit forward delta and undo data; it does not write RocksDB directly.

A later chainstate integration commits one WAL-enabled synchronous batch containing native UTXO changes, reserve changes, ExecutionAccounting changes, FeeState changes, receipt data and complete undo. In-memory accepted state is published only after that batch succeeds.

Undo records at minimum the previous reserve outpoint/entry, changed account balances/sequences, previous total execution balance, previous fee-state values/root and native deposit/withdrawal UTXO effects. Disconnect restores the exact parent cross-domain invariant before publication.

A reorg first restores the parent snapshot from undo, then validates the competing branch. It never reuses stale forward-run capabilities or escrow tickets.

## 17. Required independent vectors and adversarial tests

Implementation is test-first and must cover at least:

- max escrow and effective-price boundaries;
- `max_fee < base_fee` rejection;
- priority split/refund exactness;
- `u128` product/narrowing overflow;
- exact max weight and one-over rejection;
- commit/revert/resource-exhausted charging;
- native vs execution source matrix and sponsored payer binding;
- stale execution sequence/root and stale native outpoint rejection;
- duplicate capability, escrow-open, unknown settlement and double settlement rejection;
- canonical receipt bytes/hash and unknown outcome rejection;
- reserve creation from zero, increase, decrease and removal at zero;
- reserve underflow/overflow, stale previous outpoint, wrong reserve program and multiple-live-reserve rejection;
- exact reserve/execution-total equality;
- reverted execution fee still decreasing reserve;
- execution internal transfer with zero reserve delta;
- transition-id/root derivation independence (regression against commitment cycles);
- wrong/missing producer execution-fee output and fee underclaim rejection under the inactive helper;
- forward/undo/forward equivalence;
- later persistence fault injection proving no publication on failed durable write.

Required named mutation targets include: bypass max-fee/base-fee gate, unchecked fee multiplication, refund off-by-one, free revert, duplicate settlement acceptance, stale capability acceptance, reserve-program bypass, reserve underflow, reserve-equality bypass, fee accumulator double count, producer payout omission, FeeState domain/key substitution, transition/root-cycle regression and reserve undo mismatch. Compile failure alone is not a mutation kill.

## 18. Explicit non-goals

Stage 3B does not choose or activate EVM/WASM engines, native universal-envelope deposit/withdraw payload bytes, wallet coin selection, RPC, mempool replacement policy, RocksDB column-family names, header extension bytes/activation height, network execution messages, cross-VM runtime calls or async bridge/oracle/AI semantics.

## 19. Acceptance boundary

Stage 3B is accepted only when current active M0-M6 and integrated Stage 1/2/3A behavior remains unchanged, new helpers remain inactive/isolated, independent vectors and focused tests pass, intended mutation targets are killed, full workspace/rustdoc/docs/format/warnings-denied Clippy pass, architecture gates remain clean, a checkpoint records exact source/tree and CI evidence, and `main` integration occurs only after a separate explicit owner decision.
