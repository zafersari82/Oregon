# Oregon Execution Architecture V1 Design

**Status:** Owner-approved architecture specification; implementation not yet activated

**Date:** 2026-09-05

**Base:** `bf7675bfe17182f77d4c43e2bcbd0c283709d799`

**Parent contracts:**
- `docs/architecture/OREGON_ENGINEERING_CONSTITUTION.md`
- `docs/architecture/OREGON_PLATFORM_ARCHITECTURE_CONTRACT.md`

## 1. Purpose and authority

This specification freezes the execution architecture needed to let Oregon grow into a multi-VM Layer-1 without later rewriting its accepted PoW, native UTXO, persistence, chain-selection, networking, or orchestration foundations.

On 2026-09-05 the owner delegated selection among the remaining execution-architecture options to the lead architecture assistant and instructed that those selections be recorded in the repository. The decisions in this document therefore represent the selected owner-approved direction for this subsystem. That delegation is not a standing permission for future AI agents to alter frozen architecture. Future changes still require the Engineering Constitution amendment process.

This document does not activate smart contracts or mutate accepted M0-M6 runtime behavior. Activation requires a separate implementation branch, tests/vectors, CI/security verification, checkpoint, and explicit `main` integration.

## 2. Inherited platform decisions

The following parent decisions remain non-negotiable:

1. Oregon is Multi-VM by architecture, with EVM and WASM as the first intended VM families.
2. Oregon uses a hybrid state model: native OREG remains UTXO-based; contract execution uses separate account/contract-state domains.
3. Oregon evolves toward one versioned universal Oregon transaction envelope.
4. Native Oregon ingress and Ethereum-compatible ingress converge on one authoritative Oregon execution truth.
5. Existing M0-M6 canonical transaction encoding, txid rules, UTXO semantics, chainstate behavior, durability, mempool behavior, P2P behavior, and validation-before-relay remain frozen until an explicit versioned migration activates new behavior.

## 3. Authoritative owners and dependency boundaries

Future execution work must use these responsibility boundaries.

### 3.1 `oregon-primitives`

Owns canonical serialized execution-domain identifiers, typed internal addresses, universal-envelope wire types once activated, authorization descriptors, receipt/message identifiers, commitment descriptors, and decode/resource bounds for those types.

It does not execute transactions, price resources, verify VM semantics, or own storage representation.

### 3.2 `oregon-consensus`

Owns chain-visible activation schedules, block-level resource limits, base-fee update rules, allowed execution/commitment scheme versions, and the consensus requirement that committed state/receipt roots match authoritative execution.

It does not contain an EVM or WASM interpreter.

### 3.3 `oregon-execution`

Owns the single internal execution pipeline: normalized transaction validation, fee escrow and settlement, execution-domain dispatch, transaction journal/rollback, common call-stack accounting, cross-VM dispatch, common receipts, execution-balance transfers, and generic asynchronous-message lifecycle.

It must not depend on RocksDB representation, peer policy, or node RPC details.

### 3.4 `oregon-contract-state`

Owns logical account/contract state, versioned state-commitment schemes, snapshots/write-sets used by transaction journals, and domain-proof construction/verification interfaces.

Physical persistence remains owned by `oregon-storage`.

### 3.5 `oregon-runtime`

Owns the versioned deterministic host ABI exposed to VM backends and the versioned cross-VM call ABI. It provides no filesystem, network, wall-clock, operating-system randomness, or direct database access.

### 3.6 `oregon-vm-evm`

Owns EVM-specific decoding, revision semantics, gas metering, EVM account/storage semantics, EVM precompile mapping, and translation between the shared Oregon runtime boundary and the selected EVM engine.

It does not own Oregon fees, monetary policy, fork choice, native UTXO validity, or persistence.

### 3.7 `oregon-vm-wasm`

Owns Oregon's deterministic WASM profile, validation, WASM-native resource metering, WASM contract semantics, and translation between the shared Oregon runtime boundary and the selected deterministic WASM engine.

It does not own Oregon fees, monetary policy, fork choice, native UTXO validity, or persistence.

### 3.8 Existing crates

