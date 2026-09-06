# Oregon — current continuation record

Updated: 2026-09-06 UTC. Stage 3B is integrated into main. Stage 4 has a proposed design on a separate branch; no Stage 4 code or protocol activation is claimed.

## Resume here

- Repository: `zafersari82/Oregon`.
- Main integration: `fe762f7a5670d94a486423327e3a525cec24afb5` via merged PR #17.
- Main integration tree: `cebbc2370e094052872570069c0a149a4581eb99`.
- Active design branch: `design/runtime-journal-async-v1-2026-09-06`.
- Proposed design: `docs/superpowers/specs/2026-09-06-runtime-journal-async-v1-design.md`.
- Integration evidence: `docs/checkpoints/OREGON_STAGE3B_MAIN_INTEGRATION.md`.
- Stage 3B implementation checkpoint: `docs/checkpoints/OREGON_FEE_SETTLEMENT_RESERVE_PROGRESS.md`.

Read AGENTS.md and all applicable normative architecture/design documents before editing. Inspect the current main, design-branch head, PR and exact-head CI; this record cannot override them.

## Completed work

M0–M6, execution addresses, universal-envelope/auth structure, logical contract state, Stage 3A resource metering/base fees, and Stage 3B escrow/settlement/reserve foundations are integrated. Do not repeat them or merge PR #17 again.

The Stage 3B closure source is `d7b6773f842f35bcd44f485315e17b1cc2703078`; its own Rust CI #720 (`34039808168`, job `101504386721`) and Stage 3B x86_64/ARM vectors #12 (`34039808176`, jobs `101504386743` / `101504386825`) passed. Its resource and RandomX vector/parity workflows passed as well. The actual merge has exactly the same tree. Its separate main CI evidence is recorded in the integration note.

Historical checkpoints describe their own recording-time main and pending-integration boundaries. They remain historical evidence, not instructions to revert the current main.

## Proposed next scope

The Stage 4 proposal splits the next subsystem into independently verified slices:

1. **4A:** bounded frame journal over existing Oregon SMT snapshots, nested commit/revert, checked reads and atomic unpublished finalization.
2. **4B:** deterministic runtime/call ABI and transaction/fee/receipt composition, retaining one shared meter and authoritative source validation.
3. **4C:** bounded async message/outbox/consumption lifecycle; exact wire/expiry/delivery semantics need their own detailed design before implementation.

The current document is a **proposal for review**, not an owner-approved Stage 4 specification. The first next action is review of the decomposition and 4A semantics, followed by the 4A test-first implementation plan. Do not infer approval of future ABI bytes, bridge/oracle/AI trust or activation parameters from this proposal.

## Preserved boundaries and implementation cautions

- Stage 3B integration is not activation: current block/transaction/UTXO/storage/mempool/network/node paths are unchanged.
- `FundingCapabilityV1::new` validates data; it does not verify a live payer source. A future coordinator must obtain authority from source validation.
- Existing `WeightMeter` exhaustion is sticky and charges the maximum. Child rollback cannot reset it or cumulative conversion counters.
- EVM commitment semantics remain separate; never send EVM roots through the Oregon SMT journal.
- No generic VM-facing unrestricted system-domain write or non-revertible write API is permitted.
- Main integration remains a separate decision for each future slice. Never force-push or move accepted checkpoint refs.

## Verification and continuation discipline

This branch changes documentation only. Verify its own PR CI before calling its documentation closure complete; do not substitute Stage 3B/main results. Local documentation checks do not count as running Rust tests. The local checkout used for this proposal had no Cargo executable; GitHub CI is the Rust verification source.

Keep the proposed design and this record in the repository so another conversation can continue without relying on chat memory. After review, record the actual approval and implementation plan explicitly; do not relabel the proposal as accepted merely because it exists in git.
