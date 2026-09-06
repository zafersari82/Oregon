# Oregon Execution Resources and Fees Progress Checkpoint

Recorded: 2026-09-06 UTC. This is an inactive Stage 3A implementation checkpoint; it is not execution activation, reserve integration, or `main` integration.

## Authority and scope

- Accepted Stage 2 descendant: `7cfac99c40de96afb79d222cd5ade9f3a537215a`.
- Stage 3A design: `docs/superpowers/specs/2026-09-06-execution-resources-fees-v1.md`.
- Stage 3A plan: `docs/superpowers/plans/2026-09-06-execution-resources-v1.md`.
- Implementation branch: `work/execution-resources-v1-2026-09-06`.
- Stacked draft PR: [#15](https://github.com/zafersari82/Oregon/pull/15), based on PR #14's branch.
- Final verified descendant: `0db212148fae4ad612c38d0e62b58e61aab668a0`.
- Main remains `bf7675bfe17182f77d4c43e2bcbd0c283709d799` and is unchanged.

The design and plan select a cumulative rational normalized meter, parent-only bounded dynamic base fee, and shared block/transaction budgets. They explicitly split Stage 3B fee funding/escrow, fee-state commitments, authenticated reserve transitions and UTXO backing into a later versioned design. No current M0–M6 encoding, monetary rule, UTXO transition, storage schema, mempool, networking, RPC, VM or activation path is changed.

## Delivered Stage 3A surface

- `oregon-execution` is storage/network/VM-neutral and owns `WeightRatio`, `MeterScheduleV1`, `WeightMeter` and the closed `ResourceDomain` set.
- Conversion is integer-only, uses `u128` products, rounds cumulative conversion upward, rejects `u64` narrowing overflow, and never allows a child meter or refund to reset consumed weight.
- Meter limits are shared across Native/EVM/WASM/common work. Exact budget use succeeds; an overrun exhausts the meter, sets consumption to the authorized maximum, and remains sticky for later calls.
- `oregon-consensus::execution_resources` owns validated V1 fee parameters, parent-only base-fee arithmetic, bounded price movement, and atomic block-weight inclusion. Fee bounds use the existing supply envelope.
- Independent Python-generated literal vectors cover wide ratios/products, mixed domains, split/combined charges, exact and over limits, fee boundaries, producer sequences, and block-budget boundary/rejection transitions. The vector generator has a check-only mode and does not call Rust.
- Rust vector consumers and focused behavior tests are included. The resource workflow is configured for x86_64 and ARM; inherited Rust CI includes the new design, crate boundaries, focused tests and mutation gate.

## Verification evidence

Fresh local evidence available on the implementation worktree:

- `cargo +1.85.0 test --locked -p oregon-execution --all-targets`: 18 tests passed (15 meter + 3 independent-vector tests), 0 failed.
- `cargo +1.85.0 clippy --locked -p oregon-execution --all-targets -- -D warnings`: success.
- `cargo +1.85.0 fmt --all -- --check`: success.
- `cargo +1.85.0 check --locked -p oregon-consensus --tests` with the local CMake command unavailable and `CMAKE=/bin/true`: type-check success only; it is not a linked consensus test result.
- A real local consensus test run was blocked because this environment lacks CMake/RandomX build tooling. CI installs native build dependencies and is the acceptance source for linked consensus tests and full workspace gates.
- Python vector `--check`, Python syntax compilation, workflow YAML parsing, `git diff --check` and the inherited architecture scan all passed locally.
- Final Oregon Rust CI run `34024604462`, job `101463226708`, exact head `0db212148fae4ad612c38d0e62b58e61aab668a0`: SUCCESS. Architecture, focused contracts, full workspace/all-target tests, 3/3 address, 9/9 envelope, 17/17 Stage 2, and 13/13 Stage 3A resource mutations passed; rustdoc, docs, format and warnings-denied Clippy passed.
- Final Execution Resource Vector workflow `34024604471`: x86_64 job `101463226755` and ARM job `101463226655` SUCCESS at the same exact head. Independent reference vectors and linked consensus/resource tests passed on both runners.

The exact-head CI evidence above is the Stage 3A verification record. No green ancestor was used as a substitute.

## Required Stage 3B continuation

Write and review a new versioned design before implementation that fixes fee escrow/settlement bytes, fee-state commitment keys, authenticated payer/funding capabilities, rollback versus non-revertible fee boundaries, native-funded versus execution-funded escrow, reserve outpoint classification, exact accounting deltas, stale/double-settlement rejection, producer fee destination binding, receipts/undo and atomic WAL+sync/reorg composition. It must preserve no burn, one execution fee truth and exact 1:1 native reserve backing. Do not claim Stage 3B or activation from this checkpoint.