- `oregon-utxo` remains the authority for native UTXO transitions and protocol-reserved execution-backing outputs.
- `oregon-storage` remains the only owner of RocksDB column families, codecs, batches, migrations, and durability mechanics.
- `oregon-chainstate` atomically composes accepted native UTXO and execution-domain block transitions and publishes them only after durable storage succeeds.
- `oregon-mempool` remains the one mempool-policy authority. It may maintain domain-specific indexes/nonce lanes but must not create separate EVM/WASM mempools with separate admission truth.
- `oregon-node` remains orchestration only.
- a future `oregon-rpc` is an ingress/view adapter and must not become a consensus or mempool authority.

The execution crates may depend on lower-level primitives/consensus/runtime interfaces. Core consensus/state crates must not acquire upward dependencies on RPC, wallet, bridge, DeFi, privacy, oracle, or AI application layers.

## 4. Universal Oregon transaction envelope

Oregon will introduce a versioned canonical envelope at an explicit activation boundary. The accepted M0-M6 transaction representation is not silently modified.

The logical V1 envelope contains:

- `envelope_version`
- `chain_id`
- `execution_domain`
- `valid_after_height`
- `valid_until_height`
- `principal`
- optional distinct `fee_payer`
- bounded typed `authorization_proofs`
- fee caps
- bounded domain payload
- optional bounded access hints

All common fields and the domain discriminator are part of the Oregon signing/authorization commitment for native envelope transactions. This prevents a signed payload from being reinterpreted under another execution domain or chain.

`fee_payer` is distinct from `principal` by architecture even when they are normally identical. This reserves clean support for sponsored transactions, session authorization, and future account-abstraction designs without another transaction-format rewrite.

The envelope is a container and dispatch boundary, not a rule owner. Common validity is checked once by `oregon-execution`; payload validity is checked once by the selected authoritative domain.

### 4.1 Canonical transaction identity

Oregon has one canonical internal transaction identifier derived with Oregon's domain-separated hashing from the canonical normalized envelope.

Ethereum-compatible ingress may also have an Ethereum transaction hash. That hash is retained as a compatibility alias/index and exposed through Ethereum-compatible RPC, but it does not become a second Oregon consensus identity.

The mapping `ethereum_tx_hash -> oregon_txid` is deterministic for an accepted normalized Ethereum transaction.

## 5. Dual external ingress and Ethereum normalization

Native Oregon RPC accepts canonical Oregon envelopes.

Ethereum-compatible RPC accepts supported standard signed Ethereum transaction formats. The adapter:

1. validates the Ethereum encoding and signature under the configured EVM revision;
2. validates Ethereum `chainId` against the Oregon EVM chain identifier;
3. derives sender/nonce/fee fields exactly from the signed Ethereum transaction;
4. commits to the exact signed source bytes/hash;
5. deterministically normalizes the transaction into the Oregon EVM execution domain; and
6. submits that normalized transaction to the same authoritative mempool/execution path as native ingress.

The adapter may not add mutable authority-bearing fields that were not covered by the original Ethereum signature.

Ethereum compatibility therefore does not create a second fork-choice rule, block-validity path, state database, fee truth, or mempool.

## 6. Typed universal Oregon address

Oregon uses a canonical typed internal execution address rather than making the entire platform use Ethereum's 20-byte namespace.

The V1 internal form is logically:

`kind: u8 || payload: [u8; 32]`

The 32-byte payload is interpreted only according to `kind`.

Reserved V1 kinds include:

- EVM account/contract
- WASM account/contract
- Oregon execution identity
- protocol/system identity

UTXO ownership remains defined by UTXO locking/spend semantics; a wallet-facing UTXO address is not silently converted into an account balance.

For an EVM address, the 20-byte Ethereum address is left-zero-padded into the 32-byte payload. Ethereum-facing APIs continue to display the standard 20-byte `0x...` address. The internal type tag prevents the same bytes from being confused with a WASM or protocol identity.

Native human-readable Oregon execution addresses use Bech32m-style network-specific encoding. Mainnet, testnet, and devnet use distinct human-readable prefixes. Exact user-facing prefixes are release configuration, not consensus identity.

### 6.1 Contract address derivation

EVM `CREATE` and `CREATE2` address derivation remains Ethereum-compatible within the EVM domain, then maps into the typed EVM internal address.

WASM contract identifiers are deterministic 32-byte Oregon domain-separated hashes over chain id, deployer typed identity, deployer WASM sequence, code hash, deployment salt, and WASM contract-address scheme version. No global administrator assigns contract addresses.

## 7. Authorization framework

Oregon has one typed authorization framework with multiple approved proof schemes.

