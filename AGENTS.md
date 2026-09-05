# Oregon AI Agent Instructions

This file is the repository-root entry point for AI coding agents and human contributors using agentic tools.

## Mandatory reading before any change

Read these files before proposing or implementing Oregon changes:

1. `docs/architecture/OREGON_ENGINEERING_CONSTITUTION.md`
2. `docs/architecture/OREGON_PLATFORM_ARCHITECTURE_CONTRACT.md`
3. `docs/superpowers/specs/2026-09-05-execution-architecture-design.md` whenever work touches execution, VM, fees, contract state, token/NFT/DeFi integration, privacy boundaries, bridge boundaries, or AI/oracle boundaries
4. the latest accepted checkpoint under `docs/checkpoints/` for every subsystem you will touch
5. any additional versioned design/spec named by the current task

These documents are normative. Do not infer a different architecture from surrounding code, another blockchain, a framework default, or a convenience shortcut.

## Current accepted baseline

The accepted implementation baseline is M6. Existing M0-M6 consensus, PoW, transaction/block encoding, UTXO, storage, chainstate, mempool, networking, peer, synchronization, and node-orchestration behavior is frozen unless a separately approved versioned amendment explicitly changes it.

Do not reinterpret a future architecture decision as permission to mutate current accepted behavior.

## Frozen future platform direction

Owner-approved platform decisions are recorded in `docs/architecture/OREGON_PLATFORM_ARCHITECTURE_CONTRACT.md` and `docs/superpowers/specs/2026-09-05-execution-architecture-design.md` and must be preserved:

- Oregon is architected as **Multi-VM**, with EVM and WASM as the first intended VM families.
- Oregon uses a **hybrid state model**: native OREG remains UTXO-based; EVM and WASM use separate contract/account state domains.
- Future platform transactions use **one versioned Oregon universal transaction envelope** with explicit execution-domain dispatch; current M0-M6 transaction encoding remains frozen until an approved migration design activates the envelope.
- Oregon supports **dual external ingress with one internal execution truth**: native Oregon ingress and Ethereum-compatible EVM ingress, with Ethereum transactions deterministically normalized rather than creating a second consensus/mempool truth.
- VM-native deterministic metering maps into **one normalized Oregon weight and fee market**.
- Oregon execution fees use dynamic base fee + optional priority fee with **no fee burn** under Execution Architecture V1.
- EVM/WASM OREG balances are **1:1 backed by protocol-reserved native UTXO value**; no VM can mint native OREG.
- EVM and WASM state remain separate and communicate only through a bounded, versioned Oregon runtime/cross-VM ABI.
- Synchronous execution uses journaled call-frame rollback; external bridge/oracle/AI dependencies use explicit asynchronous messages/proofs.
- Internal execution addresses are typed rather than globally forcing Ethereum's 20-byte namespace.
- Replay protection is domain-native plus Oregon envelope/domain/chain binding.
- OREG is the only protocol-level native/fee asset in V1. Tokens, NFTs, and DeFi are contract standards/applications rather than alternate native supply systems.
- Privacy is opt-in through a future shielded domain with OREG conservation; it does not silently replace the transparent native UTXO ledger.
- Nondeterministic AI inference, HTTP lookup, and external oracle responses never execute directly as validator consensus truth.

Detailed privacy proof systems, bridge trust/finality parameters, oracle truth models, AI-result verification protocols, DeFi application economics, and production wallet recovery remain separate threat-modeled specifications. Do not invent them from convention or personal preference.

## Architecture rules

- One externally observable rule has exactly one authoritative owner.
- No patch/shim architecture. If all callers can migrate atomically, migrate them to one clean path.
- Core consensus/state crates must not acquire upward dependencies on networking, VM adapters, RPC, wallet, bridge, DeFi, privacy, or AI layers.
- VM adapters do not own Oregon monetary policy, PoW, fork choice, native UTXO validity, storage representation, or peer policy.
- Cross-domain state transitions are explicit, deterministic, bounded, and consensus-reviewed.
- Do not expose RocksDB representation or chainstate internals as convenience APIs to execution/VM layers.
- Preserve fail-closed behavior, bounded attacker-controlled resources, durable-before-publication semantics, and validation-before-relay/publication.
- Protocol upgrades are explicit height-based software activations; there is no runtime administrator key or governance RPC that can rewrite consensus.
- Production code must not use lint suppression, placeholder TODO/FIXME/HACK paths, or task-numbered architecture.

## Required workflow for architecture or behavior changes

Before implementation of any new subsystem or behavior change:

1. inspect current code/docs/checkpoints;
2. write a versioned design naming authoritative owners and dependency direction;
3. obtain explicit owner approval or operate under a current explicit owner delegation that covers the specific design choice;
4. create characterization tests/vectors before changing frozen behavior;
5. implement on an isolated branch using test-first development;
6. run all required CI, architecture, security, differential, and mutation gates appropriate to the subsystem;
7. create a new acceptance checkpoint; and
8. integrate to `main` only after a separate explicit integration decision.

A delegation to select architecture options is not permission for an AI agent to alter already-frozen decisions. If a requested change conflicts with an accepted architecture rule, stop and state the conflict. Do not silently rewrite the architecture.

If an architectural detail is intentionally unspecified by the accepted documents, do not fill the gap from assumption unless the owner has explicitly delegated that class of decision and the result is first recorded in a versioned repository design before implementation.

## Model-specific instruction files

`CLAUDE.md` and `GEMINI.md` are only compatibility entry points. They must not duplicate or override architecture rules. This `AGENTS.md`, the Engineering Constitution, the Platform Architecture Contract, and applicable versioned design specs remain authoritative.
