# Oregon M3 UTXO Chainstate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Oregon's consensus-facing UTXO state-transition engine with coinbase maturity, fee accounting, same-block topological spends, atomic block application, and deterministic undo/reorg support.

**Architecture:** Add a focused `oregon-utxo` crate. It owns UTXO entries and an in-memory consensus state engine, but does not implement Schnorr/KeyCommitV1 cryptography. Every normal spend must pass a required `SpendVerifier` trait supplied by the caller; tests use a test-only verifier. Block application uses a temporary overlay and commits only after every transaction and the final coinbase fee ceiling validate. `BlockUndo` stores exactly the spent entries and newly-created outpoints required to reverse a connected block.

**Tech Stack:** Rust 1.85.0, edition 2024, existing `oregon-primitives` and `oregon-consensus`, no new production third-party dependencies.

**Spec:** `docs/superpowers/specs/2026-09-02-oregon-pow-consensus-v1-design.md`

## Global Constraints

- UTXO model; no account balances.
- All amount arithmetic is checked integer base-unit arithmetic; no floating point.
- Coinbase maturity is exactly 120 blocks.
- Height-1 founder allocation remains exactly 50,000 OREG in coinbase output 0 and is subject to the same 120-block coinbase maturity.
- Fees are `sum(inputs) - sum(outputs)` for normal transactions and are not new issuance.
- Miner coinbase claim must remain `<= subsidy(height) + fees`; underclaim is valid.
- Same-block spends may reference only outputs created by an earlier transaction in that same block.
- Missing UTXOs, duplicate inputs, immature coinbase spends, output-value overflow, and input-value underflow are consensus-invalid.
- No production API may silently bypass spend authorization. The state engine requires a `SpendVerifier` implementation for every normal input.
- M3 does not implement BIP340 Schnorr, KeyCommitV1, address encoding, persistent database storage, mempool policy, or P2P.
- Existing M1/M2 consensus behavior and RandomX vectors must not change.

## File Structure

- Create `crates/oregon-utxo/Cargo.toml` — crate dependencies and metadata.
- Create `crates/oregon-utxo/src/lib.rs` — public exports and integration tests.
- Create `crates/oregon-utxo/src/error.rs` — typed state-transition failures.
- Create `crates/oregon-utxo/src/entry.rs` — `UtxoEntry` metadata and maturity rule.
- Create `crates/oregon-utxo/src/verifier.rs` — mandatory `SpendVerifier` contract.
- Create `crates/oregon-utxo/src/state.rs` — UTXO map, transaction application, block overlay, fee accounting.
- Create `crates/oregon-utxo/src/undo.rs` — reversible block delta.
- Modify `Cargo.toml` — add workspace member.
- Modify `Cargo.lock` only through a verified Cargo resolution step; final CI returns to `--locked`.
- Modify `.github/workflows/oregon-rust.yml` — include M3 branch while executing; keep read-only permissions.
- Create `docs/checkpoints/OREGON_V1_M3_UTXO_CHAINSTATE.md` — final acceptance evidence only after mutation and fresh CI gates.

---

### Task 1: UTXO Entry, Errors, and Mandatory Spend Verifier

**Files:**
- Create: `crates/oregon-utxo/Cargo.toml`
- Create: `crates/oregon-utxo/src/lib.rs`
- Create: `crates/oregon-utxo/src/error.rs`
- Create: `crates/oregon-utxo/src/entry.rs`
- Create: `crates/oregon-utxo/src/verifier.rs`
- Modify: `Cargo.toml`
- Modify: `.github/workflows/oregon-rust.yml`

**Interfaces:**
- Produces: `pub const COINBASE_MATURITY: u64 = 120`.
- Produces: `UtxoEntry { output: TxOutput, creation_height: u64, is_coinbase: bool }`.
- Produces: `UtxoEntry::is_spendable_at(spend_height: u64) -> bool`.
- Produces: `SpendVerifier::verify_spend(&self, transaction: &Transaction, input_index: usize, prevout: &UtxoEntry) -> Result<(), UtxoError>`.
- Produces typed errors including `MissingUtxo`, `DuplicateInput`, `ImmatureCoinbase`, `OutputValueExceedsInput`, `AmountOverflow`, `SpendAuthorizationFailed`, `InvalidBlockOrder`, and `UndoMismatch`.

