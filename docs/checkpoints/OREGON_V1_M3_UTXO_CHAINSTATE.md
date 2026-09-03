# Oregon v1 M3 UTXO Chainstate Checkpoint

Date: 2026-09-03
Branch: `oregon-v1-m3-utxo-chainstate`
M2 accepted recovery base: `ee83a3062e06b9447d091872fe77bd37eeee1f4d`
M3 final reviewed code commit: `e587f125c5ba5712baf4b1d3039a89b76cdc620c`

## Accepted scope

M3 adds the consensus-facing UTXO state-transition engine while preserving the accepted M1/M2 consensus and RandomX behavior.

Frozen M3 behavior:

- UTXO state is outpoint-based; no account-balance consensus model is introduced.
- Every normal transaction requires at least one input and one output at the non-genesis block skeleton boundary.
- Normal transaction inputs reject missing UTXOs and duplicate outpoints.
- Every normal input is routed through the caller-supplied `SpendVerifier`; there is no production permissive verifier implementation.
- Input and output sums use checked integer base-unit arithmetic.
- Transaction outputs may not exceed referenced input value; returned fee is `inputs - outputs`.
- Transaction failure does not partially mutate live UTXO state.
- Coinbase outputs, including the height-1 founder allocation, use the same coinbase metadata and exactly 120-block maturity rule.
- Coinbase maturity uses checked height addition and has no founder-specific exemption.
- Same-block normal spends are allowed only from outputs created by an earlier normal transaction in canonical block order.
- Child-before-parent and in-block double-spend attempts are consensus-invalid.
- Block normal transactions are applied to an overlay, fees are accumulated with checked arithmetic, and coinbase is validated against the exact accumulated fees before live-state commit.
- Invalid blocks do not leak partial overlay changes into live state.
- `BlockUndo` records pre-block spent entries and newly-created surviving outpoints needed for deterministic reversal.
- Undo created/spent collections are canonicalized by outpoint ordering rather than relying on `HashMap` iteration order.
- Disconnect validates the complete undo against current state before modifying live state and rejects tampered, duplicate, missing, or colliding undo data with `UndoMismatch`.

## Fresh clean CI evidence

Pre-mutation clean gate at code commit `527112bf3bbde9b403af6c7e73e292704226e104`:

- Oregon Rust CI run `33753572817` (#203)
- workspace tests: SUCCESS
- rustfmt: SUCCESS
- clippy with warnings denied: SUCCESS

Post-mutation final clean gate at exact reviewed code commit `e587f125c5ba5712baf4b1d3039a89b76cdc620c`:

- Oregon Rust CI run `33765642218` (#207)
- workspace tests: SUCCESS
- rustfmt: SUCCESS
- clippy with warnings denied: SUCCESS

The post-mutation commit uses the same source tree as the preceding clean code head and exists to provide fresh acceptance evidence after all throwaway mutation runs.

## Mutation / security acceptance evidence

### Mutation A — coinbase maturity off-by-one

- throwaway branch: `mutation-m3-maturity-off-by-one-2026-09-03`
- mutation commit: `c375d1a8793313d1305cadc774aa200dac383788`
- CI trigger commit: `2d6655eee6a8aed2f28c29713666c9e055dd02ae`
- Oregon Rust CI run `33756303835` (#204)
- result: expected FAILURE
- caught boundary tests:
  - `tests::coinbase_requires_exactly_120_blocks_of_maturity`
  - `state::coinbase_tests::founder_and_miner_outputs_share_coinbase_metadata_and_maturity`
- observed mutation symptom: heights `creation + 119` became spendable and the boundary assertions rejected it.

### Mutation B — duplicate-input rejection removed

- throwaway branch: `mutation-m3-duplicate-input-2026-09-03`
- mutation commit: `2ad93e918ea69a182ffadf9244be471653ea87cd`
- CI trigger commit: `18dcee86dfd48654d6feeac1dbda218779243742`
- Oregon Rust CI run `33765244606` (#205)
- result: expected FAILURE
- exact caught test: `tests::duplicate_input_is_rejected_without_state_change`
- observed mutation symptom: the duplicated 100-unit input was counted twice and the mutated path returned `Ok(110)` instead of `DuplicateInput`.

### Mutation C — block overlay committed before final validation

- throwaway branch: `mutation-m3-early-overlay-commit-2026-09-03`
- mutation commit: `afb0a1fe208ab3759e5e64e320796ce2bdb459dd`
- CI trigger commit: `dc3abbee3f1eb82aae85e0067912b81d9f78e213`
- Oregon Rust CI run `33765482838` (#206)
- result: expected FAILURE
- required caught test: `block_tests::final_invalid_transaction_rolls_back_all_earlier_overlay_changes`
- additional state/undo tests also failed because the mutation leaked intermediate state into the live UTXO set.

All three mutations remained isolated on throwaway branches. The accepted M3 branch retained duplicate-input rejection, exact 120-block maturity, and final-only overlay commit semantics before the post-mutation clean gate.

## M2 → M3 review disposition

Reviewed range:

- accepted M2 recovery base: `ee83a3062e06b9447d091872fe77bd37eeee1f4d`
- reviewed M3 code head: `e587f125c5ba5712baf4b1d3039a89b76cdc620c`
- relation: M3 is 44 commits ahead and 0 commits behind the accepted M2 recovery base.

Manual security/code review covered:

- consensus-validation bypass paths
- mandatory spend authorization boundary
- checked amount and fee arithmetic
- founder/coinbase maturity exemptions
- duplicate input and same-block double-spend handling
- same-block topological ordering
- exact-fee coinbase binding
- whole-block atomicity and partial-state leakage
- deterministic `BlockUndo` ordering and disconnect validation
- production dependency and CI changes

Review result:

- no production permissive `SpendVerifier` implementation found
- no founder-specific maturity exemption found
- no unchecked UTXO input/output or accumulated-fee addition path found in the reviewed state transition
- no live-state block commit before final fee-bound coinbase validation found
- no consensus-visible dependence on `HashMap` iteration order found in undo generation
- no known Critical or Important M3 review finding remains open at this checkpoint

## Acceptance

M3 UTXO chainstate is accepted at reviewed code commit `e587f125c5ba5712baf4b1d3039a89b76cdc620c` with fresh post-mutation workspace tests, rustfmt and clippy evidence, plus three targeted security mutations that were each killed by the intended state-transition tests.

This acceptance covers the M3 UTXO chainstate only. It does not claim completion of persistent chain storage, full Schnorr/KeyCommitV1 spend cryptography, mempool policy, P2P networking, node synchronization, RPC/miner integration, wallet/address encoding, testnet, genesis launch, or mainnet readiness.
