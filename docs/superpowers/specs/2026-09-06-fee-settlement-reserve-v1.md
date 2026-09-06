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

This design does not activate the universal envelope, change current transaction/block/header bytes, change the current RocksDB schema, route EVM/WASM execution, alter the active mempool, expose wallet/RPC APIs, or change the current native UTXO/coinbase validation path. Any future activation is a separate versioned migration and integration decision.

The frozen parent decisions remain mandatory:

- one Oregon execution/fee truth;
- dynamic parent-derived base fee over normalized weight;
- optional priority fee;
- no execution-fee burn;
- fee escrow survives top-level execution revert for consumed work;
- native OREG issuance remains UTXO-owned;
- execution balances are 1:1 claims on protocol-reserved native UTXO value;
- no VM, contract, bridge, RPC, oracle, AI subsystem or administrator can mint OREG.

## 2. Authoritative owners and dependency direction

### 2.1 `oregon-primitives`

Owns canonical Stage 3B receipt/state wire primitives, discriminants, fixed byte encodings, identifiers and domain-separated hashes. It does not debit balances or mutate UTXOs.

### 2.2 `oregon-consensus`

Owns Stage 3A base-fee/weight rules and the future activation rule that bounds producer claims. Stage 3B may add inactive helpers for execution-fee-aware coinbase validation, but the accepted M0-M6 validator remains unchanged until a separately approved activation.

### 2.3 `oregon-execution`

Owns the single fee lifecycle: funding-capability consumption, maximum-fee escrow, effective-price arithmetic, actual charge, deterministic refund, settlement status, fee receipts and block-level execution-fee totals. No VM implements its own Oregon fee settlement.

### 2.4 `oregon-utxo`

Owns native funding authorization handoff, classification of protocol reserve UTXOs, reserve-pool transitions, reserve undo and rejection of unauthorized reserve spends/creation once the future activation path is enabled. Reserve representation never leaks into VM or RPC code.

### 2.5 `oregon-contract-state`

Owns logical execution-accounting and fee-state SMT transitions/snapshots. Physical RocksDB layout remains `oregon-storage`'s responsibility.

### 2.6 `oregon-chainstate` and `oregon-storage`

A later integration stage atomically composes native UTXO, reserve, execution-accounting, fee-state and receipt changes into one durable block transition. No Stage 3B helper may publish partial state. Accepted publication remains after WAL-enabled synchronous durability.

Dependency direction stays downward. `oregon-consensus` must not depend on `oregon-execution`; `oregon-execution` must not depend on RocksDB, chainstate, RPC or VM implementations.

## 3. Fee payer and funding-source policy

The effective payer is the envelope's explicit `fee_payer` when present, otherwise `principal`. Existing outer-envelope authorization rules remain unchanged: a distinct fee payer requires its own authorization proof.

Stage 3B uses an internal authenticated funding capability. It is not a new envelope field and therefore does not modify the Stage 1 wire format.

V1 source policy is intentionally narrow:

| Execution domain | Allowed fee source | Notes |
| --- | --- | --- |
| Native | authenticated native UTXO funding | selected native value is locked in the transaction overlay before execution/settlement |
| EVM | backed execution balance | fee payer balance is part of authoritative execution accounting |
| WASM | backed execution balance | same Oregon settlement path as EVM |
| System | none in Stage 3B | user-fee semantics remain fail-closed until separately designed |

Cross-domain fee sponsorship is not activated in V1. A future design may extend the funding-capability producer without changing the envelope if it preserves the same payer authorization and settlement truth.

## 4. Authenticated funding capability

`FundingCapabilityV1` is an internal transaction-scoped value issued only by an authoritative source validator after payer authorization succeeds.

Canonical test encoding:

```
version:                  u16 LE = 1
source_kind:              u8
payer:                    33-byte ExecutionAddress
source_commitment:        Hash256
available_amount:         u64 LE
source_sequence:          u64 LE
authorization_commitment: Hash256
```

`source_kind`:

- `0x01` = `NativeUtxo`
- `0x02` = `ExecutionBalance`

`capability_id = H("OREGON/FEE/CAPABILITY/V1\0", canonical_bytes)`.