- [ ] **Step 1: Write RED tests** proving a non-coinbase UTXO is immediately spendable, a coinbase created at height 10 is invalid at spend height 129 and valid at 130, and the trait can return `SpendAuthorizationFailed`.
- [ ] **Step 2: Run `cargo +1.85.0 test -p oregon-utxo`** and verify compilation/test failure is caused by the missing types/API, not Cargo infrastructure.
- [ ] **Step 3: Implement only the types, maturity rule, verifier trait, and errors required by the tests.** Use `creation_height.checked_add(COINBASE_MATURITY)` and treat overflow as not spendable.
- [ ] **Step 4: Run workspace tests, rustfmt check, and clippy `-D warnings`.** Existing M1/M2 suites must remain green.
- [ ] **Step 5: Commit with `feat: add Oregon UTXO entry contract`.** Reviewer checks that no production permissive verifier exists.

### Task 2: Normal Transaction State Transition and Fee Accounting

**Files:**
- Create: `crates/oregon-utxo/src/state.rs`
- Modify: `crates/oregon-utxo/src/lib.rs`
- Modify: `crates/oregon-utxo/src/error.rs`

**Interfaces:**
- Produces: `UtxoState::new()`, `get(&OutPoint)`, and test-visible insertion only through explicit genesis/test fixture helpers under `#[cfg(test)]`.
- Produces: `apply_normal_transaction<V: SpendVerifier>(&mut self, tx: &Transaction, spend_height: u64, verifier: &V) -> Result<u64, UtxoError>` returning fee base units.
- No mutation is committed when validation fails.

- [ ] **Step 1: RED tests** for one valid spend, missing UTXO, duplicate input in one transaction, output sum greater than input sum, checked sum overflow, verifier rejection, and failure atomicity.
- [ ] **Step 2: Verify RED** with the production method absent.
- [ ] **Step 3: Minimal GREEN implementation:** first collect/validate every referenced entry without mutating state; reject duplicate `OutPoint`s with a `HashSet`; check maturity; invoke verifier for every input; checked-sum inputs and outputs; require outputs `<=` inputs; only then remove inputs and insert each new `OutPoint { txid: tx.txid(), index }`.
- [ ] **Step 4: Fresh full workspace CI gate.** Reviewer checks that validation happens before mutation and that witness/signature semantics are delegated, never skipped.
- [ ] **Step 5: Commit `feat: apply Oregon UTXO transactions atomically`.**

### Task 3: Coinbase Outputs and 120-Block Maturity

**Files:**
- Modify: `crates/oregon-utxo/src/state.rs`
- Modify: `crates/oregon-utxo/src/lib.rs`

**Interfaces:**
- Produces: internal `insert_coinbase_outputs(&Transaction, height)` that marks every coinbase output `is_coinbase=true` and rejects collisions.
- Coinbase validation itself remains delegated to existing `oregon_consensus::validate_coinbase` after total block fees are known.

- [ ] **Step 1: RED tests** proving height-1 founder output and miner output are both marked coinbase, both immature for exactly 120 blocks, and a later transaction cannot spend a just-created same-block coinbase output.
- [ ] **Step 2: Verify RED.**
- [ ] **Step 3: Implement coinbase insertion metadata and maturity rejection through the existing normal-spend path.** Do not add founder-specific maturity exemptions.
- [ ] **Step 4: Full workspace tests/fmt/clippy.**
- [ ] **Step 5: Commit `feat: enforce Oregon coinbase maturity in UTXO state`.**

### Task 4: Atomic Block Application, Same-Block Ordering, and Coinbase Fee Binding

**Files:**
- Modify: `crates/oregon-utxo/src/state.rs`
- Create: `crates/oregon-utxo/src/undo.rs`
- Modify: `crates/oregon-utxo/src/lib.rs`
- Modify: `crates/oregon-utxo/src/error.rs`

