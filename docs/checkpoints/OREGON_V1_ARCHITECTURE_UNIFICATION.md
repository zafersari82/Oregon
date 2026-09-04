# Oregon v1 Architecture Unification Checkpoint

Date: 2026-09-04
Branch: `oregon-v1-architecture-unification-2026-09-04`
Accepted M5 base: `a2aab4b73489aa0cf21bd7d14f8b930328c3465c`
Final reviewed code commit: `f7098ea187a360c7a30327c2a5e664a781fe05f3`

## Accepted scope

This checkpoint accepts the post-M5 architecture-unification work as a structural cleanup of the already accepted M0-M5 protocol baseline. It does not introduce a new monetary, consensus, persistence, UTXO, chain-selection or mempool policy rule.

Accepted architecture properties:

- one authoritative implementation owner per externally observable rule, following `docs/architecture/OREGON_ENGINEERING_CONSTITUTION.md`
- responsibility-named production modules and tests instead of task-numbered implementation paths
- chainstate split into admission, recovery, transition and UTXO-delta responsibilities
- mempool split into admission, capacity and pool responsibilities
- storage representation remains owned by `oregon-storage`
- unsafe Rust remains confined to the `oregon-pow` RandomX boundary
- accepted durable publication rule remains unchanged: active state is published only after successful RocksDB batch commit with WAL and synchronous durability
- accepted fail-closed recovery/reorg behavior remains unchanged
- accepted RandomX validation path and full/light parity remain unchanged
- accepted mandatory `SpendVerifier` boundary remains unchanged for confirmed and unconfirmed spends
- obsolete task-numbered and replaced implementation paths are deleted rather than retained as compatibility shims
- CI enforces architecture boundaries, workspace tests, rustdoc/docs, formatting and clippy
- the two RandomX integration workflows support explicit manual verification as well as path-triggered verification

This checkpoint intentionally does not claim P2P networking, peer discovery, node/mining RPC, wallet/address support, production spend authorization, production genesis/testnet/mainnet configuration or network launch readiness.

## Final integration verification

All required gates were run on the exact reviewed code commit:

`f7098ea187a360c7a30327c2a5e664a781fe05f3`

### Oregon Rust CI

Run: `33887973835`
Job: `101072360519`
Result: SUCCESS

Verified gates:

- Architecture scan: SUCCESS
- `cargo +1.85.0 test --locked --workspace --all-targets`: SUCCESS
- chainstate rustdoc with warnings denied: SUCCESS
- workspace docs: SUCCESS
- `cargo +1.85.0 fmt --all -- --check`: SUCCESS
- `cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings`: SUCCESS

### Oregon RandomX Architecture Vector

Run: `33887973842`
Result: SUCCESS

Matrix:

- `ubuntu-24.04`, job `101072360054`: SUCCESS
- `ubuntu-24.04-arm`, job `101072360343`: SUCCESS

The frozen Oregon RandomX architecture vector passed on both x86_64 and ARM runners.

### Oregon RandomX Full Light Parity

Run: `33887973847`
Result: SUCCESS

Matrix:

- `ubuntu-24.04`, job `101072360778`: SUCCESS
- `ubuntu-24.04-arm`, job `101072361029`: SUCCESS

The native RandomX full/light parity test passed on both x86_64 and ARM runners.

## Integration disposition

The reviewed architecture-unification code is accepted at `f7098ea187a360c7a30327c2a5e664a781fe05f3` subject to this checkpoint documentation commit itself passing the normal Oregon Rust CI gate.

After that documentation-only gate is green, an immutable accepted checkpoint branch may be created at the checkpoint commit and `main` may be advanced only by a non-forced fast-forward integration decision.

No force update, history rewrite or compatibility patch is part of this checkpoint.