For native funding, `source_commitment` commits the canonical authorized native funding set in ascending `OutPoint` order, including each outpoint, amount and locking-program commitment. `source_sequence` is zero; native staleness is detected by the referenced outpoints still being present and unlocked in the authoritative overlay.

For execution funding, `source_commitment` binds the current execution-accounting root and payer key. `source_sequence` is the payer's exact current accounting sequence. Escrow consumes that sequence exactly once. A capability with a stale root, stale sequence, mismatched payer or mismatched authorization commitment fails closed.

Capabilities are never accepted from RPC, a VM backend or an untrusted caller as claims of authority. They are produced by authoritative validation and consumed once by `oregon-execution`.

## 5. Fee arithmetic and pre-execution gate

Let the envelope provide:

- `max_fee_per_weight`
- `max_priority_fee_per_weight`
- `max_weight`

and let consensus provide the current block `base_fee_per_weight`.

Pre-execution validation requires:

1. `max_weight` is nonzero and does not exceed the activated per-transaction weight limit;
2. `max_fee_per_weight >= base_fee_per_weight`;
3. `max_priority_fee_per_weight <= max_fee_per_weight` (already structurally enforced by `FeeCaps`);
4. `max_escrow = max_weight * max_fee_per_weight` is computed in `u128`, fits `u64`, and does not exceed the Oregon supply envelope;
5. the authenticated funding capability has at least `max_escrow` available;
6. the capability has not been consumed or invalidated by a prior transition in the same canonical overlay.

The effective price is:

`effective_price = min(max_fee_per_weight, base_fee_per_weight + max_priority_fee_per_weight)`

The addition is evaluated in `u128`; narrowing occurs only after the capped result is proven to fit `u64`.

After execution:

`actual_charge = actual_weight * effective_price`

`base_component = actual_weight * base_fee_per_weight`

`priority_component = actual_charge - base_component`

`refund = max_escrow - actual_charge`

All products use `u128`; every narrowing conversion is checked. `actual_weight` must be in `1..=max_weight`. Arithmetic overflow, impossible subtraction, a charge above escrow, or an out-of-range amount invalidates the block transition rather than saturating.

## 6. Escrow lifecycle and rollback boundary

`oregon-execution` owns one transaction-scoped escrow state machine:

`Open -> Settled`

There is no path from `Settled` back to `Open` inside one forward block transition.

The lifecycle is:

1. structural/domain/replay/authorization checks;
2. funding capability validation;
3. reserve `max_escrow` from the authoritative source overlay;
4. execute using a transaction-scoped journal and normalized meter;
5. either commit revertible execution writes or revert them according to the deterministic execution outcome;
6. charge `actual_charge` from escrow;
7. refund `refund` to the same authenticated source;
8. record one settlement receipt;
9. accumulate `actual_charge` into the block's execution-fee total.

A top-level deterministic contract revert, explicit VM revert, or normalized resource exhaustion does not undo escrow or consumed-resource fees. Revertible contract/account writes roll back; the fee charge survives.

A structural/authentication/funding failure before escrow means the transaction is invalid and no fee is charged.

A fatal invariant failure, host/runtime fault that is not a valid deterministic transaction outcome, arithmetic failure, commitment mismatch or storage failure invalidates the whole candidate block transition. The overlay is discarded; no partial fee or execution state is published.

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

The transaction overlay maintains a set of opened and settled escrow ids. Opening an existing id, settling an unknown id, settling an already settled id, changing payer/source/capability fields between open and settle, or reusing a consumed capability fails closed.

Native staleness is additionally enforced by outpoint existence/lock ownership. Execution-balance staleness is additionally enforced by exact accounting sequence. Reorg replay recreates the prior source state and sequences from undo; it does not reuse forward-run in-memory tickets.

## 8. Canonical fee settlement receipt

`FeeSettlementReceiptV1` is consensus-facing inactive canonical data owned by `oregon-primitives`.

Encoding:

