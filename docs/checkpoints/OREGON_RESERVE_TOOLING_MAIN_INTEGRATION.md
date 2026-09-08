# Reserve verification tooling and initial model: main integration

Recorded: 2026-09-07 UTC. This records the partial verification foundation merged
through PR #21, not acceptance of Reserve Conservation Proof V1 or activation.

## Source identity

- PR: https://github.com/zafersari82/Oregon/pull/21 (merged).
- Previous main: `dd7cdcb566273c39d5a38cf0c0036058b08a7d89`.
- Verified PR head: `51659a39dd0c41bbf607e75f2a4d5b3589e53f27`.
- Merge: `a530ae1d20e5a1648cf9cfea045221ec13326d25`.
- Head and merge tree: `628f3b54e1a3c96c129368c66ad8f55991426afc`.
- Merge parents are previous main and verified PR head, in that order.

The owner directed Clippy repair, green candidate CI, then integration of PR #21.
The same verification model source had been declared twice in the UTXO test
crate. It is now declared once under cfg(test) and imported by both test modules.
No lint suppression, production reserve behavior change or activation was added.

## Exact candidate evidence

At the verified PR head, all triggered PR workflows succeeded:

- Rust CI #823: https://github.com/zafersari82/Oregon/actions/runs/34132383577
  (job `101775331351`).
- Fee Settlement Vectors #98:
  https://github.com/zafersari82/Oregon/actions/runs/34132383546
- Reserve Verifier Bootstrap #52:
  https://github.com/zafersari82/Oregon/actions/runs/34132383672

Rust CI includes architecture/focused contracts, full workspace/all-target tests,
all inherited mutation gates, rustdoc/docs, format and warnings-denied Clippy.
Bootstrap verifies the pinned tool setup and positive/negative/positive smoke
checks, not RC01-RC10.

## Actual merge evidence

The following push workflows succeeded at the actual merge SHA, rechecked during
the evidence-gate continuation:

- Rust CI #824: https://github.com/zafersari82/Oregon/actions/runs/34133369147
  (job `101778546642`).
- Execution Resource Vectors #125:
  https://github.com/zafersari82/Oregon/actions/runs/34133369104
- Fee Settlement Vectors #99:
  https://github.com/zafersari82/Oregon/actions/runs/34133369150
- Runtime Journal Vectors #48:
  https://github.com/zafersari82/Oregon/actions/runs/34133369144

No main-push Kani or RandomX rerun is claimed. These historical results do not
validate later documentation or parser changes, which need their own exact CI.

## Scope and next work

The integrated source contains a standalone verification-only Rust package,
checked arithmetic, four-slot apply/undo model and 12 crate-local correspondence
tests. It also contains the pinned bootstrap runner and retained tooling fixtures.

RC01-RC10 harnesses, full-width symbolic verification, reachability, all ten model
controls with counterexamples, the full correspondence matrix and final evidence
manifest remain incomplete. Existing Rust tests and production mutation gates are
not formal model proofs. Workstream A remains open; B/C are not started by this
integration. Production/model equivalence, unbounded state, fee provenance, live
outputs, storage/crashes, VM execution, block/reorg integration and global native
supply conservation remain outside the claimed proof scope.
