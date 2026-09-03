# Oregon v1 M2 RandomX PoW Bridge Checkpoint

Date: 2026-09-03
Branch: `oregon-v1-m2-randomx-pow-exec`
Accepted bridge commit: `017f4139290ac78dc5234882914db2356de36945`
Accepted CI run: `33725750221`

## Frozen at this checkpoint

- RandomX upstream is pinned to `aaafe71322df6602c21a5c72937ac284724ae561`.
- Oregon RandomX Argon salt is `OREGON-RANDOMX-V1` and is applied only to the build copy.
- Key epoch is 864 blocks with a 24-block activation delay.
- RandomX keys are BLAKE3 over `OREGON/RANDOMX-KEY/V1\0 || key_block_id`.
- PoW input is `OREGON/POW/V1\0 || canonical 114-byte block header`.
- RandomX output is interpreted as an unsigned little-endian 256-bit integer.
- Header `difficulty_commitment` is the full little-endian 256-bit target.
- Consensus validates candidate-height/key-height schedule consistency before hashing.
- Consensus rejects an engine bound to a different RandomX key before hashing.
- Consensus computes the RandomX hash itself and rejects insufficient proof of work.
- Normal CI is read-only and uses the pinned Cargo.lock with `--locked`.

## Evidence

CI run `33725750221` passed workspace tests, rustfmt and clippy with warnings denied.

This is an intermediate M2 recovery checkpoint, not final M2 acceptance. Full/light parity, architecture vectors and mutation evidence remain to be completed.
