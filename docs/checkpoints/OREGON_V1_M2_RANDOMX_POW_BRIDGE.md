# Oregon v1 M2 RandomX PoW Bridge Checkpoint

Date: 2026-09-03
Branch: `oregon-v1-m2-randomx-pow-exec`
Initial bridge commit: `017f4139290ac78dc5234882914db2356de36945`
Final reviewed code commit: `44d22ae112b9182cf054aa9faa3426a66770b7ae`

## Frozen at this checkpoint

- RandomX upstream is pinned to `aaafe71322df6602c21a5c72937ac284724ae561`.
- Oregon RandomX Argon salt is `OREGON-RANDOMX-V1` and is applied only to the build copy.
- Key epoch is 864 blocks with a 24-block activation delay.
- RandomX keys are BLAKE3 over `OREGON/RANDOMX-KEY/V1\0 || key_block_id`.
- PoW input is `OREGON/POW/V1\0 || canonical 114-byte block header`.
- RandomX output is interpreted as an unsigned little-endian 256-bit integer.
- Header `difficulty_commitment` is the full little-endian 256-bit target.
- Consensus pre-validation enforces parent linkage, MTP, ASERT-required target and POW_LIMIT before RandomX validation.
- `PrePowHeaderFacts` is an opaque header-bound token containing candidate height, exact header ID, validated target and block work.
- RandomX validation rejects facts created for a different header and consumes the target from the pre-validation token rather than trusting the candidate target independently.
- The RandomX key-block height is computed by consensus from the prevalidated candidate height.
- The required key-block ID is retrieved only through `PowKeyBlockSource` from an already-validated active chain; callers do not inject an arbitrary key-block ID into the validator.
- Consensus rejects an engine bound to a different RandomX key before hashing.
- Consensus computes the RandomX hash itself and rejects insufficient proof of work.
- LightEngine remains the portable validation path.
- FullEngine initializes the complete RandomX dataset, binds the same immutable key, and uses recommended host CPU flags only as a performance optimization.
- The `oregon-pow` crate explicitly documents that it is a hashing engine, not a standalone consensus validator.
- Normal CI is read-only, uses the pinned Cargo.lock with `--locked`, and pins `actions/checkout` v7.0.1 by immutable commit `3d3c42e5aac5ba805825da76410c181273ba90b1`.

## Frozen architecture vector

The frozen vector commits to the complete Oregon path from key-block identity through RandomX output:

- key block ID: `4444444444444444444444444444444444444444444444444444444444444444`
- derived key: `f4b6344379c73549e3673e73fe8b43a6dba0df462f4bf1f7c1aa147731e959dc`
- PoW input length: `128`
- RandomX hash: `c33bcaf498accad910ed40a346ac3820700496b2ead640ead6892cb01332143c`

Historical architecture evidence:
- first native x64 measurement run: `33726625242`
- x64 + ARM64 frozen-vector proof run: `33726766050`
- parity checkpoint vector run: `33727492548`

Final reviewed-code architecture evidence at `44d22ae112b9182cf054aa9faa3426a66770b7ae`:
- run `33729550956`
- x64: SUCCESS
- ARM64: SUCCESS

## Full / Light parity evidence

Historical parity checkpoint at `c97d165e405b1151fecdd0b8be50d92845150d12`:
- normal Rust CI run `33727492678`: tests SUCCESS, rustfmt SUCCESS, clippy SUCCESS
- FullEngine vs LightEngine native parity run `33727492545`: x64 SUCCESS and ARM64 SUCCESS

Final reviewed-code parity at `44d22ae112b9182cf054aa9faa3426a66770b7ae`:
- FullEngine vs LightEngine run `33729551071`
- x64: SUCCESS
- ARM64: SUCCESS
- both engines use the same Oregon key and canonical PoW input and produce the same frozen RandomX hash
- the full-dataset path allocates and initializes the complete RandomX dataset; it is not a mocked or reduced-memory substitute

## Final normal CI evidence

At exact reviewed code commit `44d22ae112b9182cf054aa9faa3426a66770b7ae`:
- Oregon Rust CI run `33729550987`
- workspace tests: SUCCESS
- rustfmt: SUCCESS
- clippy with warnings denied: SUCCESS

The immediately preceding functional GREEN commit `f1781a7a4c642b6a662d05827fd730c8cf033da1` also passed fresh normal CI in run `33729398164`.

## Mutation / RED-GREEN evidence

Endian mutation evidence:
- mutation branch: `oregon-v1-m2-mutation-endian-2026-09-03`
- mutation commit: `77ee893610760c5008aa5c08bee051cb368be2e4`
- CI trigger commit: `0849726b409fd7a52ef954785a6e19eb9ea4dfdf`
- CI run: `33726007274`
- result: expected failure
- exact caught test: `tests::randomx_hash_target_comparison_is_little_endian`
- the deliberately incorrect big-endian comparator accepted the crafted `above` hash; the test rejected that mutation.

Validated-chain key source review fix:
- review finding: the earlier bridge accepted a caller-provided key-block ID even though it checked the scheduled height.
- RED commit: `5b2c082d0896115a4bc6b97007565bcd8a803172`
- RED CI run: `33727888150`
- expected failure: `PowKeyBlockSource` did not yet exist.
- GREEN behavior commit family culminated in `4737ec58f5e8fea8eb2e960e640755da0596d5e7`.
- GREEN CI run: `33728320480`, tests/rustfmt/clippy SUCCESS.

Pre-PoW typestate review fix:
- review finding: the earlier API could be called with only a candidate height and independently trusted header target, allowing callers to bypass the intended ASERT/POW_LIMIT pre-validation ordering.
- RED commit: `5dd499795c1c31ce1d72f3cb29362db28d403de2`
- RED CI run: `33728486479`
- expected failure: the old API still expected a raw height and had no `PowPrevalidationMismatch` path.
- GREEN implementation culminated in `f1781a7a4c642b6a662d05827fd730c8cf033da1`.
- GREEN CI run: `33729398164`, tests/rustfmt/clippy SUCCESS.
- final reviewed code commit `44d22ae112b9182cf054aa9faa3426a66770b7ae` repeated all normal and architecture/full-light gates successfully.

The accepted execution branch and recovery checkpoint never contained the bad-endian mutation.

## Review disposition

Manual security/code review covered:
- RandomX FFI ownership and Drop order
- Full vs Light engine parity
- key schedule and key provenance
- little-endian target semantics
- pre-PoW ordering and target trust boundary
- validated-chain key-block provenance
- RandomX submodule pinning and Oregon salt patching
- CI dependency pinning and read-only permissions

Three important review findings were fixed before acceptance:
1. caller-controlled RandomX key-block identity
2. ability to misuse PoW validation without a prevalidated header token
3. mutable/obsolete `actions/checkout@v4` workflow dependency

No known Critical or Important review finding remains open at this checkpoint.

## Acceptance

M2 RandomX PoW bridge is accepted at reviewed code commit `44d22ae112b9182cf054aa9faa3426a66770b7ae` with fresh normal CI, native x64/ARM64 architecture-vector parity, and native x64/ARM64 FullEngine/LightEngine parity evidence recorded above.

This acceptance covers the M2 RandomX PoW bridge only. It does not claim that the full Oregon node, chainstate, P2P network, miner, wallet, genesis or mainnet launch stack is complete.