**Interfaces:**
- Produces: `connect_block<V: SpendVerifier>(&mut self, block: &Block, height: u64, founder_locking_program: &[u8], verifier: &V) -> Result<BlockUndo, UtxoError>`.
- Produces: `BlockUndo { spent: Vec<(OutPoint, UtxoEntry)>, created: Vec<OutPoint> }`.
- Calls `oregon_consensus::validate_non_genesis_block_structure` first.
- Applies normal transactions in canonical block order to a cloned/overlay state, accumulating checked fees.
- Validates coinbase with exact accumulated fees only after all normal transactions validate.
- Inserts coinbase outputs only after fee-bound coinbase validation passes.
- Commits overlay to live state only after the entire block is valid.

- [ ] **Step 1: RED tests** for same-block parent→child spend success, child-before-parent failure, double-spend across two transactions in one block, coinbase overclaim using exact accumulated fees, and whole-block rollback when the final transaction fails.
- [ ] **Step 2: Verify RED.**
- [ ] **Step 3: Implement overlay/clone block connection and `BlockUndo` capture.** Do not partially mutate live state.
- [ ] **Step 4: Run all tests/fmt/clippy and inspect the diff.**
- [ ] **Step 5: Commit `feat: connect Oregon blocks atomically to UTXO state`.**

### Task 5: Deterministic Disconnect / Reorg Undo

**Files:**
- Modify: `crates/oregon-utxo/src/state.rs`
- Modify: `crates/oregon-utxo/src/undo.rs`
- Modify: `crates/oregon-utxo/src/lib.rs`

**Interfaces:**
- Produces: `disconnect_block(&mut self, undo: BlockUndo) -> Result<(), UtxoError>`.
- Disconnect removes all `created` outpoints and restores all `spent` entries exactly.
- Undo fails rather than guessing when a created output is missing or an outpoint to restore already exists.

- [ ] **Step 1: RED tests** for connect→disconnect exact state equality, same-block spend chains restoring only pre-block UTXOs, and tampered undo rejection.
- [ ] **Step 2: Verify RED.**
- [ ] **Step 3: Implement deterministic undo in reverse-safe order.** Validate the whole undo against current state before modifying live state.
- [ ] **Step 4: Full workspace gate.**
- [ ] **Step 5: Commit `feat: add deterministic Oregon block undo`.**

### Task 6: Mutation/Security Acceptance and M3 Checkpoint

**Files:**
- Modify tests only on throwaway mutation branches.
- Create: `docs/checkpoints/OREGON_V1_M3_UTXO_CHAINSTATE.md`

**Interfaces:**
- No new production behavior.

- [ ] **Step 1: Fresh baseline CI** on the clean M3 code head: workspace tests, rustfmt, clippy `-D warnings`.
- [ ] **Step 2: Mutation A** on a throwaway branch: change coinbase maturity comparison so height `creation + 119` is accepted. CI must fail specifically on the 120-block boundary test.
- [ ] **Step 3: Mutation B** on a fresh throwaway branch: remove duplicate-input rejection. CI must fail on duplicate-input/double-spend tests.
- [ ] **Step 4: Mutation C** on a fresh throwaway branch: allow block overlay to commit before final validation. CI must fail the whole-block rollback/state-equality test.
- [ ] **Step 5: Return to clean branch and run fresh full CI again.** Confirm mutation code is absent.
- [ ] **Step 6: Review M2 accepted → M3 diff** for consensus bypasses, unchecked amount arithmetic, production permissive verifier paths, maturity exemptions, nondeterministic map-order semantics, and partial state mutation.
- [ ] **Step 7: Write the checkpoint with observed commit SHAs and CI/mutation run IDs only.** No placeholders.
- [ ] **Step 8: Create recovery branch `oregon-v1-checkpoint-m3-utxo-chainstate-accepted-2026-09-03` from the final acceptance SHA.** Do not merge main.
