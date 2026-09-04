# Oregon v1 M5 Mempool Checkpoint

Date: 2026-09-04
Branch: `oregon-v1-m5-mempool`
Accepted M4 base: `6ff8168bb79b0f7e1aa015ce910cedaf108614ae`
M5 final reviewed code commit: `1faf667eec4974d3b9b6e14cd1d22f59f71452d1`

## Accepted scope

M5 adds a policy-only in-memory transaction mempool and deterministic admission/reconciliation layer while preserving the accepted M1-M4 consensus, RandomX, UTXO and persistent-chainstate behavior.

Frozen M5 behavior:

- `oregon-mempool` owns in-memory transaction admission, dependency tracking, deterministic ordering, bounded capacity, eviction and chain-tip/reorg reconciliation.
- Canonical transaction identity and size are reused from `Transaction::txid()` and `Transaction::encode()`; M5 does not define a second transaction encoding or id.
- Every admitted spend remains subject to the existing mandatory `SpendVerifier` boundary through `UtxoState::apply_normal_transaction()`.
- Admission uses next-block context, `tip_height.checked_add(1)`, so exact 120-block coinbase maturity semantics remain inherited from M3.
- A txid already present in the pool is rejected.
- At most one mempool transaction may claim an outpoint. M5 has no RBF path; a conflicting spend is rejected rather than replacing the existing claimant.
- Unconfirmed parent-before-child dependencies are supported. Child-before-parent or otherwise missing dependencies are rejected and are not retained as orphans.
- Parent output indexes are checked before dependency admission.
- Dependency traversal and ready-set ordering are deterministic; topological ordering requires parents before children and uses ascending txid among simultaneously ready transactions.
- Default hard policy limits are 50,000 entries, 64 MiB canonical transaction bytes, 25 unconfirmed ancestors and 25 unconfirmed descendants. Ancestor/descendant counts exclude the transaction itself.
- Capacity planning is staged before live publication. Exact-limit equality is allowed; exceeding a count or byte limit invokes deterministic eviction.
- Eviction ordering uses exact integer fee-rate comparison by cross multiplication, then fee, then txid; no floating-point ordering is used.
- Evicting a parent removes its entire descendant subtree.
- A candidate cannot satisfy capacity by evicting itself or an ancestor required by that candidate; such admission fails with `CapacityRejected` without live-state mutation.
- Reconciliation after an accepted active block is staged. Confirmed mempool transactions are removed, children of confirmed parents can be promoted to chain-backed inputs, and transactions conflicting with the active block plus their descendants are removed.
- Reorg reconciliation rebuilds only from transactions already present in the current mempool. Transactions from disconnected blocks are not implicitly resurrected.
- Rebuild/reconciliation publishes the new pool only after the staged pool is completely rebuilt; invariant/cycle/height failures preserve the previous live pool.
- Reconciliation and reorg outputs are deterministic across independent mempool insertion order and chain-UTXO insertion order.
- Zero-fee transactions are valid policy when all other rules and capacity constraints pass.
- Witness bytes remain part of canonical transaction identity/size; changing witness changes the txid and canonical encoded size used by the mempool.
- `oregon-mempool` depends only on `oregon-consensus`, `oregon-primitives`, `oregon-utxo` and `thiserror`; it does not depend on `oregon-storage` or `oregon-chainstate`.
- The M5 crate forbids unsafe Rust.

M5 intentionally does not add P2P networking, orphan storage, RBF, package relay, CPFP/package scoring, mempool persistence, wallet/address encoding, mining RPC, production spend cryptography beyond the existing verifier boundary, testnet/genesis launch or mainnet readiness.

## Recovery / deterministic acceptance matrix

The completed M5 test matrix covers, together with the existing M1-M4 tests:

- exact default policy limits and invalid zero entry/byte capacity
- valid chain-backed admission with canonical fee/size recording
- zero-fee admission
- witness-sensitive txid and encoded-byte accounting
- duplicate txid rejection without state mutation
- conflicting-spend rejection without state mutation
- missing dependency rejection without orphan retention
- structural rejection and mandatory verifier rejection without mutation
- stale chain-base rejection
- checked next-height overflow
- exact 120-block coinbase maturity through next-block context
- valid parent -> child admission
- child-before-parent rejection
- invalid parent-output-index rejection
- exact ancestor and descendant boundaries
- insertion-order-independent topological ordering
- exact entry/byte capacity equality and deterministic eviction after exceedance
- parent subtree eviction
- candidate-self and candidate-ancestor non-eviction atomicity
- active-block confirmed-parent promotion
- active-block conflict + descendant removal
- ordinary active-tip filtering
- reorg retain/filter behavior
- no implicit resurrection of disconnected transactions absent from the current pool
- stale admission until reorg reconciliation publishes the new base
- reorg insertion-order determinism
- UTXO insertion-order reorg determinism
- checked reorg-height overflow
- rejecting-verifier filtering during rebuild
- internal graph invariant failure atomicity
- internal dependency-cycle reorg atomicity

## Clean CI evidence

Exact reviewed code commit: `1faf667eec4974d3b9b6e14cd1d22f59f71452d1`

### Pre-mutation clean gate

Oregon Rust CI run `33845806483`, attempt 1:

- workspace tests: SUCCESS
- rustfmt: SUCCESS
- clippy with `-D warnings`: SUCCESS

### Post-mutation clean gate

The exact same clean SHA was re-run after all three required mutation experiments.

Oregon Rust CI run `33845806483`, attempt 2, job `100940461004`:

- workspace tests: SUCCESS
- rustfmt: SUCCESS
- clippy with `-D warnings`: SUCCESS

