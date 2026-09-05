# Oregon Platform Architecture Contract

**Status:** Accepted architecture decisions; implementation not yet activated

**Accepted on:** 2026-09-05

**Applies to:** all Oregon work that introduces or prepares smart contracts, virtual machines, contract state, token/NFT systems, privacy, bridges, DeFi, or AI/oracle/agent capabilities

**Authority:** This document is normative together with `docs/architecture/OREGON_ENGINEERING_CONSTITUTION.md`. It records owner-approved platform decisions that future implementation designs must preserve.

## 1. Compatibility with the accepted M0-M6 baseline

This contract does not rewrite, reinterpret, or silently migrate any accepted M0-M6 rule.

The existing Oregon monetary rules, RandomX proof of work, canonical transaction/block rules, UTXO transitions, storage durability, chain selection, mempool policy, P2P protocol, peer lifecycle, synchronization, and validation-before-relay behavior remain frozen until a later versioned design explicitly changes a rule under the Engineering Constitution change-control process.

In particular, the existing canonical UTXO transaction representation is not replaced merely because this contract reserves a future universal transaction envelope. Any transition from the accepted transaction format must be explicit, versioned, characterized by vectors/tests, isolated on its own implementation branch, and accepted by a new checkpoint.

## 2. Frozen decision A — Oregon is Multi-VM by architecture

Oregon is not an EVM-only chain and is not a WASM-only chain.

The platform architecture must support multiple deterministic virtual-machine backends behind one Oregon execution boundary. The first two intended VM families are:

- an EVM-compatible backend for Solidity/Ethereum ecosystem compatibility; and
- a WASM-compatible backend for Oregon-native contracts and applications.

The shared execution boundary, not any individual VM, owns Oregon-level execution semantics such as dispatch, deterministic resource accounting interfaces, state-transition publication, receipts/outputs, and consensus-visible execution results.

A VM backend is an adapter/executor. It must not become the owner of Oregon monetary policy, PoW, chain selection, RocksDB representation, peer policy, or native UTXO validity.

Adding, removing, or materially changing a VM backend is a versioned protocol change. No implementation may make EVM assumptions part of the permanent Oregon core merely for short-term tooling convenience.

## 3. Frozen decision B — Oregon uses a hybrid state model

Native OREG value remains rooted in the accepted UTXO model.

Smart-contract execution uses a separate account/contract-state domain suitable for EVM and WASM execution. The contract-state domain must not replace the native UTXO domain or duplicate its ownership rules.

Cross-domain movement or interaction is allowed only through explicit, deterministic, consensus-reviewed interfaces. VM code must not reach directly into UTXO, chainstate, storage representation, or mempool internals.

Any milestone that activates contract execution must make the accepted result of both the native UTXO transition and the contract-state transition consensus-verifiable within the block transition. The exact commitment/root encoding is intentionally not defined by this decision record and therefore may not be invented ad hoc during implementation; it requires its own versioned execution/state design before code is written.

## 4. Frozen decision C — future platform transactions use one Oregon universal envelope

Oregon will evolve toward one versioned native transaction envelope that can dispatch to distinct execution domains without forcing every domain to share identical payload semantics.

The envelope must provide one Oregon-level framing/identity boundary and an explicit domain/type discriminator. Domain-specific payloads remain owned by their corresponding authoritative subsystem.

Intended domains include native UTXO actions, EVM execution, WASM execution, and later explicitly designed protocol domains.

The universal envelope must not create multiple competing transaction-validity algorithms. Common envelope validity is owned once; payload validity is delegated once to the authoritative domain owner.

The accepted M0-M6 transaction encoding remains frozen until a separate migration design defines how the envelope is introduced. No agent may silently mutate the current transaction encoding, txid rules, witness semantics, mempool identity, or block commitment rules in the name of this future architecture.

## 5. Frozen decision D — dual external ingress, single internal execution truth

Oregon must support both:

1. native Oregon transaction/RPC ingress; and
2. Ethereum-compatible ingress for EVM users and tooling.

Standard Ethereum transaction formats may be accepted at an Ethereum-compatible RPC boundary when the EVM milestone is implemented. That compatibility boundary must validate and deterministically normalize the request into Oregon's EVM execution domain.

Ethereum-compatible ingress is an adapter, not a second consensus system. It must not introduce a separate authoritative mempool, chain state, fork-choice rule, fee truth, or EVM-only block-validity path.

