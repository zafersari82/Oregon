# Reserve Conservation Proof V1 — Main Integration

Recorded: 2026-09-08 UTC.

## Owner decision and source identity

The owner explicitly approved integrating PR #24 with "Tamam yapalım" in response
to the specific merge request. GitHub merged the PR using the expected head guard
and the merge method, preserving original commit history.

- PR: https://github.com/zafersari82/Oregon/pull/24
- Previous main: `a530ae1d20e5a1648cf9cfea045221ec13326d25`.
- Verified PR head: `962b84f526c9477ec8c8206dbbf260b33a49426f`.
- Main merge: `4670d4ccedd09f02c2a87a13968e74f1fdfa9f4b`.
- Merge tree: `dcee6ca67d12198c4c73722ee0cbe81ba94ae384`.
- The merge tree is identical to the verified PR-head tree; local Git comparison
  also returned no differences. Both previous main and PR head are merge parents.

## Final PR-head proof evidence

At `962b84f526c9477ec8c8206dbbf260b33a49426f`:

- Reserve Conservation Proofs: run `34222683669`, job `102049285521`, SUCCESS.
- Reserve Verifier Bootstrap: run `34222683630`, job `102049285408`, SUCCESS.
- Oregon Rust CI: run `34222683676`, job `102049285358`, SUCCESS.

The proof job passed arithmetic and state RC01–RC10 obligations, ten negative
controls, full positive rerun and evidence retention. The acceptance checkpoint
`OREGON_RESERVE_CONSERVATION_PROOF_V1.md` retains exact proof boundaries and
historical implementation-source evidence.

## Actual main-merge verification

The following runs were triggered by main merge
`4670d4ccedd09f02c2a87a13968e74f1fdfa9f4b` and all completed successfully:

| Workflow | Run | Jobs |
| --- | --- | --- |
| Oregon Rust CI | 34234481433 | 102088552158 |
| Runtime Journal Vectors | 34234481466 | x86_64 102088552059; ARM 102088552458 |
| Fee Settlement Vectors | 34234481489 | x86_64 102088552555; ARM 102088552544 |
| Execution Resource Vectors | 34234481297 | x86_64 102088551415; ARM 102088551575 |

Canonical run URL pattern:
`https://github.com/zafersari82/Oregon/actions/runs/<run-id>`.

Rust CI passed architecture and focused contracts, full workspace/all-target tests,
all six inherited mutation gates, rustdoc/docs, Format and warnings-denied Clippy.
The 24 Python proof-runner tests also passed locally against the merged code.

Reserve proof and bootstrap workflows do not trigger on main pushes. Their
successful PR-source evidence is retained alongside verified tree equality;
there was no main-only Kani/bootstrap rerun. No new main RandomX full/light or
architecture-proof run is claimed. ARM results above are executable vectors,
not ARM Kani verification.

## Continuation

PR #24 integration and actual merge-source verification are complete. This
checkpoint, HANDOFF and plan closure are documentation-only follow-up changes;
the runs above attest to the specified merge source, not a later documentation
commit. Later head checks must be identified by their own SHA.

Next: Workstream B of `OREGON_TRUST_VERIFICATION_ROADMAP.md`. Inventory the six
existing production mutation runners named by `oregon-rust.yml`, their semantic
kill criteria and machine-readable evidence, then prepare a versioned publication
design and implementation plan. Reuse those authorities and keep formal model
controls separate from production mutation claims.

No execution, reserve block-path, VM, protocol or monetary activation occurs in
this integration. Stage 4B/4C and the RandomX public testnet remain separately gated.