V1 architecture supports:

- Oregon-native secp256k1 Schnorr authorization with Oregon-specific domain-separated signing commitments;
- Ethereum-compatible secp256k1 ECDSA authorization for Ethereum ingress;
- bounded multi-proof/threshold authorization descriptors; and
- future versioned authorization schemes without changing the envelope format.

The current M0-M6 `SpendVerifier` boundary remains the mandatory UTXO spend-validity boundary until the production native authorization design is activated.

A VM may consume an already-authorized principal/caller context. A VM must not independently redefine whether the outer Oregon transaction was authorized.

Authorization proofs are bounded in count and bytes before expensive verification.

## 8. Replay protection and ordering

Replay protection is layered rather than forcing one nonce model onto every domain.

- Native UTXO spends use outpoint consumption as their native replay protection.
- EVM accounts use Ethereum sender nonce semantics inside the EVM domain.
- WASM/account execution uses a per-account per-domain sequence.
- The Oregon envelope binds chain id, envelope version, domain, authorization context, validity window, and payload commitment.
- Asynchronous messages use unique message ids plus source domain, destination domain, source sequence, and a consumed marker.

A testnet transaction cannot be replayed on mainnet merely by changing an RPC endpoint. An EVM transaction cannot be reinterpreted as a WASM transaction. An async message cannot be consumed twice.

Block execution semantics are canonical block order. Clients may execute independent transactions in parallel only when they prove the result is exactly equivalent to canonical serial order; optimization may never change consensus semantics.

## 9. VM-specific metering with unified Oregon fee settlement

EVM and WASM keep their native deterministic meters.

- EVM uses the gas schedule of the explicitly activated EVM revision.
- WASM uses a versioned Oregon WASM resource schedule covering instructions, memory growth, host calls, cryptographic operations, and state I/O.

Each VM converts native consumption into a deterministic normalized `execution_weight`. The conversion schedule is versioned and consensus-visible. EVM V1 uses a fixed deterministic gas-to-weight mapping; WASM V1 uses a fixed integer-only resource-to-weight schedule established by benchmark-backed activation vectors.

There is one block-level normalized execution-weight budget and bounded per-transaction budgets. VM-native safety limits remain in addition to the normalized budget. There are no permanently reserved block quotas per VM; unused capacity is not stranded by domain.

Exact numeric block/transaction caps are activation parameters and must be frozen by benchmark-backed consensus vectors before the execution milestone is activated. They are not runtime-tunable settings.

## 10. Dynamic base fee and priority fee, with no fee burn

Oregon uses a deterministic dynamic base fee plus an optional priority fee for execution-weight-priced transactions.

Native envelope fee fields support:

- `max_fee_per_weight`
- `max_priority_fee_per_weight`
- `max_execution_weight`

The effective price follows the familiar capped form:

`effective_price = min(max_fee_per_weight, base_fee_per_weight + max_priority_fee_per_weight)`

The base fee for a block is derived only from already-committed prior-block utilization, uses integer arithmetic, has a target utilization, and has a bounded maximum rate of change per block. It never depends on transactions in the block currently being validated.

**OREG fees are not burned.** Both the charged base-fee component and priority-fee component are assigned to the block producer under consensus rules.

Because non-burned base fees do not inherit Ethereum's burn-based miner-incentive properties, Oregon's activation design must use a bounded adjustment rate and explicit adversarial tests for producer self-fill/base-fee manipulation. This is a security requirement, not permission to change the no-burn decision silently.

Pre-execution structural/authentication failure is invalid and is not an executed fee-paying transaction. Once an included transaction passes pre-execution validation and fee escrow succeeds, execution resource consumption is charged even when contract execution reverts. Unused fee escrow is refunded deterministically.

## 11. Fee escrow and transaction rollback boundary

Fee payment is separated from revertible contract state.

The transaction lifecycle is:

1. decode and bounded structural validation;
2. replay/authorization/fee-cap validation;
3. reserve the maximum permitted fee from an authoritative payer source;
4. execute in a transaction-scoped journal;
5. commit or revert execution state according to the execution outcome;
6. charge actual resource use from the reserved fee;
7. refund unused reserve; and
8. settle block-producer fees.

Fee escrow and actually consumed resource fees survive a top-level contract revert. Revertible contract writes do not.

This prevents contracts from obtaining free computation by intentionally reverting.

