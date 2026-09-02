# Oregon v0 Protocol Progress

Plan: `docs/superpowers/plans/2026-09-02-oregon-v0-protocol-foundation.md`
Spec: `docs/superpowers/specs/2026-09-02-oregon-v0-protocol-design.md`
Development branch: `oregon-v0-protocol`

## Recovery checkpoint

Verified checkpoint branch: `oregon-v0-checkpoint-task4-green-2026-09-02`
Verified commit: `6b3bfb6f3a8c21cd66833c3456a0e520fc03930b`
GitHub Actions run: `33663066004` (`Oregon Rust CI`, success)

## Task status

- Task 1 — Workspace and Amount Safety: complete and CI-verified.
- Task 2 — Hash256 and domain-separated object hashing: complete and CI-verified.
- Task 3 — Canonical integer encoding and bounded decoder: complete and CI-verified.
- Task 4 — Transaction primitive, canonical bytes, and TxID: base implementation complete and CI-verified at the recovery checkpoint. Remaining before Task 4 acceptance: hostile-length tests, truncation/trailing-byte tests, bounded property round-trip tests, and final full CI verification.
- Task 5 — Merkle commitment, block header, and block ID: not started.
- Task 6 — Protocol-v0 golden vectors and foundation acceptance: not started.

## Protocol decisions already fixed

- Rust 1.85.0, edition 2024.
- `1 OREG = 100,000,000` base units.
- Maximum supply envelope: `1,000,000 OREG`.
- Public founder allocation constant: `50,000 OREG` (5%).
- Transaction IDs use domain-separated BLAKE3 with `OREGON/TX/V0\0` and commit to witness bytes.
- Consensus binary encoding is explicit and canonical; no generic serde/bincode codec.
- Defensive parsing uses `DecodeLimits`; hostile input must return typed errors rather than panic.

Do not delete the recovery checkpoint branch while the foundation milestone is in progress.
