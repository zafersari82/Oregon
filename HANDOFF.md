# Oregon — current continuation record

Updated: 2026-09-06 UTC. Stage 3B is integrated into `main`. Stage 4A is under test-first implementation on an isolated branch and is **not** integrated or activated. A post-Stage-4A trust/verification roadmap is now queued in-repo so the next work is preserved across conversations.

## Resume here

- Repository: `zafersari82/Oregon`.
- Main integration: `fe762f7a5670d94a486423327e3a525cec24afb5` via merged PR #17.
- Main integration tree: `cebbc2370e094052872570069c0a149a4581eb99`.
- Active Stage 4A branch: `work/runtime-journal-v1-2026-09-06`.
- Active draft PR: #19 — `Stage 4A: bounded execution journal foundation`.
- Stage 4 design: `docs/superpowers/specs/2026-09-06-runtime-journal-async-v1-design.md`.
- Stage 4A plan: `docs/superpowers/plans/2026-09-06-runtime-journal-v1.md`.
- Post-Stage-4A trust roadmap: `docs/checkpoints/OREGON_TRUST_VERIFICATION_ROADMAP.md`.
- Stage 3B integration evidence: `docs/checkpoints/OREGON_STAGE3B_MAIN_INTEGRATION.md`.
- Stage 3B implementation checkpoint: `docs/checkpoints/OREGON_FEE_SETTLEMENT_RESERVE_PROGRESS.md`.

Read `AGENTS.md` and all applicable normative architecture/design documents before editing. Inspect the current `main`, active-branch head, PR and exact-head CI; this record cannot override them.

## Completed integrated work

M0–M6, execution addresses, universal-envelope/auth structure, logical contract state, Stage 3A resource metering/base fees, and Stage 3B escrow/settlement/reserve foundations are integrated. Do not repeat them or merge PR #17 again.

The Stage 3B closure source is `d7b6773f842f35bcd44f485315e17b1cc2703078`; its own Rust CI #720 (`34039808168`, job `101504386721`) and Stage 3B x86_64/ARM vectors #12 (`34039808176`, jobs `101504386743` / `101504386825`) passed. Its resource and RandomX vector/parity workflows passed as well. The actual merge has exactly the same tree. Main Rust CI #721 also passed after integration.

Historical checkpoints describe their own recording-time main and pending-integration boundaries. They remain historical evidence, not instructions to revert the current main.

## Active Stage 4A work

Stage 4 remains decomposed into independently verified slices:

1. **4A:** bounded frame journal over existing Oregon SMT snapshots, nested commit/revert, checked reads and atomic unpublished finalization.
2. **4B:** deterministic runtime/call ABI and transaction/fee/receipt composition, retaining one shared meter and authoritative source validation.
3. **4C:** bounded async message/outbox/consumption lifecycle; exact wire/expiry/delivery semantics require their own detailed design before implementation.

Stage 4A has moved beyond design-only status. Test-first journal coverage and production modules have been added on PR #19, including constructor/domain validation, overlay reads/writes, bounded frame lifecycle, child-to-parent commit, revert and unpublished finalization/export work. The expected RED state was previously observed at the public API boundary before production export.

**Do not call Stage 4A complete yet.** The exact current branch head still requires its focused compile/test result, any resulting root-cause fixes, independent Stage 4A vectors/mutation gates required by the plan, full exact-head Rust CI, checkpoint closure, and then a separate owner integration decision. `main` must remain unchanged until that decision.

## Queued work after Stage 4A reaches main

Once Stage 4A has been fully verified, checkpointed, separately approved for integration and successfully merged to `main`, the next owner-requested program is the trust/verification track recorded in:

`docs/checkpoints/OREGON_TRUST_VERIFICATION_ROADMAP.md`

Default execution order:

1. **Formal 1:1 reserve-conservation proof.** Design and pin a Rust-compatible formal verification tool (Kani is the initial candidate), prove the explicitly modeled Stage 3B reserve invariants, include negative proof controls, and publish the exact proof boundary.
2. **Reproducible mutation evidence.** Turn the existing consensus/runtime mutation gates into a versioned machine-readable public evidence manifest and reproducible technical report. Mutation testing must not be mislabeled as formal proof.
3. **RandomX public testnet/adversarial challenge.** Prepare reproducible CPU-mining participation and reorg/security challenge material, but launch only after node/testnet readiness. No mainnet stress event and no unapproved monetary bounty promise.

This queued trust track does not activate Stage 4B/4C and does not erase their accepted architecture. It is the next verification/public-evidence work after Stage 4A integration unless the owner explicitly reprioritizes it.

## Preserved boundaries and implementation cautions

- Stage 3B integration is not activation: current block/transaction/UTXO/storage/mempool/network/node paths remain unchanged.
- `FundingCapabilityV1::new` validates data; it does not verify a live payer source. A future coordinator must obtain authority from source validation.
- Existing `WeightMeter` exhaustion is sticky and charges the maximum. Child rollback cannot reset it or cumulative conversion counters.
- EVM commitment semantics remain separate; never send EVM roots through the Oregon SMT journal.
- No generic VM-facing unrestricted system-domain write or non-revertible write API is permitted.
- Stage 4A finalization is an unpublished proposal only; it is not persistence or consensus activation.
- Main integration remains a separate decision for each future slice. Never force-push or move accepted checkpoint refs.

## Verification and continuation discipline

For Stage 4A, do not substitute ancestor or Stage 3B CI for current-head verification. Close failures by root cause; do not weaken tests to make a gate green. Record exact commit SHA and exact CI/vector/mutation evidence in the Stage 4A checkpoint before proposing integration.

After Stage 4A reaches `main`, open `OREGON_TRUST_VERIFICATION_ROADMAP.md` before choosing the next implementation task. The default first target is the formal reserve-conservation proof design.

Keep this record and the roadmap in the repository so another conversation can continue without relying on chat memory.