```
version:                  u16 LE = 1
txid:                     Hash256
escrow_id:                Hash256
payer:                    33-byte ExecutionAddress
source_kind:              u8
execution_outcome:        u8
base_fee_per_weight:      u64 LE
max_fee_per_weight:       u64 LE
max_priority_fee_per_weight: u64 LE
max_weight:               u64 LE
actual_weight:            u64 LE
effective_price:          u64 LE
base_component:           u64 LE
priority_component:       u64 LE
charged:                  u64 LE
refund:                   u64 LE
```

`execution_outcome`:

- `0x00` = committed
- `0x01` = reverted
- `0x02` = resource exhausted/reverted

Unknown values fail closed.

`receipt_id = H("OREGON/FEE/RECEIPT/V1\0", canonical_bytes)`.

The receipt must recompute exactly from the envelope caps, current base fee, actual normalized weight and settlement result. A decoder never trusts duplicated arithmetic fields without validation.

Fee receipts are later included under the existing `ExecutionReceipts` commitment domain; Stage 3B does not add a competing receipt root.

## 9. Execution-accounting state keys

Execution balances use the already reserved `CommitmentDomainId::ExecutionAccounting` with `OregonSmtV1`.

Raw key bytes before the existing domain-separated `path_key` hashing are:

- balance: `b"acct/v1/balance/" || address[33]`
- sequence: `b"acct/v1/sequence/" || address[33]`
- total execution balance: `b"acct/v1/total_execution_balance"`
- current reserve outpoint: `b"acct/v1/reserve_outpoint"`

Values:

- balances and total execution balance: exactly 8-byte `u64` LE;
- sequences: exactly 8-byte `u64` LE;
- reserve outpoint: `0x00` when no reserve exists, otherwise `0x01 || txid[32] || index_u32_le`.

A zero balance is represented by key absence, not an encoded zero leaf. Per-account sequence is monotonic and increments on an accepted debit/credit transition that consumes an authenticated execution-accounting capability. The total balance is updated atomically with all account deltas and is a derived invariant checked against the resulting account set in tests/vectors.

## 10. Fee-state commitment keys

Fee state uses the already reserved `CommitmentDomainId::FeeState` with `OregonSmtV1`.

Raw key bytes are exactly:

- `b"fee/v1/height"`
- `b"fee/v1/base_fee_per_weight"`
- `b"fee/v1/block_weight_used"`
- `b"fee/v1/execution_fee_total"`
- `b"fee/v1/producer_coinbase_txid"`
- `b"fee/v1/reserve_transition_id"`

Values are:

- height/base fee/block weight/execution fee total: exactly 8-byte `u64` LE;
- producer coinbase txid/reserve transition id: exactly 32 bytes.

The base fee and block weight stored for block `H` are the values used by Stage 3A's parent-only rule to derive block `H+1`. No transaction in `H+1` may influence its own base fee.

The FeeState descriptor remains distinct from ExecutionAccounting and NativeUtxo descriptors. The aggregate state commitment orders domains by the already frozen numeric domain ids.

## 11. Protocol reserve UTXO representation

Stage 3B chooses one canonical protocol reserve pool UTXO rather than a fragmented per-account reserve set. This keeps the 1:1 backing proof and undo boundary small and deterministic while preserving native UTXO ownership of OREG.

The reserve `TxOutput.locking_program` is exactly the 23 bytes:

`b"OREGON/EXEC/RESERVE/V1\0"`

hex:

`4f5245474f4e2f455845432f524553455256452f563100`

This program is protocol-reserved. Once the future activation path is enabled, an ordinary native spend path may neither create nor spend an output with this exact program. Only the authoritative reserve-transition API in `oregon-utxo` may do so. Stage 3B implementation helpers remain inactive and are not wired into accepted M0-M6 validation.

There is at most one live reserve pool UTXO. Its value must equal `acct/v1/total_execution_balance` exactly. If the total is zero, no reserve UTXO exists and the accounting reserve-outpoint value is `0x00`.

## 12. Reserve transition and canonical reserve outpoint

A block-level `ReserveTransitionV1` applies the net result after all transaction settlements in canonical block order.

