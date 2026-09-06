# Oregon Stage 3B Fee Settlement and Reserve Progress Checkpoint

Recorded: 2026-09-06 UTC. This is the verified **inactive** Stage 3B fee-settlement and native-reserve foundation. It does not activate universal execution, VM backends, reserve handling in the live block path, or any protocol upgrade.

## Authority and scope

- Main baseline: `a07eb04be910ff1312a0c2a08aa66affa4fd75b5`.
- Stage 3B design: `docs/superpowers/specs/2026-09-06-fee-settlement-reserve-v1.md`.
- Stage 3B implementation plan: `docs/superpowers/plans/2026-09-06-fee-settlement-reserve-v1.md`.
- Implementation branch: `work/fee-settlement-reserve-v1-2026-09-06`.
- Draft PR: #17, `Stage 3B: fee settlement and reserve foundation`.
- Exact verified implementation source: `670fa62c078882f40d82d05ad25d894b5eaf8bf6`.
- Exact verified implementation tree: `370cc3743626c1be7fd172e60bee4a321f82ab36`.
- `main` remains `a07eb04be910ff1312a0c2a08aa66affa4fd75b5`; Stage 3B has not been merged or activated.

## Delivered inactive foundation

- `oregon-primitives` owns canonical `FeeSettlementReceiptV1` bytes/identity, fee-source and execution-outcome discriminants, the exact versioned execution-reserve locking program, reserve transition identity and derived reserve outpoint identity.
- `oregon-execution` owns bounded authenticated funding capabilities, checked fee terms, one-shot escrow tickets and deterministic settlement. Fee products use wide arithmetic; stale/duplicate funding and duplicate settlement fail closed; deterministic revert/resource exhaustion still pays for consumed work.
- `oregon-contract-state` owns canonical `ExecutionAccounting` and `FeeState` logical writes with domain-separated commitments and exact versioned keys/value codecs.
- `oregon-utxo` owns an inactive singleton reserve-pool transition and undo model. The reserve equation is checked, execution balance must equal reserve value exactly, stale/multiple reserves fail closed, collisions are rejected atomically, and forward/undo/forward equivalence is tested.
- `oregon-consensus` exposes the separate inactive `validate_coinbase_with_execution_fees_v1` producer helper. Existing `validate_coinbase` and active block validation are unchanged.
- No current transaction/header bytes, active monetary rules, RocksDB schema, mempool policy, network protocol, node orchestration, wallet/RPC or VM backend is activated or rewritten by this stage.

## Independent vectors and adversarial gates

The committed `tests/vectors/fee-settlement-v1.json` corpus is produced by an independent Python oracle that does not invoke Rust. Its `--check` mode regenerates the complete JSON and byte-compares it with the committed artifact.

The corpus contains 14 named behavioral cases: 4 fee arithmetic cases, 1 canonical capability/escrow/receipt settlement case, 3 reserve-transition cases and 6 producer-boundary cases. Rust consumers independently bind receipt bytes/IDs, capability and escrow IDs, reserve transition/outpoint identities, settlement arithmetic, producer boundaries, FeeState commitments and reserve state transitions.

Required Stage 3B mutation targets are all killed: **14/14**. The exact verified head also preserves the inherited gates: address **3/3**, envelope **9/9**, contract-state **17/17**, execution-resource **13/13**. Mutation runners restore source bytes and reject compilation failure as a mutation kill.

## Exact verification evidence

Exact source `670fa62c078882f40d82d05ad25d894b5eaf8bf6`, tree `370cc3743626c1be7fd172e60bee4a321f82ab36`:

- Oregon Rust CI run `34039255482` (#718), job `101502890618`: **SUCCESS**. Architecture scan, focused Stage 1/2/3A/3B contracts, full workspace/all-target tests, all inherited mutation gates, Stage 3B 14/14 mutation gate, chainstate rustdoc with warnings denied, workspace docs, `cargo fmt --check`, and workspace Clippy with warnings denied all passed.
- Oregon Fee Settlement Vectors run `34039255396` (#10): **SUCCESS** on both runners. x86_64 job `101502890461` and ARM job `101502890592` passed the independent oracle check and every Stage 3B Rust consumer.
- Oregon Execution Resource Vectors run `34039255398` (#62): **SUCCESS**.
- Oregon RandomX Architecture Vector run `34039255447` (#91): **SUCCESS**.
- Oregon RandomX Full Light Parity run `34039255407` (#81): **SUCCESS**.

No green ancestor is substituted for the verified implementation tree above. The documentation checkpoint commit that contains this file changes only repository continuation records and must receive its own exact-head CI before PR closure claims are made.

## Explicit limitations

Stage 3B is a foundation, not activation. It does **not** deliver VM execution, runtime journaling, synchronous/async execution composition, durable RocksDB execution state, chainstate/reorg/WAL integration, unified execution mempool/block admission, EVM ingress/backend, WASM backend, wallet/RPC production integration or protocol activation constants.

The native reserve helpers are not wired into `apply_normal_transaction`, active `connect_block`, or the current production coinbase path. The separate execution-fee producer validator likewise has no active caller.

## Exact next action

Finish the documentation-only closure head and verify its exact PR CI. Stop before `main`: Stage 3B main integration is a separate explicit owner decision.

After an authorized integration, the next separate versioned design is the **runtime/journal/async execution core**: authoritative execution journal boundaries, atomic committed-vs-reverted state effects, non-revertible fee settlement composition, synchronous nested-call semantics, explicit asynchronous message lifecycle, receipts/undo, and durable/reorg composition requirements. VM backends and protocol activation remain later stages.