MetaMask, Solidity tooling, ethers/web3-style clients, and similar Ethereum ecosystem tools should be supportable without making Oregon an Ethereum fork or making Ethereum transaction semantics the permanent container for WASM or future Oregon-native domains.

## 6. Reserved platform domains — planned, but not yet semantically frozen

The architecture is intentionally being prepared for the following future capabilities:

- token and NFT standards;
- DeFi applications and protocol primitives;
- privacy capabilities;
- bridge/interoperability capabilities; and
- AI/oracle/agent capabilities.

Their presence on this list means the architecture must not make them impossible or force a later core rewrite. It does **not** authorize an agent to invent their consensus rules, cryptography, trust model, bridge security model, privacy model, oracle authority, token economics, or AI execution semantics.

Each such domain requires its own versioned design, explicit owner approval, threat model, authoritative rule ownership, tests/vectors, isolated implementation, and acceptance checkpoint before activation.

## 7. Non-negotiable architectural consequences

Future Oregon work must preserve all of the following:

- Native OREG remains UTXO-based unless a separately approved protocol amendment explicitly changes that rule.
- Contract account/state remains a distinct state domain rather than leaking account semantics into the UTXO owner.
- EVM and WASM share an Oregon execution boundary rather than defining parallel block-consensus systems.
- Ethereum compatibility is an ingress/tooling adapter and execution backend, not Oregon's global architecture.
- Future protocol domains extend the universal Oregon envelope through versioned, bounded discriminants rather than introducing unrelated top-level transaction consensuses.
- Cross-domain calls are explicit and deterministic; no VM receives direct storage or chainstate representation access.
- Existing fail-closed durability, one-authoritative-owner, bounded-resource, and validation-before-publication/relay rules continue to apply.
- Compatibility shims or duplicate rule implementations are forbidden when callers can migrate to one authoritative path.

## 8. Explicitly prohibited shortcuts

An implementation must be rejected if it does any of the following without an approved architecture amendment:

- converts Oregon into an EVM-only chain;
- converts Oregon into a WASM-only chain;
- replaces native OREG UTXO state with an account balance model;
- embeds contract account state directly into UTXO implementation internals;
- gives EVM/WASM code direct RocksDB column-family or chainstate-internal access;
- creates independent Ethereum and Oregon consensus/mempool truths;
- changes current canonical transaction encoding as an incidental refactor;
- duplicates fee, authorization, state-transition, or block-validity rules in multiple VM adapters;
- implements privacy, bridge, DeFi, NFT, token, oracle, or AI consensus semantics from assumption rather than an approved design; or
- introduces a temporary patch/shim that violates the one-owner architecture because a proper migration was considered inconvenient.

## 9. Change control

These four accepted decisions are frozen architectural constraints:

1. Multi-VM architecture with EVM and WASM as first intended VM families;
2. hybrid native-UTXO plus contract account/state model;
3. one future versioned Oregon universal transaction envelope; and
4. dual external ingress with Ethereum compatibility normalized into one Oregon EVM execution path.

Changing one of them requires, before implementation:

1. a versioned architecture-amendment document that names the existing rule and proposed replacement;
2. explicit owner approval;
3. characterization tests or vectors that make the behavioral difference observable;
4. an isolated implementation branch;
5. full required CI/security verification; and
6. a new acceptance checkpoint followed by a separate `main` integration decision.

No AI agent, contributor, refactor, dependency upgrade, compatibility task, or feature request may bypass this process.

## 10. Instructions for AI coding agents

Before proposing or implementing platform work, an AI agent must read:

1. repository-root `AGENTS.md`;
2. `docs/architecture/OREGON_ENGINEERING_CONSTITUTION.md`;
3. this Platform Architecture Contract; and
4. the latest accepted checkpoint relevant to the subsystem being changed.

If a requested implementation would conflict with this contract, the agent must stop and surface the conflict instead of silently choosing a new architecture.

If this contract intentionally leaves a later subsystem's semantics unspecified, the agent must not fill the gap from convention, another blockchain, or personal preference. It must first produce a versioned design for owner review.

This contract freezes architecture direction, not the detailed implementation of future subsystems. Exact VM engines, gas schedules, contract-state commitment format, cross-domain call ABI, token standards, privacy construction, bridge verification model, DeFi primitives, and AI/oracle semantics remain subjects for their respective approved designs.