No mutation source or mutation-only workflow/script commit is in the `oregon-v1-m5-mempool` branch at this reviewed SHA.

## Required security mutation evidence

All mutation behavior remained isolated on throwaway branches created from clean M5 commit `1faf667eec4974d3b9b6e14cd1d22f59f71452d1`.

### Mutation A — conflict ownership bypass

Branch: `mutation-m5-conflict-bypass-2026-09-04`

Behavioral mutation commit: `ba2cd2c863c24b9e1b4b0146b69bbc03f807da71`
Targeted CI head: `ef91480e700cb51c1c0f47b8e827b79902c0848b`
Oregon Rust CI run: `33846652264`
Job: `100939921369`

Result: expected FAILURE.

Killed by:

- `conflicting_spend_rejection_is_atomic`

Observed symptom: after conflict ownership checks were intentionally bypassed, the second transaction spending the already-claimed outpoint returned `Ok(AdmissionOutcome)` instead of the required `Err(Conflict)`. The test therefore detects actual conflicting-spend acceptance, not merely an error-code change.

### Mutation B — missing-parent admission bypass

Branch: `mutation-m5-missing-parent-bypass-2026-09-04`

The first branch-only workflow draft failed before creating a job because of workflow syntax and is explicitly excluded from mutation evidence. The corrected mutation used a throwaway injection script and reached the real test step.

Targeted CI head: `7213997ac650f134a0e336520b7e08d8e94f9204`
Oregon Rust CI run: `33846666133`
Job: `100939964441`

Result: expected FAILURE.

Killed by:

- `missing_dependency_rejection_is_atomic`

Observed symptom: after missing-parent rejection was intentionally bypassed, a transaction with an unavailable outpoint returned `Ok(AdmissionOutcome)` with fee 0 instead of `Err(MissingDependency)`. The test therefore directly guards against orphan/missing-parent acceptance.

### Mutation C — early live publication before capacity decision

Branch: `mutation-m5-early-publication-2026-09-04`

Targeted CI head: `c7f492172aa24fe63e6bf4722feb2ab1a0e31236`
Oregon Rust CI run: `33846679300`
Job: `100940005850`

Result: expected FAILURE.

Killed by:

- `candidate_self_eviction_rejects_without_any_public_mutation`

Observed symptom: the mutation published candidate byte accounting before capacity acceptance. A rejected candidate left live `total_bytes` at 189 instead of the previous 126 while the stored-entry set remained otherwise unchanged. The rollback test detected the partial publication exactly.

These three failures bind M5's core security properties to tests: one-owner-per-outpoint, missing-parent rejection, and no partial live publication before admission succeeds.

## M4 -> M5 manual security review

Reviewed range:

- accepted M4 base: `6ff8168bb79b0f7e1aa015ce910cedaf108614ae`
- reviewed M5 code: `1faf667eec4974d3b9b6e14cd1d22f59f71452d1`
- relation: M5 is 49 commits ahead and 0 commits behind the accepted M4 base; merge base is exactly the accepted M4 SHA

Review covered:

- M4 consensus-structure behavior while extracting the reusable normal-transaction structural validator
- canonical txid/encoding reuse
- mandatory `SpendVerifier` enforcement during candidate admission, ancestor replay and rebuild
- next-block coinbase-maturity context
- duplicate and conflicting spend handling
- missing-parent/orphan policy and parent-output validation
- ancestor/descendant accounting and graph reciprocity/cycle checks
- deterministic topological ordering
- exact integer eviction comparison and tie-breakers
- subtree eviction and candidate-ancestor protection
- checked entry/byte accounting
- plan/preflight/commit publication order
- active-block reconciliation and confirmed-parent promotion
- reorg-only-current-pool rule and non-resurrection
- staged rebuild publication and failure atomicity
- UTXO/mempool insertion-order independence
- dependency boundaries and unsafe-code prohibition
- regression risk to M4 storage/chainstate

Review disposition:

- M5's reusable normal-transaction skeleton helper preserves the block-level structural checks while exposing the same rules to mempool admission.
- No M4 storage, chainstate or UTXO implementation source file is modified by the M5 reviewed diff.
- The new mempool crate has no dependency on storage or chainstate and is policy-only/in-memory.
- No alternate transaction identity or encoding was introduced.
- No RBF, orphan retention or disconnected-block resurrection path exists in the accepted M5 scope.
- Exact integer arithmetic is used for eviction comparison; no floating-point nondeterminism was introduced.
- Live pool publication occurs after staged planning/preflight/rebuild success in the accepted code.
- Required security mutations are all killed by dedicated tests, and the exact clean SHA passes the full workspace gate again after the mutation experiments.
- No known Critical or Important M5 security finding remains open at this checkpoint.

## Acceptance

M5 is accepted as the policy-only in-memory mempool layer at reviewed code tree `1faf667eec4974d3b9b6e14cd1d22f59f71452d1`, subject to the checkpoint commit itself passing the same full Oregon Rust CI gate.

Acceptance includes deterministic transaction admission, dependency graph handling, one-spender conflict policy, exact bounded capacity/eviction, active-block reconciliation, staged reorg revalidation and the recovery/mutation evidence described above.

This acceptance does **not** claim completion of P2P networking, network transaction relay, orphan storage, RBF/package relay/CPFP, mempool persistence, wallet/address encoding, mining RPC, production Schnorr/KeyCommitV1 spend cryptography beyond the existing verifier boundary, testnet, genesis launch or mainnet readiness.

`main` was not merged or modified as part of M5 implementation, mutation testing or checkpoint preparation.
