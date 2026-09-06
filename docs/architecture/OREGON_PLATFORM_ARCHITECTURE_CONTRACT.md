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

Smart-contract execution uses separate account/contract-state domains suitable for EVM and WASM execution. Contract-state domains must not replace the native UTXO domain or duplicate its ownership rules.

Cross-domain movement or interaction is allowed only through explicit, deterministic, consensus-reviewed interfaces. VM code must not reach directly into UTXO, chainstate, storage representation, or mempool internals.

Any milestone that activates contract execution must make the accepted result of both the native UTXO transition and execution-domain transitions consensus-verifiable within the block transition.

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

## 6. Frozen execution direction and reserved future domains

The detailed owner-approved execution architecture is normative in:

`docs/superpowers/specs/2026-09-05-execution-architecture-design.md`

That specification freezes, among other things, VM-specific deterministic metering with one normalized Oregon weight/fee market, no fee burn, UTXO-backed execution OREG, domain-separated commitments, separate EVM/WASM state, versioned cross-VM calls, synchronous journaled execution, explicit async messages, typed internal addresses, layered replay protection, and a multi-scheme authorization framework.

The following future-domain directions are also fixed at architecture level while their cryptographic/application details still require their own threat-modeled specifications:

- OREG remains the only protocol-level native asset and protocol-level fee asset in V1.
- Fungible tokens, NFTs, and DeFi are smart-contract standards/applications rather than new built-in consensus asset types when contracts can own the rule cleanly.
- Privacy is opt-in through a future shielded domain that must preserve publicly verifiable OREG conservation; it does not silently replace the transparent native UTXO ledger.
- Bridges use explicit asynchronous proof/message boundaries and may not gain administrator mint authority over native OREG.
- AI/oracle work uses asynchronous jobs/messages and deterministic proof/attestation verification; nondeterministic model inference, HTTP lookups, or external service responses do not execute as validator consensus truth.

The presence of a future domain does not authorize an agent to invent its proof system, trust model, economics, quorum, disclosure model, or application semantics. Those details require a separate versioned design, explicit owner-approved direction under the current delegation/process, threat model, tests/vectors, isolated implementation, and acceptance checkpoint before activation.

## 7. Non-negotiable architectural consequences

Future Oregon work must preserve all of the following:

- Native OREG remains UTXO-based unless a separately approved protocol amendment explicitly changes that rule.
- Contract account/state remains distinct from native UTXO ownership semantics.
- EVM and WASM share an Oregon execution boundary rather than defining parallel block-consensus systems.
- Ethereum compatibility is an ingress/tooling adapter and execution backend, not Oregon's global architecture.
- Future protocol domains extend the universal Oregon envelope through versioned, bounded discriminants rather than introducing unrelated top-level transaction consensuses.
- Cross-domain calls are explicit and deterministic; no VM receives direct storage or chainstate representation access.
- Existing fail-closed durability, one-authoritative-owner, bounded-resource, and validation-before-publication/relay rules continue to apply.
- Compatibility shims or duplicate rule implementations are forbidden when callers can migrate to one authoritative path.
- No runtime administrator key, governance RPC, bridge operator, oracle, or AI service can alter consensus rules or mint native OREG.

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
- creates VM-specific fee tokens or unbacked execution OREG;
- implements synchronous external bridge/oracle/AI callbacks into consensus;
- promotes contract tokens/NFTs into native OREG or native supply accounting;
- implements privacy, bridge, DeFi, NFT, token, oracle, or AI security semantics from assumption rather than an approved design; or
- introduces a temporary patch/shim that violates the one-owner architecture because a proper migration was considered inconvenient.

## 9. Change control

The following accepted decisions are frozen architectural constraints:

1. Multi-VM architecture with EVM and WASM as first intended VM families;
2. hybrid native-UTXO plus separate execution account/state domains;
3. one future versioned Oregon universal transaction envelope;
4. dual external ingress with Ethereum compatibility normalized into one Oregon EVM execution path; and
5. all execution-level constraints marked normative by `docs/superpowers/specs/2026-09-05-execution-architecture-design.md`.

Changing one of them requires, before implementation:

1. a versioned architecture-amendment document that names the existing rule and proposed replacement;
2. explicit owner approval or a later explicit owner delegation that specifically authorizes the replacement;
3. characterization tests or vectors that make the behavioral difference observable;
4. an isolated implementation branch;
5. full required CI/security verification; and
6. a new acceptance checkpoint followed by a separate `main` integration decision.

No AI agent, contributor, refactor, dependency upgrade, compatibility task, or feature request may bypass this process.

## 10. Instructions for AI coding agents

Before proposing or implementing platform work, an AI agent must read:

1. repository-root `AGENTS.md`;
2. `docs/architecture/OREGON_ENGINEERING_CONSTITUTION.md`;
3. this Platform Architecture Contract;
4. `docs/superpowers/specs/2026-09-05-execution-architecture-design.md` for execution, VM, fee, contract-state, token/NFT/DeFi integration, privacy-boundary, bridge-boundary, or AI/oracle-boundary work; and
5. the latest accepted checkpoint relevant to the subsystem being changed.

If a requested implementation would conflict with these contracts, the agent must stop and surface the conflict instead of silently choosing a new architecture.

If a contract intentionally leaves a later subsystem's detailed security semantics unspecified, the agent must not fill the gap from convention, another blockchain, or personal preference. It must first produce the required versioned design under the repository's architecture process.

The contracts freeze architecture direction. Activation constants, exact first EVM engine/revision, state-tree byte encodings, privacy proof construction, bridge proof/quorum parameters, oracle truth model, and AI-result verification protocol remain separate versioned implementation/security decisions where explicitly stated by the execution specification.
