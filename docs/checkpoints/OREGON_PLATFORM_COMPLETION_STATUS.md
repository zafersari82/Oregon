# Oregon platform completion status

Recorded: 2026-09-11 UTC. This is a source-backed progress map, not a new architecture, activation decision or completion percentage.

## Reviewed sources

- Accepted main: `529b6e8fd50ad4a16d69485b5720b89e979055aa`.
- Stage 4B integration: PR #20 and `docs/checkpoints/OREGON_STAGE4B_MAIN_INTEGRATION.md`.
- Reserve proof integration: `4670d4ccedd09f02c2a87a13968e74f1fdfa9f4b`.
- Mutation-evidence integration: `cc791386899a777344404b98278cf41f0752f173`.
- Normative execution sequence: Execution Architecture V1, section 27.
- Trust track: `docs/checkpoints/OREGON_TRUST_VERIFICATION_ROADMAP.md`.

PR descriptions and historical plans may lag actual repository state. Main integration, implementation verification and protocol activation are reported separately.

## Platform sequence

| Stage | Verified repository status | Remaining closure or implementation |
| --- | --- | --- |
| Native M0–M6 | Accepted consensus, RandomX, UTXO, storage, chainstate, mempool and bounded networking/node foundations | Public-node launch readiness remains outside M6 |
| Execution primitives | Typed addresses and inactive envelope/authentication structures integrated | Activation and domain-specific ingress verification remain later work |
| Contract state | Logical contract-state commitments integrated as inactive foundations | Connect active execution transitions and persistence at later planned stages |
| Resources, fees, reserve | Inactive metering, escrow/settlement and reserve foundations integrated | Live caller provenance and active block execution remain later work |
| Stage 4A journal | Integrated bounded synchronous journal with rollback and unpublished finalization | Does not itself execute a production VM or publish durable state |
| Stage 4B coordinator | **Integrated on main** at `529b6e8…`; exact-main Rust CI, runtime-coordinator, journal, fee and resource vector workflows passed | Still inactive/test-gated; no production VM backend, active block path, durable publication, mempool/RPC wiring or activation |
| Stage 4C async | Architecture direction exists only | Delivery, expiry, replay protection and exactly-once consumption design/implementation/verification |
| EVM and Ethereum ingress | Frozen architecture direction | Backend, compatibility profile, deterministic normalization/RPC and differential vectors |
| WASM | Frozen deterministic architecture direction | Backend, host restrictions/metering and cross-architecture deterministic vectors |
| Cross-VM calls | Explicit runtime boundary defined directionally | Real-backend nested calls, transfers, revert/reentrancy/exhaustion evidence |
| Mempool and block execution | Planned in section 27 | Native/Ethereum ingress convergence and atomic native/execution block transitions |
| Durable execution/reorg | Planned in section 27 | WAL/crash/recovery and reorg across every active state domain |
| Activation | Not delivered by proof/publication/Stage 4B integration | Complete applicable verification and explicit protocol activation/checkpoint decision |

Stage 4B now contributes `oregon-runtime` and the test-gated coordinator to `main`. Do not describe that code as absent. It remains an inactive foundation and must not be described as live EVM/WASM execution or completed Target 2.

## Stage 4B exact-main evidence

Exact main SHA: `529b6e8fd50ad4a16d69485b5720b89e979055aa`.

- Oregon Rust CI run `34517797355` — SUCCESS.
- Oregon Runtime Coordinator Vectors run `34517797314` — SUCCESS on x86_64 and ARM.
- Oregon Runtime Journal Vectors run `34517797466` — SUCCESS on x86_64 and ARM.
- Oregon Fee Settlement Vectors run `34517797360` — SUCCESS on x86_64 and ARM.
- Oregon Execution Resource Vectors run `34517797377` — SUCCESS on x86_64 and ARM.
- Runtime coordinator mutation gate: **16/16 compiled mutations killed**; inherited mutation gates remained green.

See `docs/checkpoints/OREGON_STAGE4B_MAIN_INTEGRATION.md` for exact pre-merge and post-merge identities and claim boundaries.

## Assurance track

| Workstream | Status and boundary |
| --- | --- |
| A: Reserve Conservation Proof V1 | **Integrated.** RC01–RC10 prove the selected bounded model; production correspondence is executable differential testing, not full refinement of arbitrary production state/storage/runtime behavior |
| B: Mutation Evidence Publication V1 | **Integrated.** The published package records 68 selected injected faults across six production authorities; this is reproducible mutation evidence, not formal proof or a platform-completion percentage |
| C: Public RandomX testnet/challenge | **Readiness-gated and not launched.** The next trust-track work is a versioned readiness design, not immediate public exposure |

## Workstream C readiness gap

The M6 baseline deliberately excludes several things an external participant would need for a reproducible public mining/adversarial event, including production node/mining startup surfaces, production spend authorization, wallet integration, peer discovery/DNS bootstrap, production genesis/public-network configuration and a documented outside-participant workflow.

Before public launch, a versioned readiness design must define at minimum:

- deterministic isolated testnet/genesis and network identifiers;
- node and CPU-miner startup/configuration path;
- safe testnet spend authorization without test-only accept-all shortcuts;
- peer bootstrap/discovery suitable for outside participants;
- observability for peers, mining, forks, reorgs and failures while minimizing unnecessary personal data;
- deterministic incident capture sufficient to reproduce or reject submitted findings;
- x86_64 and ARM external verification instructions;
- explicit challenge scope, responsible disclosure and launch/rollback criteria.

A temporary loopback harness, fixture genesis, accept-all verifier or existing RandomX engine correctness evidence is not a substitute for this readiness gate. Any monetary bounty remains a separate owner/funding decision.

## Separate execution track

Workstream C does not replace the remaining execution roadmap. Stage 4C async semantics, production EVM/WASM backends, cross-VM behavior, active block/mempool integration, durable execution publication/recovery and protocol activation remain separate versioned designs and implementation slices.

Do not smuggle execution activation into testnet-readiness work.

## Progress reporting rule

Report implementation, exact-head verification, main integration and activation separately. Neither CI pass counts, 68/68 mutation kills, nor RC01–RC10 proof completion is a percentage of the whole Oregon platform. No total completion percentage is supported until remaining work and its weighting are explicitly estimated and reviewed.
