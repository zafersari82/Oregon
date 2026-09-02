# Oregon v0 Protocol Progress

Plan: `docs/superpowers/plans/2026-09-02-oregon-v0-protocol-foundation.md`
Spec: `docs/superpowers/specs/2026-09-02-oregon-v0-protocol-design.md`
Development branch: `oregon-v0-protocol`

## Recovery checkpoints

Base Task 4 implementation checkpoint:
- Branch: `oregon-v0-checkpoint-task4-green-2026-09-02`
- Commit: `6b3bfb6f3a8c21cd66833c3456a0e520fc03930b`
- GitHub Actions run: `33663066004` (`Oregon Rust CI`, success)

Task 4 accepted checkpoint:
- Branch: `oregon-v0-checkpoint-task4-accepted-2026-09-02`
- Commit: `330a8c47f4bc0b6e74109485ce94591a727fe2e2`
- GitHub Actions run: `33663994561` (`Oregon Rust CI`, success)
- Coverage at acceptance: 29 unit/property tests, including configured decode limits, truncation, trailing bytes, witness-to-txid commitment, canonical round-trip, and arbitrary hostile-byte no-panic coverage.

Task 5 accepted checkpoint:
- Branch: `oregon-v0-checkpoint-task5-accepted-2026-09-02`
- Commit: `4418f42581c0904e5c4a48c886828ef1386741ed`
- GitHub Actions run: `33665201443` (`Oregon Rust CI`, success)
- Coverage at acceptance: 44 unit/property tests total. Merkle tests prove ordered leaves and odd-node promotion without duplicate-last behavior. Block tests cover 114-byte header round-trip, header-only block identity, block round-trip, transaction-count/object limits, truncation, trailing bytes, and arbitrary hostile-byte no-panic behavior.

Task 6 accepted checkpoint:
- Branch: `oregon-v0-checkpoint-task6-accepted-2026-09-02`
- Commit: `29576e09948670853ca24c3edf40b529ccc8b60a`
- GitHub Actions run: `33666671802` (`Oregon Rust CI`, success)
- Golden fixture: `tests/vectors/protocol-v0.json`
- Golden coverage: canonical varint boundaries, explicit non-minimal varints, minimum and multi-input/output/witness transactions, exact canonical transaction bytes and TxIDs, one/two/three-transaction Merkle roots, exact canonical block-header bytes and block ID, and maximum/above-maximum amount boundaries.
- Mutation-sensitivity evidence: branch `oregon-v0-mutation-odd-merkle-2026-09-02`, mutation commit `cc1d84b297fa6557625b0afcb2737ea229bfb5c4`, CI trigger commit `ca3ee725fc1e146893b140adff6352489fa06081`, GitHub Actions run `33666851003` failed exactly at `merkle::tests::three_transaction_root_promotes_last_leaf_without_duplication` after intentionally replacing odd-node promotion with duplicate-last hashing. The development branch was never mutated.

Foundation acceptance record:
- File: `docs/checkpoints/OREGON_V0_PROTOCOL_FOUNDATION.md`
- Fresh pre-checkpoint full gate: commit `aed2e932a485f1e987b3024e3cc657e7c3ad544b`, GitHub Actions run `33667003179` (`Oregon Rust CI`, success)
- Final recovery branch name: `oregon-v0-checkpoint-foundation-accepted-2026-09-02`
- The final recovery branch is created only after the exact final development-head commit passes the branch CI gate.

## Task status

- Task 1 — Workspace and Amount Safety: complete and CI-verified.
- Task 2 — Hash256 and domain-separated object hashing: complete and CI-verified.
- Task 3 — Canonical integer encoding and bounded decoder: complete and CI-verified.
- Task 4 — Transaction primitive, canonical bytes, and TxID: complete and CI-verified.
- Task 5 — Merkle commitment, block header, and block ID: complete and CI-verified.
- Task 6 — Protocol-v0 golden vectors: complete and CI-verified, including mutation sensitivity.
- Task 7 — Foundation acceptance record and independence review: complete pending the final checkpoint commit's CI gate.

## Protocol decisions already fixed

- Rust 1.85.0, edition 2024.
- `1 OREG = 100,000,000` base units.
- Maximum supply envelope: `1,000,000 OREG`.
- Public founder allocation constant: `50,000 OREG` (5%).
- Transaction IDs use domain-separated BLAKE3 with `OREGON/TX/V0\0` and commit to witness bytes.
- Block IDs use domain-separated BLAKE3 with `OREGON/BLOCK/V0\0` over canonical 114-byte headers only.
- Merkle leaves use `OREGON/MERKLE-LEAF/V0\0`; internal nodes use `OREGON/MERKLE/V0\0`; odd terminal nodes are promoted unchanged.
- Consensus binary encoding is explicit and canonical; no generic serde/bincode codec.
- Defensive parsing uses `DecodeLimits`; hostile input must return typed errors rather than panic.
- The deterministic vector generator remains only as a reproducibility aid; the checked-in golden JSON is the protocol artifact consumed by acceptance tests.

Do not delete the recovery checkpoint branches while later protocol milestones are in progress.