Canonical transition bytes are:

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
fee_state_root:               Hash256
execution_accounting_root:    Hash256
```

`previous_reserve_flag` is only `0x00` or `0x01`. When zero, the outpoint bytes are absent and `previous_reserve_amount` must be zero. When one, the referenced UTXO must exist, have the exact reserve locking program and exact recorded amount.

Arithmetic is checked in `u128` and must satisfy:

`new_reserve = previous_reserve_amount + native_deposit_total - execution_withdrawal_total - execution_fee_total`

and

`new_reserve == new_execution_balance_total`.

Reserve underflow, overflow, a stale previous reserve outpoint, mismatched amount/program, more than one live reserve output, or accounting-root mismatch invalidates the transition.

`reserve_transition_id = H("OREGON/RESERVE/TRANSITION/V1\0", canonical_transition_bytes)`.

When `new_reserve > 0`, the new protocol reserve outpoint is:

- `txid = H("OREGON/RESERVE/OUTPOINT/V1\0", reserve_transition_id)`
- `index = 0`

and its output value is exactly `new_reserve` with the exact reserve locking program above. Domain separation makes collision with ordinary Oregon transaction ids cryptographically distinct. When `new_reserve == 0`, no new reserve outpoint is created.

The old reserve output is consumed and the new reserve output is created atomically with the corresponding execution-accounting transition. There is no in-place mutation of one UTXO.

## 13. Deposits, withdrawals and internal transfers

A native-to-execution deposit is valid only after authoritative native authorization produces a capability for the exact native value being diverted. The native value is consumed in the same atomic block overlay in which the selected execution balance is credited and the reserve pool increases by the same amount.

An execution-to-native withdrawal debits the authorized execution balance, decreases the reserve pool by the same amount and creates authenticated normal native output value in the same atomic overlay. Withdrawal output bytes and destination authorization belong to the future native execution-domain payload design; Stage 3B fixes the accounting consequence and forbids an unbacked release.

EVM-to-WASM, WASM-to-EVM and same-domain execution-balance transfers do not change total execution balance or reserve value. They only move backed claims between accounts and therefore have zero reserve delta.

No deposit, withdrawal, transfer, fee charge or refund may cause the total execution balance to diverge from reserve value at a block acceptance boundary.

## 14. Native-funded versus execution-funded escrow

Native-domain fees are funded from the authenticated native overlay. `max_escrow` value is locked before execution and final native outputs are not published until settlement. The final native fee is the actual charge; unused locked value returns through the same authoritative native transition. Native fee value does not enter the execution reserve pool.

EVM/WASM fees are funded from backed execution balances. Opening escrow temporarily removes `max_escrow` from payer-available balance in the transaction overlay. Settlement returns the refund and leaves the net `actual_charge` debited. At block reserve settlement, the reserve pool decreases by the aggregate execution-funded `actual_charge` total.

A deterministic contract revert therefore still reduces the execution payer balance and reserve by the actual consumed-fee amount.

## 15. Producer fee assignment and no-burn rule

Execution-funded fees leave the reserve pool and must reappear as native producer value in the same accepted block transition. They may not be omitted, burned, retained in an unowned limbo account, or credited only inside a VM.

The future execution-fee-aware coinbase rule is versioned and inactive in Stage 3B. At activation:

1. the block validator computes `native_fee_total` and `execution_fee_total` independently from authoritative transitions;
2. miner outputs must claim at least `native_fee_total + execution_fee_total`; any allowed underclaim is therefore attributable only to not-yet-issued subsidy, never to transaction fees;
3. if `execution_fee_total > 0`, the final miner output after any mandatory founder output must have value exactly `execution_fee_total` and must not use the protocol reserve locking program;
4. the fee-state transition commits the complete canonical coinbase txid as `producer_coinbase_txid`;
5. the reserve pool decreases by exactly `execution_fee_total`.

The current accepted `validate_coinbase` behavior remains untouched until that explicit activation. Stage 3B may implement a separate inactive validator/helper and distinguishing tests; it must not replace the current path silently.

## 16. Block ordering and one settlement truth

Transactions settle in canonical block order. The block owns one mutable execution-fee accumulator and one normalized `BlockWeightBudget`.

For each included envelope:

- resource inclusion succeeds once;
- escrow opens once;
- execution produces one deterministic outcome;
- escrow settles once;
- one fee receipt is appended in canonical order;
- charged execution-funded fee is added once to the block execution-fee total.

After the last transaction, the implementation constructs the ExecutionAccounting root, FeeState root and reserve transition, validates 1:1 reserve equality and validates producer fee assignment. No VM-specific or ingress-specific settlement path exists.

## 17. Persistence, undo and reorg composition

Stage 3B logical code must return explicit forward delta and undo data rather than mutating storage directly.

A later chainstate integration writes, in one WAL-enabled synchronous RocksDB batch:

- normal native UTXO changes;
- previous/new reserve UTXO changes;
- execution-accounting state changes;
- fee-state changes;
- execution receipts/receipt commitment data;
- block undo sufficient to restore every prior value/root/outpoint.

In-memory accepted state is published only after that batch succeeds. Any write/sync failure faults the candidate acceptance and exposes no partial new state.

Undo records the previous reserve outpoint and entry, all execution-accounting old values/sequences, prior fee-state values/root, receipt/settlement state required by the persistence schema, and the native outputs/inputs touched by deposits/withdrawals. Disconnect restores the exact previous cross-domain invariant before publication.

A reorg never "settles again" against current forward state. It first restores the parent snapshot using undo, then validates the competing branch from that authoritative parent state.

## 18. Required vectors and adversarial tests

Implementation must be test-first and include independent literal vectors for at least:

- max escrow at zero/one/max boundaries;
- `max_fee < base_fee` rejection;
- effective-price cap and priority split;
- `u64`/`u128` product boundaries and narrowing overflow;
- exact `max_weight` use and one-over rejection;
- success, explicit revert and resource-exhausted fee outcomes;
- refund exactness;
- native versus execution funding-source matrix;
- distinct sponsored payer authorization binding;
- stale execution sequence/root rejection;
- stale/missing native funding outpoint rejection;
- duplicate capability consumption;
- duplicate escrow open/settle and unknown-settlement rejection;
- canonical receipt bytes/hash and non-canonical outcome rejection;
- reserve creation from zero;
- reserve increase/decrease/zero removal;
- reserve underflow and arithmetic overflow;
- stale previous reserve outpoint and wrong reserve locking program;
- two-live-reserve-output rejection;
- execution-total/reserve mismatch;
- execution fee on reverted transaction still reducing reserve;
- internal EVM/WASM transfer with zero reserve delta;
- wrong/missing producer execution-fee output;
- transaction-fee underclaim rejection under the inactive V2 helper;
- forward/undo/forward equivalence for reserve/accounting/fee state;
- durable-write failure producing no publication once persistence integration is implemented.

Mutation gates must target, by named test, at minimum: skip max-fee/base-fee gate, unchecked fee multiplication, refund off by one, free revert, duplicate settlement acceptance, stale capability acceptance, reserve-program bypass, reserve underflow, reserve equality bypass, fee accumulator double count, producer payout omission, fee-state key/domain substitution and undo reserve mismatch. Compilation failure alone is not a mutation kill.

## 19. Explicit non-goals

Stage 3B does not choose or activate:

- EVM engine/revision;
- WASM engine/schedule beyond Stage 3A's normalized meter contract;
- native universal-envelope payload bytes for deposits/withdrawals;
- wallet coin selection or RPC methods;
- mempool nonce lanes or replacement policy;
- RocksDB column-family names/codecs;
- block-header activation bytes/height;
- public execution-network protocol messages;
- cross-VM runtime calls;
- async bridge/oracle/AI semantics.

Those remain later versioned designs. Stage 3B defines the accounting/escrow/reserve contract they must consume.

## 20. Acceptance boundary

A Stage 3B implementation is acceptable only when:

1. current M0-M6 and integrated Stage 1/2/3A behavior remains byte-for-byte/behaviorally unchanged on the active path;
2. the new Stage 3B path is inactive and isolated;
3. independent vectors and focused tests pass;
4. required mutation targets are killed by intended named tests;
5. full workspace tests, rustdoc/docs, format and warnings-denied Clippy pass;
6. x86_64/ARM deterministic vectors pass where the new vector workflow requires them;
7. a new Stage 3B checkpoint records exact source/tree and CI evidence;
8. integration to `main` happens only after a separate explicit owner decision.
