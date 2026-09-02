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

## Task status

- Task 1 — Workspace and Amount Safety: complete and CI-verified.
- Task 2 — Hash256 and domain-separated object hashing: complete and CI-verified.
- Task 3 — Canonical integer encoding and bounded decoder: complete and CI-verified.
- Task 4 — Transaction primitive, canonical bytes, and TxID: complete and CI-verified.
- Task 5 — Merkle commitment, block header, and block ID: complete and CI-verified.
- Task 6 — Protocol-v0 golden vectors and foundation acceptance: next active task.

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

Do not delete the recovery checkpoint branches while the foundation milestone is in progress.
