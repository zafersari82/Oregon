# Oregon — current continuation record

Updated: 2026-09-06 UTC. The owner explicitly requested main integration: “MAİNE İŞLEYELİM O ZAMAN”. This authorizes integration of the completed inactive foundations through Stage 3A; execution activation still requires its own design and decision.

## Resume here

- Repository: https://github.com/zafersari82/Oregon
- Integration branch: `work/integrate-execution-foundations-2026-09-06`.
- Integration target and continuation baseline after merge: `main`.
- Pre-integration main: `bf7675bfe17182f77d4c43e2bcbd0c283709d799`.
- Complete verified source: `06233e9b509c25e6aed3466cd79613cfbf0ff850`.
- Source tree: `994b317bb0a3e0aad9b37aca45e493a3ff3bb4f3`.
- This source includes the full ancestor history of PRs #9–#15. The integration uses a merge commit to preserve that history and leaves the original source branches intact.

Before editing, fetch current `main` and inspect the integration PR's merge state and CI. If it is merged, start the next isolated branch from current `main`; do not resume an old Stage 1/2/3A branch. If it is still open, finish its checks and the authorized integration first. GitHub's recorded merge commit and its CI are the authoritative integration outcome; this file does not predict a future merge SHA.

## Completed inactive foundations

| Work | Source | Specification/checkpoint |
| --- | --- | --- |
| Execution Architecture V1 | PR #9, `ed67ccb89131970571d93911cf5553be33636e2f` | `docs/superpowers/specs/2026-09-05-execution-architecture-design.md` |
| Typed addresses | PR #10, `8057ba2a030a8e79c10a240d48675be758c4d875` | `docs/checkpoints/OREGON_EXECUTION_ADDRESS_PROGRESS.md` |
| Universal envelope and authorization structure | PRs #11–#12, `b97f9d3af9e2c9c4011750cfb69cce8fd9117a8a` | `docs/checkpoints/OREGON_EXECUTION_ENVELOPE_PROGRESS.md` |
| Logical contract state and commitments | PRs #13–#14, `7cfac99c40de96afb79d222cd5ade9f3a537215a` | `docs/checkpoints/OREGON_CONTRACT_STATE_PROGRESS.md` |
| Resource metering, parent-derived base fee, transaction/block budgets | PR #15, `06233e9b509c25e6aed3466cd79613cfbf0ff850` | `docs/checkpoints/OREGON_EXECUTION_RESOURCES_PROGRESS.md` |

Subsystem checkpoints preserve the decisions and verification evidence from their recorded dates. Their old “main unchanged/not approved” statements describe those historical checkpoints; the explicit integration decision above supersedes that progress status, without changing any normative architecture or activation rule.

## Verification and integration gate

The exact source `06233e9b509c25e6aed3466cd79613cfbf0ff850` passed:

- Oregon Rust CI [34025020784](https://github.com/zafersari82/Oregon/actions/runs/34025020784), job `101464341445`: full workspace/all-target tests, architecture and focused contracts, rustdoc/docs, format, warnings-denied Clippy.
- Address mutations 3/3, envelope mutations 9/9, contract-state mutations 17/17, resource mutations 13/13; restored suites passed.
- Execution Resource Vectors [34025020773](https://github.com/zafersari82/Oregon/actions/runs/34025020773): x86_64 job `101464341320`, ARM job `101464341471`, both successful.

The integration adds only this continuation update and main-push triggers for the Rust and resource-vector workflows. Production code, tests, vectors, Cargo dependencies and normative contracts are identical to the verified source. The integration PR must pass its own checks before merging; the resulting `main` commit must pass its own Rust and x86_64/ARM resource workflows. Record their exact SHAs and run links in the integration PR. Do not substitute an ancestor's green run for a changed tree.

## Exact next action after integration

Read `AGENTS.md`, the Engineering Constitution, Platform Architecture Contract, Execution Architecture V1 and the Stage 3A checkpoint. Continue with a separate versioned Stage 3B design for fee escrow/settlement, fee-state transitions and native UTXO reserve conservation.

Define authoritative owners, exact bytes/keys, authenticated funding and producer destinations, deterministic arithmetic/overflow rules, rollback versus non-revertible fee boundaries, backing invariants, stale/double-settlement rejection, receipts/undo, required vectors and adversarial tests before implementation. Preserve no fee burn, one fee/execution truth and exact 1:1 native backing. Activation constants require their specified benchmark/vector evidence.

Do not repeat completed M0–M6, Stage 1, Stage 2 or Stage 3A work. The owner requests autonomous progress without repeated low-level approval prompts. Frozen architecture changes and future main integrations still follow their applicable explicit-decision process; this authorization covers the current integration.

## Remaining sequence and boundaries

Stage 3B fee accounting/reserve integration, runtime/journal/async core, EVM ingress/backend, WASM backend, cross-VM operations, unified mempool/block execution, durable chainstate/reorg/recovery, then separate activation decisions.

The current header/transaction bytes, RocksDB schema, UTXO and monetary rules, and active M0–M6 behavior are preserved by this integration. No wallet/RPC, public network or active contracts are claimed. Never force-push accepted/shared checkpoint refs. Preserve sources, verification evidence, limitations and the next action in the repository.