## 12. UTXO-backed execution OREG

Native OREG issuance remains owned by the UTXO domain. EVM/WASM account balances do not create a second supply authority.

Moving OREG into execution creates protocol-reserved native UTXO backing and atomically credits the selected execution account/domain. Moving OREG back debits execution balance and atomically releases equivalent value into normal spendable native UTXO output(s).

Protocol-reserved execution-backing outputs cannot be spent by ordinary key authorization. They are consumed/reorganized only by the authoritative cross-domain reserve transition.

The mandatory supply invariants are:

`sum(spendable_native_utxo) + sum(protocol_execution_reserve_utxo) = issued_oreg_not_otherwise_locked_by_consensus`

and

`sum(evm_oreg_balances) + sum(wasm_oreg_balances) + sum(other_approved_execution_oreg_balances) = sum(protocol_execution_reserve_utxo)`

No VM opcode, contract, RPC, bridge, oracle, or administrator can mint OREG.

Transfers of backed OREG between EVM and WASM execution balances are explicit cross-domain transitions and do not change total reserve backing.

Execution fees paid to a native block-producer output atomically reduce execution balances and corresponding protocol reserve backing by the exact paid amount while creating the equal native fee output. No supply is created or destroyed by fee settlement.

## 13. Domain-separated state commitments with one extensible header commitment

Oregon commits distinct domain states while avoiding a header redesign for every future domain.

Each execution/state domain publishes a typed commitment descriptor:

- `domain_id`
- `commitment_scheme_id`
- `state_root`

V1 logically includes at least:

- native UTXO state root;
- EVM state root;
- WASM state root;
- execution receipt root;
- asynchronous outbox root;
- asynchronous consumed-message root; and
- execution fee/base-fee state commitment when required by the activated fee design.

These child commitments are canonically ordered and domain-separated, then committed by one Oregon aggregate `state_commitment_root` in the activated block-header extension.

A child root remains independently provable. Light clients and bridges can prove a domain root against the aggregate root and then use the domain's declared proof scheme.

The commitment descriptor includes a scheme id so a future explicitly activated state-tree scheme can change without inventing a second header format.

### 13.1 Initial domain commitment schemes

- EVM V1 uses Ethereum-compatible account/storage commitment semantics where required for EVM proof compatibility.
- WASM V1 uses an Oregon domain-separated 256-bit authenticated state commitment with deterministic key encoding.
- The Oregon aggregate commitment uses Oregon's canonical domain-separated 256-bit hashing rules.

Exact trie/node encoding and proof byte format are implementation-spec details that must be frozen with golden vectors before activation; the `domain_id + scheme_id + root` container is frozen here.

## 14. Separate EVM and WASM state with controlled cross-VM ABI

EVM and WASM do not share raw account/storage databases.

A VM cannot read or mutate another VM's storage directly. Cross-VM interaction goes only through `oregon-runtime`'s versioned call ABI using typed caller/target identities, bounded input/output bytes, a shared transaction resource budget, and the transaction journal.

The common call result is logically:

- success with bounded return data;
- revert with bounded return data; or
- deterministic trap/error code.

Every call frame has its own journal checkpoint. A failed child call reverts that child frame and descendants. The calling contract may handle a returned failure according to its VM semantics. If failure propagates to the top-level transaction, all revertible execution-state changes are rolled back. Consumed execution fees remain charged.

The cross-VM call stack has a consensus-bounded maximum depth. A cross-VM transition never resets the remaining resource budget, caller identity, or replay context.

Reentrancy is not hidden or emulated away. Synchronous cross-VM calls participate in one explicit call stack, allowing contracts to reason about reentrancy under documented runtime semantics.

## 15. Synchronous atomicity and explicit asynchronous messages

Synchronous calls inside one Oregon transaction share the transaction journal and can therefore be atomic across EVM/WASM and execution-balance movements.

Operations that depend on information not deterministically available during current-block execution are asynchronous by definition. This includes external bridges, external oracles, remote AI jobs, and other off-chain or cross-chain dependencies.

The generic async core uses a committed outbox/inbox model. A message has a bounded canonical form containing source domain, destination domain, source sequence, emitted height, expiry policy, payload commitment, and message id.

External systems cannot call directly into consensus execution. A later Oregon transaction must present the approved proof/authorization required by the destination protocol domain and explicitly consume the message. Consumption is exactly once and is committed in state.

