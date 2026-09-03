# Oregon v1 M1 Consensus Core Acceptance

Development branch: `oregon-v1-m1-consensus-core`
Plan: `docs/superpowers/plans/2026-09-03-oregon-m1-consensus-core.md`
Golden vector: `tests/vectors/consensus-m1-v1.json`
Golden consumer: `crates/oregon-consensus/tests/golden_vectors.rs`

## Accepted code evidence

- Accepted code SHA: `e53764827d0f23ba0c4f4b9a6ff56d69d3a36a3b`
- Exact-head GitHub Actions run: `33720158447`
- Result: success for workspace tests, rustfmt check, and clippy with warnings denied.
- Golden RED evidence: commit `ec11297a4d7c5b62f35b9aa0c6bbf57658a91df5`, run `33720029133`; the only new golden consumer failed because `tests/vectors/consensus-m1-v1.json` did not exist while the existing 41 consensus tests remained green.

## Mutation sensitivity evidence

Throwaway branch: `oregon-v1-m1-mutation-emission-asert`. Mutation commits are test evidence only and are not merged into the development branch.

### Mutation A — emission halving off-by-one

- Mutation commit: `ad60a405b8fc59bbf2dfafc79c81ed4204adbea1`
- GitHub Actions run: `33720291053`
- Mutation: `(height - 1) / HALVING_INTERVAL` changed to `height / HALVING_INTERVAL`.
- Detection: `emission::tests::exact_halving_boundaries` failed; height `200000` produced `118750000` base units instead of `237500000`.

### Mutation B — ASERT half-life

- Mutation commit: `08feb91381fdb2a71d3defa6d5f18270a8f3c934`
- GitHub Actions run: `33720376563`
- Mutation: ASERT implementation half-life changed from `21600` to `21601` seconds while tests/vectors remained unchanged.
- Detection: `asert::tests::half_half_life_late_is_frozen`, `asert::tests::one_half_life_early_halves`, and `asert::tests::one_half_life_late_doubles` failed.

These failures demonstrate that the accepted tests are sensitive to the two deliberately weakened consensus rules required by the M1 plan.

## Accepted M1 consensus surface

M1 accepts the following consensus-core behavior:

- 256-bit nonzero little-endian target representation and `pow_limit` validation.
- Monetary emission with genesis subsidy zero, `2.375 OREG` initial subsidy, `200000`-block halvings, exact scheduled mining issuance `949999.97 OREG`, and the frozen supply envelope.
- Canonical coinbase form, canonical height witness, exact one-time founder allocation at height 1, miner reward upper bound of subsidy plus supplied fees, and no later founder mint.
- Per-block fixed-point ASERT using a 300-second target spacing and 21600-second half-life, with parent timestamp as the difficulty time input and deterministic target clamps.
- Median Time Past over one through eleven predecessor timestamps, using the sorted upper median for even early windows.
- Exact chain-work contribution `2^256 / (target + 1)`.
- Pre-PoW header-context validation for parent linkage, timestamp strictly above MTP, required ASERT target, nonzero/within-limit target, and work facts. This API does not validate a RandomX proof.
- Non-genesis block structural validation: maximum encoded block size `1048576` bytes, maximum encoded transaction size `102400` bytes, frozen v0 Merkle root reuse, exactly one coinbase at index zero, and no null outpoint in ordinary transactions.
- Public consensus golden vectors for target encoding, emission boundaries, ASERT vectors, and work values.

## Explicitly excluded from M1

M1 does not claim or implement:

- RandomX hashing, seed scheduling, or proof-of-work hash verification — M2.
- UTXO existence/spend state, signatures, script/locking-program execution, double-spend detection, fee derivation, or coinbase maturity — M3.
- Persistent chainstate, storage, reorg application/rollback, or crash recovery — M4.
- Final genesis construction, address/network profile, launch artifacts, or chain identity freeze — M5.
- P2P networking, synchronization, peer policy, mempool, mining RPC/template service, wallet, node operations, or later subsystems.

## Acceptance rule

This checkpoint is valid only together with a final documentation-head GitHub Actions run whose `head_sha` is the final checkpoint commit and whose test, format, and clippy steps all succeed. The final recovery branch must be created from that exact successful head. Until that run exists, this document records the accepted code evidence but does not by itself declare the final documentation head accepted.
