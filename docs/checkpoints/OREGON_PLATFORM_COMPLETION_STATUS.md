# Oregon platform completion status

Recorded: 2026-09-09 UTC. This is a source-backed progress map, not a new
architecture, activation decision or completion percentage.

## Reviewed sources

- Main: `5785089e0cc2de18b1b50ef112afbe2f8265a8d8`.
- Mutation publication: PR #25, package-verification implementation
  `1de457fa985e0af1c8149a8e8c51ead4e80679a7`.
- Stage 4B: PR #20, `912e9d01791c954e8432e3f80f6913e3b87c8db6`.
- Normative sequence: Execution Architecture V1, section 27.
- Trust track: `OREGON_TRUST_VERIFICATION_ROADMAP.md`.

PR descriptions and unchecked historical plan templates can lag actual source.
An implementation on a work branch is not an accepted main integration.

## Platform sequence

| Stage | Verified repository status | Remaining closure or implementation |
| --- | --- | --- |
| Native M0–M6 | Accepted consensus, RandomX, UTXO, storage, chainstate, mempool and bounded networking/node foundations | Public-node launch readiness is outside M6 |
| Execution primitives | Typed addresses and inactive envelope/authentication outer structures integrated | Activation and domain-specific ingress verification remain later work |
| Contract state | Logical contract-state commitments integrated as inactive foundations | Connect active execution transitions and persistence at their planned stages |
| Resources, fees, reserve | Inactive metering, escrow/settlement and reserve foundations integrated | Caller authorization/provenance and integrated execution still require coordinator/block work |
| Stage 4A journal | Integrated, bounded synchronous journal with rollback and unpublished finalization | Does not itself execute a production VM or publish durable state |
| Stage 4B coordinator | Runtime ABI, receipts/effects/events and coordinator implementation exist on PR #20 | Resolve divergence from current main, complete its 16-mutation gate and dedicated x86_64/ARM vector workflow, acceptance checkpoint and integration |
| Stage 4C async | Architecture direction exists | Delivery, expiry, replay and exactly-once consumption implementation/verification |
| EVM and Ethereum ingress | Frozen architecture direction | Backend, supported compatibility profile, deterministic normalization/RPC and differential vectors |
| WASM | Frozen deterministic architecture direction | Backend, host restrictions/metering, cross-architecture deterministic vectors |
| Cross-VM calls | Explicit runtime boundary required | Integrated nested calls, transfers, reverts, reentrancy and exhaustion evidence with real backends |
| Mempool and block execution | Planned in section 27 | Native/Ethereum ingress convergence, atomic native/execution block transitions |
| Durable execution/reorg | Planned in section 27 | WAL/crash/recovery and reorg across every active state domain |
| Activation | Not delivered by the proof/publication work | Complete section 24 verification and explicit protocol activation/checkpoint decision |

The main workspace contains no `oregon-runtime`, `oregon-vm-evm` or
`oregon-vm-wasm` member at the reviewed main source. The runtime crate is present
on PR #20. Do not describe all runtime code as absent or count that draft as
integrated platform completion.

## Assurance track

| Workstream | Status and boundary |
| --- | --- |
| A: Reserve Conservation Proof V1 | Integrated through PR #24. RC01–RC10 verify the bounded model; production correspondence is tested, not a full refinement proof |
| B: Mutation Evidence Publication V1 | PR #25 publishes 68 selected faults across six existing production authorities. Implementation and package verification passed at the reviewed source; documentation-successor CI and main integration remain separate |
| C: Public RandomX testnet/challenge | Readiness-gated preparation remains. No public launch or bounty is authorized by A/B completion |

## Concrete Stage 4B gap

At the reviewed PR #20 head, source includes `oregon-runtime` and coordinator
accounting/calls/effects/host/settlement modules, plus independent receipt vectors.
Its historical Rust CI run `34093950643` succeeded, and inherited vector workflows
also succeeded at that head. Those results do not replace its dedicated closure
requirements or validate a future merge with current main.

Two planned Task 10 files are absent at that exact source:

- `scripts/verify_runtime_coordinator_mutations.py`;
- `.github/workflows/oregon-runtime-coordinator.yml`.

The branch plan requires 16 named semantic controls, a tampered-vector negative
control, dedicated x86_64/ARM runtime-coordinator execution and architecture gates.
The acceptance checkpoint `OREGON_RUNTIME_COORDINATOR_PROGRESS.md` is also absent.
The first continuation should reconcile the preserved branch with current main
and inspect remaining task requirements before adding those tests/gates.

## Public testnet readiness

M6 explicitly excludes node/mining RPC, production spend authorization, wallet,
peer discovery/DNS bootstrap, production genesis and public network configuration.
The reviewed `oregon-node` package is a library: no `src/main.rs`, `src/bin/` or
explicit binary target is present. Test fixtures are not launch configuration.

Before an outside participant can reproduce a challenge, a versioned readiness
design must define node/miner startup, network/genesis isolation, spend
authorization, peer bootstrap, observability and reproducible incident capture.
Do not substitute an accept-all test verifier, temporary test genesis or loopback
test harness for a production or public-testnet security decision.

## Progress reporting rule

Report implementation, verified branch evidence, main integration and activation
separately. Neither a percentage of passing CI workflows nor the 68/68 mutation
score is a percentage of the entire Oregon platform. No total completion
percentage is supported until remaining work and its weighting are explicitly
estimated and reviewed. This map does not claim that either 50% or 90% is measured.
