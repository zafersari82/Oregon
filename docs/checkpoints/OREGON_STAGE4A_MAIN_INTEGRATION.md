# Oregon Stage 4A Main Integration Record

Recorded: 2026-09-06 UTC. Stage 4A bounded execution journal is integrated into `main` as an **inactive foundation**.

## Integration

After exact-head implementation/checkpoint verification, the owner gave the repository-required separate integration decision. PR #19 was marked ready and merged through GitHub with an exact expected-head guard. No force-push, activation change or historical checkpoint-ref movement was used.

- PR: https://github.com/zafersari82/Oregon/pull/19
- Previous main: `fe762f7a5670d94a486423327e3a525cec24afb5`.
- Verified closure source: `bb24a2cb1bed17736c931888b334388a67f29842`.
- Verified closure tree: `02da551eba74ee53ab003364c6a054176c02755f`.
- Actual main merge: `dee4ca6ea3b6dc75e4920ab56a44e2b5da8aa0a3`.
- Actual merge tree: `02da551eba74ee53ab003364c6a054176c02755f`.

The actual merge tree is identical to the checkpoint successor tree that had already passed exact-head verification.

## Closure-source evidence

- Oregon Rust CI #772: run `34047120052`, job `101524055508` — SUCCESS.
- Runtime Journal Vectors #25: run `34047120019` — SUCCESS.
  - x86_64 job `101524055523`.
  - ARM job `101524055354`.
- Execution Resource Vectors #102 — SUCCESS.
- Fee Settlement Vectors #51 — SUCCESS.
- RandomX Architecture Vector #129 — SUCCESS.
- RandomX Full Light Parity #119 — SUCCESS.

The closure-source Rust CI passed architecture/focused contracts, full workspace/all-target tests, inherited mutation gates and Stage 4A journal 12/12 mutations, rustdoc/docs, Format and warnings-denied Clippy.

## Actual main-merge evidence

The following runs target the actual merge SHA `dee4ca6ea3b6dc75e4920ab56a44e2b5da8aa0a3`:

- Oregon Rust CI #773: run `34048254431`, job `101527099485` — SUCCESS.
  - architecture/focused contracts — SUCCESS
  - full workspace/all-target tests — SUCCESS
  - execution address mutation gates 3/3 — SUCCESS
  - execution envelope mutation gates 9/9 — SUCCESS
  - contract-state mutation gates 17/17 — SUCCESS
  - execution-resource mutation gates 13/13 — SUCCESS
  - fee-settlement mutation gates 14/14 — SUCCESS
  - runtime-journal mutation gates 12/12 — SUCCESS
  - rustdoc/docs — SUCCESS
  - Format — SUCCESS
  - warnings-denied Clippy — SUCCESS
- Runtime Journal Vectors #26: run `34048254365` — SUCCESS.
  - x86_64 job `101527099240` — SUCCESS.
  - ARM job `101527099317` — SUCCESS.
- Execution Resource Vectors #103: run `34048254372` — SUCCESS.
  - x86_64 job `101527099230` — SUCCESS.
  - ARM job `101527099377` — SUCCESS.
- Fee Settlement Vectors #52: run `34048254591` — SUCCESS.
  - x86_64 job `101527099826` — SUCCESS.
  - ARM job `101527099958` — SUCCESS.

No RandomX main-push rerun is claimed for this merge. The exact merge push triggered Rust CI, runtime-journal vectors, execution-resource vectors and fee-settlement vectors. RandomX closure-source evidence remains recorded separately.

## Integrated Stage 4A scope

Stage 4A adds the inactive bounded journal foundation in `oregon-execution`:

- validated supported Oregon SMT domain snapshots;
- checked base reads through the Stage 2 `StateSource`/SMT path;
- top-frame overlay put/delete/read semantics with present-empty distinct from deletion;
- bounded nested begin/commit/revert lifecycle;
- monotonic frame creation, depth/entry/retained-byte/key/value ceilings;
- child-to-parent ownership-moving commit and ancestor-revert behavior;
- deterministic numeric-domain/raw-key ordering;
- atomic unpublished multi-domain finalization through existing checked Stage 2 write-set transitions;
- independent Python-generated trace/root vectors consumed by Rust on x86_64 and ARM;
- dedicated runtime-journal mutation gates.

## Non-activation boundary

This integration does **not** activate or deliver:

- a VM or deterministic runtime/call ABI;
- Stage 4B transaction/fee/receipt composition;
- Stage 4C async message delivery/expiry/consumption semantics;
- durable execution publication or persistence;
- universal-envelope execution in active blocks;
- new transaction/block wire bytes;
- EVM-through-Oregon-SMT journal behavior.

A Stage 4A journal result remains an unpublished proposal. Integration is foundation availability, not protocol activation.

## Next work

The owner previously queued `docs/checkpoints/OREGON_TRUST_VERIFICATION_ROADMAP.md` to begin after Stage 4A reaches main. The default next target is therefore the **formal Stage 3B 1:1 reserve-conservation proof design**, followed by reproducible mutation-evidence publication and, when node/testnet readiness permits, RandomX public testnet/adversarial challenge preparation.

Do not claim Stage 3B is mathematically/formally proven until the formal proof harness and pinned CI exist and its proof boundary is explicitly recorded. Do not conflate mutation testing with formal proof, and do not claim RandomX alone establishes ASIC-proof decentralization.