The generic message framework does not define bridge trust, oracle trust, privacy proofs, or AI-result truth. Those remain separate threat-modeled protocol designs.

## 16. Deterministic WASM profile

Oregon WASM is a consensus runtime profile, not arbitrary browser/server WASM.

V1 requirements:

- deterministic integer semantics only;
- floating-point instructions disabled;
- threads/shared memory disabled;
- nondeterministic or implementation-dependent proposals disabled unless a later versioned activation proves deterministic semantics;
- bounded code size;
- bounded linear memory;
- bounded stack/call depth;
- bounded table sizes;
- deploy-time validation before code can enter state;
- immutable deployed code bytes at the protocol layer; upgrades use explicit application patterns/new deployments rather than hidden administrator mutation;
- no filesystem, sockets, wall clock, environment variables, OS randomness, or host process access;
- deterministic host functions only; and
- consensus execution uses a deterministic engine/profile. Optional JIT/AOT acceleration may be used only if differential tests prove byte-for-byte equivalent results to the reference consensus execution.

Cryptographic and state host calls are versioned and metered.

## 17. EVM compatibility profile

Oregon supports standard Solidity/EVM tooling without becoming an Ethereum fork.

Rules:

- EVM revision is an explicit consensus activation parameter; nodes never execute an implicit floating `latest` revision.
- Revision changes and precompile changes require versioned activation and differential vectors.
- Ethereum `CREATE`/`CREATE2`, sender nonce, call/revert semantics, gas accounting, logs, and standard 20-byte external addresses are preserved within the supported EVM profile.
- Oregon maps deterministic block context into the EVM environment through one versioned adapter.
- Values that do not have exact Ethereum meaning on Oregon PoW are explicitly specified by the EVM adapter rather than guessed by each client.
- Any PoW-derived EVM entropy field must be documented as miner-influenceable and must not be advertised as secure randomness.
- Ethereum-compatible RPC may expose compatibility views/aliases, but Oregon canonical block and transaction commitments remain authoritative.

The exact first EVM revision and engine/library version are selected and frozen in the implementation plan after compatibility and differential testing; changing a library version may not silently change EVM semantics.

## 18. Receipts, events, and proofs

Every executed envelope produces one canonical Oregon receipt with bounded data. The receipt commits at least:

- Oregon txid;
- execution domain;
- success/revert/trap status;
- normalized execution weight consumed;
- fee charged and fee payer;
- domain state transition commitment/reference;
- bounded events/log commitments; and
- outbound asynchronous message commitments.

EVM-compatible RPC derives Ethereum-style receipt fields from the authoritative Oregon EVM receipt data and may expose the Ethereum transaction-hash alias. It cannot fabricate a different execution result.

Proof APIs identify `domain_id` and `commitment_scheme_id` so verifiers know how to verify the child state proof against the aggregate Oregon state commitment.

## 19. Mempool and block execution

There is one authoritative mempool admission path after universal-envelope activation.

The mempool may maintain specialized indexes for:

- UTXO outpoint conflicts/dependencies;
- EVM nonce lanes;
- WASM sequence lanes;
- fee-payer reservations; and
- async-message consumed conflicts.

Those indexes are accelerators of one policy truth, not separate mempools.

Ethereum RPC submission enters this same path after normalization.

A block executes transactions in exact encoded block order. Transaction execution uses the block's pre-state plus preceding committed transaction effects. Any parallel execution optimization must validate conflict/equivalence and reproduce the same roots/receipts as canonical serial execution.

## 20. Fail-closed durability and reorganization

A block containing execution state is not accepted/published until the native UTXO transition, contract-domain transitions, receipts/messages, fee settlement, undo information, and all consensus commitments are durably written as one accepted chainstate operation under the existing WAL+sync rule.

A storage failure faults the acceptance path rather than publishing partial native or execution state.

Reorganization undo must cover every activated state domain and cross-domain reserve transition atomically. There is no state-domain-specific partial reorg.

## 21. Protocol upgrades

Execution changes activate only through an explicit height-based protocol schedule distributed in node software. There is no administrator key, governance RPC, or contract that can mutate consensus rules at runtime.

The schedule versions at least:

- envelope format;
- authorization schemes;
- VM revisions/profiles;
- runtime/host ABI;
- cross-VM ABI;
- native-to-normalized resource mapping;
- fee parameters/formula version;
- domain commitment schemes; and
- async message format.

