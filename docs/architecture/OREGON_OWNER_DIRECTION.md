# Oregon owner direction and continuity

Recorded: 2026-09-06 UTC, following explicit owner approval of Reserve Conservation
Proof V1 at design commit `7743aaa`.

## Approved immediate work

The owner explicitly stated **"planı onayladım"** ("I approved the plan") after
reviewing the proposed Kani bounded-model proof and production differential-test
approach. The design and implementation plan are:

- `docs/superpowers/specs/2026-09-06-reserve-conservation-proof-v1.md`
- `docs/superpowers/plans/2026-09-06-reserve-conservation-proof-v1.md`

Implementation of that approved scope may proceed without asking for the same
approval again. This records design approval, not completed proof evidence or a
decision to integrate into main.

## Enduring engineering objective

The owner wants internationally significant engineering and welcomes ambitious,
difficult choices and original inventions. Do not choose an inferior design solely
because it is easier, more familiar or quicker to demonstrate. Actively research
stronger approaches and bring concrete, reviewable proposals forward.

Evaluate challenging choices through explicit alternatives, benefits, failure
modes, prototype evidence and rejection criteria. Technical risk should serve a
demonstrable improvement; difficulty or risk alone is not evidence of superiority.
Treat originality as a hypothesis until comparative research supports it. Do not
promise world-leading performance, security or novelty without corresponding evidence.

This direction encourages research and proposals. It does not silently replace
frozen monetary, Multi-VM, hybrid-state, no-burn, ownership or durability decisions.
Record proposed amendments before implementation, under the existing constitution.

## Assurance must expand beyond the reserve equation

The current ten proof obligations develop one reserve-conservation assurance slice.
They are not ten independent proofs of the entire blockchain. Retain its bounded
model scope and distinguish production correspondence testing from formal refinement.

Subsequent critical subsystems need their own scoped verification designs, including:

- VM/runtime determinism, metering, isolation and cross-VM call behavior;
- execution authorization, escrow, revert, settlement and receipt consistency;
- block integration and atomic native/execution transitions;
- durable publication, crashes, recovery and cross-domain reorganization.

These are future assurance needs, not completed work or permission to invent their
unapproved detailed semantics. The current trust roadmap's Workstream B still means
mutation-evidence publication and C still means the readiness-gated RandomX public
testnet/challenge. Do not relabel B/C as VM or reorg formal proofs.

## Quality, time and completion

Do not compress ten obligations, their negative controls, differential cases or
evidence reporting into a superficial green result to meet a conversational deadline.
Progress is measured by verified artifacts and remaining proof obligations. Do not
invent a delivery date or treat an estimate of weeks as a measured schedule.

Report proof failure, unsupported tooling and incomplete evidence directly. A design
approval is not a proof result; a green ancestor is not current-head acceptance.

## Cross-session procedure

Every continuation reads this direction, `HANDOFF.md`, the current plan and applicable
normative documents before selecting work. Preserve the approved scope and completed
decisions. Do not ask for this design approval again unless the proposed design changes.

Before ending a working session, update the active branch, exact code commit, actual
verification evidence, blockers and next incomplete action. State whether changes
are local or available from GitHub; local commits do not establish remote persistence.
New conversations must verify the real repository state rather than relying on a
claim that chat memory will always preserve the project.
