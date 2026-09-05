# Oregon AI Agent Instructions

This file is the repository-root entry point for AI coding agents and human contributors using agentic tools.

## Mandatory reading before any change

Read these files before proposing or implementing Oregon changes:

1. `docs/architecture/OREGON_ENGINEERING_CONSTITUTION.md`
2. `docs/architecture/OREGON_PLATFORM_ARCHITECTURE_CONTRACT.md`
3. the latest accepted checkpoint under `docs/checkpoints/` for every subsystem you will touch
4. any versioned design/spec named by the current task

These documents are normative. Do not infer a different architecture from surrounding code, another blockchain, a framework default, or a convenience shortcut.

## Current accepted baseline

The accepted implementation baseline is M6. Existing M0-M6 consensus, PoW, transaction/block encoding, UTXO, storage, chainstate, mempool, networking, peer, synchronization, and node-orchestration behavior is frozen unless a separately approved versioned amendment explicitly changes it.

Do not reinterpret a future architecture decision as permission to mutate current accepted behavior.

## Frozen future platform direction

Owner-approved platform decisions are recorded in `docs/architecture/OREGON_PLATFORM_ARCHITECTURE_CONTRACT.md` and must be preserved:

- Oregon is architected as **Multi-VM**, with EVM and WASM as the first intended VM families.
- Oregon uses a **hybrid state model**: native OREG remains UTXO-based; contract execution uses a separate account/contract-state domain.
- Future platform transactions use **one versioned Oregon universal transaction envelope** with explicit execution-domain dispatch; current M0-M6 transaction encoding remains frozen until an approved migration design activates the envelope.
- Oregon supports **dual external ingress with one internal execution truth**: native Oregon ingress and Ethereum-compatible EVM ingress, with Ethereum transactions deterministically normalized into Oregon's EVM execution domain rather than creating a second consensus/mempool truth.

Future token/NFT, DeFi, privacy, bridge/interoperability, and AI/oracle/agent domains are planned architectural capabilities, but their detailed semantics are not yet approved. Do not invent or implement their consensus-affecting rules without a separate versioned design and owner approval.

## Architecture rules

- One externally observable rule has exactly one authoritative owner.
- No patch/shim architecture. If all callers can migrate atomically, migrate them to one clean path.
- Core consensus/state crates must not acquire upward dependencies on networking, VM adapters, RPC, wallet, bridge, DeFi, privacy, or AI layers.
- VM adapters do not own Oregon monetary policy, PoW, fork choice, native UTXO validity, storage representation, or peer policy.
- Cross-domain state transitions are explicit, deterministic, bounded, and consensus-reviewed.
- Do not expose RocksDB representation or chainstate internals as convenience APIs to execution/VM layers.
- Preserve fail-closed behavior, bounded attacker-controlled resources, durable-before-publication semantics, and validation-before-relay/publication.
- Production code must not use lint suppression, placeholder TODO/FIXME/HACK paths, or task-numbered architecture.

## Required workflow for architecture or behavior changes

Before implementation of any new subsystem or behavior change:

1. inspect current code/docs/checkpoints;
2. write a versioned design naming authoritative owners and dependency direction;
3. obtain explicit owner approval;
4. create characterization tests/vectors before changing frozen behavior;
5. implement on an isolated branch using test-first development;
6. run all required CI, architecture, security, and mutation gates appropriate to the subsystem;
7. create a new acceptance checkpoint; and
8. integrate to `main` only after a separate explicit integration decision.

If a user request conflicts with an accepted architecture rule, stop and state the conflict. Do not silently rewrite the architecture.

If an architectural detail is intentionally unspecified by the accepted documents, do not fill the gap from assumption. Propose options and obtain approval before implementation.

## Model-specific instruction files

`CLAUDE.md` and `GEMINI.md` are only compatibility entry points. They must not duplicate or override architecture rules. This `AGENTS.md`, the Engineering Constitution, and the Platform Architecture Contract remain authoritative.