Unknown required versions fail closed.

## 22. Security invariants

Implementation and review must preserve all of these invariants:

1. A VM cannot mint native OREG.
2. Sum of execution OREG balances equals protocol-reserved native execution backing.
3. Ethereum ingress cannot bypass Oregon mempool/execution validity.
4. A payload cannot be reinterpreted under a different chain or execution domain.
5. Fee-paying computation cannot become free by reverting.
6. A failed call frame cannot leak partial state writes.
7. Cross-VM calls cannot reset resource budgets or caller identity.
8. Async messages cannot be consumed twice.
9. External bridge/oracle/AI systems cannot synchronously inject unverified state into consensus.
10. VM code cannot access RocksDB, filesystem, sockets, OS randomness, or wall clock.
11. State roots and receipt roots are reproducible across supported architectures.
12. Reorg/recovery cannot publish a mixture of old UTXO state and new execution state or vice versa.
13. No compatibility adapter owns a second transaction, fee, state, or fork-choice truth.
14. Attacker-controlled counts/lengths are bounded before allocation or expensive verification.
15. Protocol-version or commitment-scheme changes fail closed when unsupported.

## 23. Required verification before activation

The execution milestone cannot be accepted without all of the following:

- canonical golden vectors for envelope encoding/signing/txid and typed addresses;
- Ethereum normalization vectors covering supported transaction types and malformed/malleable cases;
- authorization replay/cross-domain/cross-chain mutation tests;
- fee escrow/revert/refund vectors;
- base-fee boundary and producer-manipulation adversarial tests;
- exact UTXO reserve versus execution-balance conservation tests;
- state commitment and proof vectors for every active domain/scheme;
- EVM differential vectors against an independent compatible implementation;
- WASM deterministic vectors on x86_64 and ARM;
- cross-VM nested call/revert/reentrancy/resource-exhaustion tests;
- async duplicate/replay/expiry/consumption tests;
- crash/durable-write/recovery/reorg atomicity tests spanning all active state domains;
- bounded-decoder/allocation tests for every new remotely supplied structure;
- real mempool/block integration tests using native and Ethereum ingress; and
- targeted security mutation tests proving the critical invariants are actually enforced by the test suite.

The existing workspace format, Clippy, rustdoc/docs, architecture scan, and all inherited M0-M6 gates remain mandatory.

## 24. Explicitly deferred subsystem semantics

This design reserves integration boundaries but deliberately does not invent the following independent security models:

- privacy proof system and privacy policy;
- external bridge trust/proof/finality model;
- oracle provider/quorum/truth model;
- AI job/result verification and payment semantics;
- DeFi application protocols;
- fungible/NFT token application standards; and
- production wallet UX/key-recovery policy.

Each gets its own threat model and versioned specification. They must use the execution/runtime/message/authorization/commitment boundaries defined here rather than rewriting the core.

## 25. Rejected architectures

The following shortcuts are rejected:

- one global account state replacing native UTXO;
- EVM-only Oregon;
- WASM-only Oregon;
- raw cross-VM storage access;
- independent EVM and WASM mempools;
- VM-specific fee tokens;
- fee burn in Execution Architecture V1;
- unbacked EVM/WASM OREG balances;
- one global nonce forced onto UTXO, EVM, and WASM;
- 20-byte Ethereum addresses as Oregon's universal internal namespace;
- synchronous bridge/oracle/AI callbacks into consensus;
- floating-point or host-I/O-enabled consensus WASM;
- implicit `latest` EVM semantics;
- runtime administrator keys that can change consensus; and
- temporary shims that duplicate authoritative rules instead of using versioned migration.

## 26. Implementation sequencing

Implementation is decomposed so each stage can be reviewed and rejected independently without half-activating contracts:

1. execution primitives and inactive universal-envelope/address/authorization types;
2. logical contract-state and commitment framework;
3. resource metering, fee escrow, fee-state transition, and execution-backing invariants;
4. shared deterministic runtime/call journal and async message core;
5. EVM backend plus Ethereum-compatible normalization/RPC adapter;
6. deterministic WASM backend;
7. cross-VM calls and execution-balance transfer;
8. unified mempool integration and block execution;
9. durable chainstate integration, reorg/recovery, full vectors/mutations; and
10. only then an activation/checkpoint decision.

No step may activate unfinished consensus behavior merely to make a demo work.
