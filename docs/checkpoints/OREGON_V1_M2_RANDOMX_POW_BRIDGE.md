# Oregon v1 M2 RandomX PoW Bridge Checkpoint

Date: 2026-09-03
Branch: `oregon-v1-m2-randomx-pow-exec`
Accepted bridge commit: `017f4139290ac78dc5234882914db2356de36945`
Current parity-accepted commit: `c97d165e405b1151fecdd0b8be50d92845150d12`

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
- LightEngine remains the portable consensus validation path.
- FullEngine initializes the complete RandomX dataset, binds the same immutable key, and uses recommended host CPU flags only as a performance optimization.
- Normal CI is read-only and uses the pinned Cargo.lock with `--locked`.

## Consensus bridge evidence

Bridge CI run `33725750221` passed workspace tests, rustfmt and clippy with warnings denied.

## Frozen architecture vector

The frozen vector commits to the complete Oregon path from key-block identity through RandomX output:

- key block ID: `4444444444444444444444444444444444444444444444444444444444444444`
- derived key: `f4b6344379c73549e3673e73fe8b43a6dba0df462f4bf1f7c1aa147731e959dc`
- PoW input length: `128`
- RandomX hash: `c33bcaf498accad910ed40a346ac3820700496b2ead640ead6892cb01332143c`

Architecture parity evidence:
- first native x64 measurement run: `33726625242`
- x64 + ARM64 frozen-vector proof run: `33726766050`
- final parity-accepted exact-head vector run: `33727492548`
- result: x64 SUCCESS and ARM64 SUCCESS

## Full / Light parity evidence

At exact commit `c97d165e405b1151fecdd0b8be50d92845150d12`:

- normal Rust CI run `33727492678`: tests SUCCESS, rustfmt SUCCESS, clippy SUCCESS
- FullEngine vs LightEngine native parity run `33727492545`: x64 SUCCESS and ARM64 SUCCESS
- both engines use the same Oregon key and canonical PoW input and produce the same RandomX hash
- the full-dataset path allocates and initializes the complete RandomX dataset; it is not a mocked or reduced-memory substitute

## Mutation evidence

Endian mutation evidence:
- mutation branch: `oregon-v1-m2-mutation-endian-2026-09-03`
- mutation commit: `77ee893610760c5008aa5c08bee051cb368be2e4`
- CI trigger commit: `0849726b409fd7a52ef954785a6e19eb9ea4dfdf`
- CI run: `33726007274`
- result: expected failure
- exact caught test: `tests::randomx_hash_target_comparison_is_little_endian`
- the deliberately incorrect big-endian comparator accepted the crafted `above` hash; the test rejected that mutation.

The accepted execution branch and recovery checkpoints never contained the bad-endian mutation.

This is an M2 parity recovery checkpoint. Final M2 acceptance still requires diff/security review and final exact-head verification.
