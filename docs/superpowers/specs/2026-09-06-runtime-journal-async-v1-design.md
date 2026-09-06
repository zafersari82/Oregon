# Oregon Runtime, Journal and Async Core V1 — Stage 4 Design

**Status:** Proposed design for owner review; no implementation or activation approval is asserted.

**Date:** 2026-09-06 UTC.

**Base:** `fe762f7a5670d94a486423327e3a525cec24afb5`, Stage 3B integration through PR #17.

**Base tree:** `cebbc2370e094052872570069c0a149a4581eb99`.

## 1. Authority and scope

This proposal follows Execution Architecture V1 section 27, step 4. The Engineering Constitution, Platform Architecture Contract, Execution Architecture V1, Stage 2 state design, and Stage 3A/3B designs remain normative. Nothing here replaces their accepted encodings, fees, reserve equations, ownership rules or activation boundaries.

The owner requested continuation after Stage 3B closure. Stage 3B has been integrated. This document makes the next architectural decisions reviewable before implementation; it does not label new Stage 4 choices as already accepted.

The objective is one deterministic execution journal with explicit rollback and fee boundaries, a VM-neutral host boundary, and a later generic asynchronous message core. These are three separately verifiable implementation slices. Stage 4A below is the first implementation scope; sections describing 4B/4C constrain their future designs but do not authorize unspecified wire formats or production adapters.

Current block/header/transaction bytes, native UTXO/coinbase behavior, storage schema, mempool, networking and node orchestration stay inactive with respect to execution. No EVM/WASM engine, RPC path, protocol activation or real external proof verifier is delivered by this proposal.

## 2. Existing implementation inspected

| Existing owner/API | Consequence for Stage 4 |
| --- | --- |
| `oregon-contract-state::DomainSnapshot`, `StateSource`, `read_value`, `StateWriteSet`, `apply_write_set` | Reuse checked reads and immutable transitions; never implement another SMT inside execution. |
| Stage 2 present-empty/absence and duplicate-path rules | Preserve `Some([])` versus deletion; collapse repeated raw-key writes before canonical finalization, but never hide a path collision. |
| `oregon-execution::WeightMeter` | One cumulative transaction meter; rollback cannot reset it. Exhaustion is sticky and consumes the authorized maximum. |
| `EscrowBookV1`, `FundingCapabilityV1`, `FeeTermsV1` | Reuse arithmetic and one-shot settlement. The public capability constructor checks data, not live source authorization or current backing. |
| Stage 3B accounting keys, reserve helpers and producer validator | These remain authoritative and inactive; a journal is not permission to mint balances or manipulate a reserve directly. |
| Reserved EVM commitment scheme | Do not treat an EVM descriptor as an Oregon SMT snapshot or claim Ethereum state compatibility. |

## 3. Approaches considered

| Approach | Benefit | Cost / reason for choice |
| --- | --- | --- |
| Bounded stack of frame-local write maps over immutable snapshots | Simple LIFO rollback, explicit parent visibility, no whole-state clone; chosen for 4A | Must budget live maps, copies and merges before allocation. |
| One mutable map plus inverse-operation log | Efficient writes in some workloads | Every overwrite/delete/child merge needs correct inverse bookkeeping; larger rollback audit surface. |
| Snapshot/clone the full execution state per frame | Straightforward prototype | Work and memory depend on total state rather than transaction bounds; rejected for production foundation. |

The chosen implementation is a stack of bounded overlays, not a second state database. The final write set still goes through the Stage 2 owner.

## 4. Authoritative owners and dependency direction

- `oregon-execution` owns transaction lifecycle, frame stack, journal visibility, rollback, metering composition and later message lifecycle.
- `oregon-contract-state` owns checked logical reads, canonical write sets, SMT transitions and proofs. It never imports execution/runtime.
- A later `oregon-runtime` owns versioned host/call ABI data and traits, depending only on lower primitives. It never imports execution, VM implementations or storage. Execution implements that boundary; VM adapters consume it.
- `oregon-primitives` owns eventual receipt/message bytes and IDs, not frame authority or mutation.
- `oregon-consensus` owns activation/version policy and block budgets; no upward execution/runtime dependency is introduced.
- `oregon-storage` and `oregon-chainstate` retain persistence and atomic durable publication ownership respectively.

Stage 4A adds only a downward `oregon-execution -> oregon-contract-state` dependency. It does not scaffold an unused runtime crate. Stage 4B introduces the ABI only when an execution consumer and test adapter exercise it together. No new production API exists just to reserve a name.

## 5. Incremental delivery

| Slice | Deliverable | Separate acceptance evidence |
| --- | --- | --- |
| 4A | Bounded frame journal over explicitly supplied Oregon SMT snapshots; checked reads, nested commit/revert, atomic unpublished finalization | Journal model tests, independent vectors, limits/corruption tests and targeted mutations |
| 4B | Runtime host/call boundary, transaction coordinator, single meter, fee settlement and bounded receipt composition | Reviewed ABI/outcome/receipt design, adversarial test adapter, fee/revert/fault vectors |
| 4C | Bounded canonical messages, outbox/consumed state and authorized delivery composition | Reviewed wire/keys/expiry/failure policy, independent vectors and duplicate/replay/reorg tests |

Every slice requires its own plan, expected-red evidence, exact-head CI, checkpoint and separate integration decision. Completing 4A must not be reported as completion of all Stage 4 or VM execution.

## 6. Stage 4A journal context and state access

The journal is a local, unpublished transaction object. Construction receives an immutable set of unique domain snapshots and a `StateSource` whose records remain stable for the journal lifetime, plus validated structural limits. Its internal context contains `chain_id: u64`, `height: u64`, `parent_block_hash: Hash256` and `txid: Hash256`; these are copied from the trusted caller, not independently authenticated by this library. No new wire encoding is introduced for this local context. Domains are fixed at construction and cannot be added by a child frame.

4A supports only Oregon SMT domains: `Wasm`, `ExecutionAccounting`, `ExecutionReceipts`, `AsyncOutbox`, `AsyncConsumed`, `FeeState`. `NativeUtxo` and `Evm` are rejected. Inclusion means a trusted execution owner can stage logical writes; it does not authorize a future VM to write these domains. There is no VM-facing journal API in 4A.

The constructor accepts a borrowed snapshot slice, rejects more than six snapshots before copying, and rejects duplicate domains and an empty snapshot set. Snapshots describe Oregon SMT roots explicitly; no generic scheme descriptor is silently converted into this representation. Missing/corrupt referenced records are detected through the existing checked read/finalization path. Construction does not traverse the entire persistent tree.

For a raw `(domain, key)` read:

1. validate the allowed domain and key length;
2. search from current frame toward the root frame;
3. a found put returns its bytes, including a present empty value;
4. a found deletion returns absence and stops the search;
5. otherwise call the authoritative Stage 2 `read_value` against that domain's base snapshot.

No raw node/value lookup shortcut may bypass Stage 2 hash/depth/value checks. An overlay read need not read a shadowed base value; finalization still checks that base through `apply_write_set`, including no-op and deletion cases.

## 7. Frame lifecycle and authority

Construction creates one root frame at depth 1. Beginning a child pushes an empty map after validating depth and frame-creation limits. Only the top frame can be committed or reverted. A child handle must not permit access to a parent frame or another journal.

Prefer operations on the current top frame with no externally supplied numeric frame ID. If an implementation requires tokens for borrowing, tokens must be opaque, journal-bound, generation-bound and non-reusable; those invariants need distinguishing tests before introducing that API.

- Child commit merges child writes into its immediate parent with last-write-wins for the same raw `(domain, key)`; the child is then removed.
- Child revert discards child writes and removes the child.
- Committing a child does not publish it: a later ancestor revert also discards all child effects merged into that ancestor.
- Ending the root through child APIs is an error. Transaction finalization consumes the journal instead.
- Finalization with open children is rejected. Dropping a journal publishes nothing.

Write-map ordering is numeric domain then lexicographic raw-key bytes for predictable iteration. Canonical commitment ordering remains the Stage 2 path-hash ordering; these are deliberately different responsibilities.

## 8. Bounded resources and rejection atomicity

Proposed inactive structural ceilings for 4A are:

| Limit | Ceiling |
| --- | --- |
| Live frame depth, including root | 64 |
| Total frames created, including root | 4,096 |
| Live write entries across all frames | 65,536 |
| Retained raw key and value bytes across all frame maps | 16,777,216 |
| Single raw key / present value | Existing Stage 2 1,024 / 1,048,576 bytes |
| Final writes in each domain | Existing Stage 2 65,536 entries |

A validated `JournalLimitsV1` may select lower positive limits for tests or an eventual approved consensus configuration. These are not activation parameters or operator-tunable consensus values. Raising a ceiling requires a versioned review. Total-frame count is monotonic and is not refunded by revert, preventing sibling churn from evading the transaction work bound.

Entry and byte budgets count each retained frame copy, not only distinct keys after collapse. Replacing an entry deducts its old retained bytes and adds its new bytes with checked arithmetic. Empty values and deletion markers both consume an entry; keys always count toward bytes. Map/container overhead is bounded by entry and frame ceilings separately.

All domain/length/count/byte checks occur before copying caller input or inserting state. The public write entry point takes borrowed input so it can reject an oversized value before making its own copy. Child merge must precompute budget effects and use ownership moves where possible; it must not clone an entire parent map outside the accounting budget. Finalization consumes the journal and moves its writes; temporary node/value allocations remain bounded by Stage 2 algorithms and write/key/value ceilings. Activation needs benchmarks for worst-case transition memory and lower economic limits, not an assumption that the structural ceiling is a safe block budget.

Rejected push/write/merge operations leave maps and frame position unchanged. Resource-limit errors are typed separately from lifecycle misuse and state corruption. The journal does not invent fees for them; 4B specifies outcome mapping and host charging before these operations become VM-visible.

## 9. Unpublished finalization and failure semantics

4A finalization consumes a journal with exactly the root frame open. It takes either committed or reverted intent; this is an internal foundation API, not authority to declare a transaction valid.

Committed intent groups final raw-key writes by domain, constructs `StateWriteSet` using the existing owner and applies each to its corresponding base snapshot. Reverted intent discards all frame writes and returns unchanged domain roots with no new node/value records.

The returned bundle records every participating domain's old and new root in numeric domain order and the successful immutable `StateTransition` outputs for changed domains. It also carries the transaction context identity supplied by the trusted caller so a future coordinator can reject cross-context attachment. That identity is data, not authentication.

No result is returned until all domains succeed. If any domain has duplicate/colliding paths, corrupt or missing source records, or a transition failure, discard every staged transition and return an error. Neither the base source nor an active root set is mutated. A partial successful prefix must never escape through a callback, shared map or incremental public result.

For a reverted journal, unchanged roots are not proof that untouched state is valid: the trusted base snapshot is still a prerequisite. This library does not audit all state on every transaction.

## 10. Stage 4B fee and call composition requirements

These requirements guide the separate 4B design, which must freeze exact ABI/error/receipt bytes before code is written:

1. Pre-escrow validation binds chain, domain, transaction, principal/payer, authorization and fresh source state. A caller-constructed `FundingCapabilityV1` is not sufficient evidence. A production adapter must obtain it from authoritative source validation; test-only adapters must remain visibly test-only.
2. One transaction coordinator owns the escrow book, meter, root journal and receipt assembly. Raw host access cannot mutate accounting, fee, receipt or async system keys. VM-specific storage access is scoped to the active callee and its state backend.
3. Root fee reservation occurs outside contract rollback. Revertible effects see only spendable value after reservation. Settlement reuses Stage 3B exactly once; arithmetic is not copied into runtime or a VM adapter.
4. There is one shared Stage 3A cumulative meter. Parent/child frame rollback never refunds consumed work or resets per-domain counters. Full transaction-meter exhaustion is sticky and maps to a top-level resource-exhausted outcome. Later VM-native child gas rules must not be guessed from this global budget.
5. Ordinary child revert/trap discards child and descendant writes/events/outbox effects; the caller may handle the bounded failure where its approved VM semantics permit. Fatal state corruption, invariant failures and commitment failures cannot be caught and converted into success.
6. Top-level committed/reverted/resource-exhausted execution settles actual consumed work and produces exactly one receipt. Pre-escrow invalidity produces no executed receipt/fee. Fatal failure invalidates the whole candidate block overlay, including provisional fees, and publishes nothing.
7. Fee effects, payer accounting sequence, receipt insertion and block fee totals survive contract revert only within a successfully accepted candidate. They are still undone by block rejection/reorg. A journal must never expose a generic public `non_revertible_write` host function.
8. Failed or double finalization cannot publish a second receipt, refund or fee total. Settlement, journal result and receipt are bound to the same transaction and escrow.
9. Events/return data need per-item and aggregate byte/count bounds before copy. Event/outbox truncation is not a successful execution result.

4B must choose numeric inactive ABI bounds, deterministic host charge scheduling, stable trap classes, return-data semantics and canonical receipt commitments in its own reviewable design. No placeholder backend or generic unrestricted closure is a production executor.

## 11. Stage 4C asynchronous core requirements

The generic core is committed state, not background threads or a network queue. Existing `AsyncOutbox` and `AsyncConsumed` commitment domains are reused; no new header root is invented.

Messages bind version, chain, source/destination domains and identities, source sequence, emitted height, expiry policy and payload commitment. Construction uses the emitting transaction's execution context, not caller-supplied authority. Same-transaction journal emission is revertible with its frame and cannot become externally deliverable before durable acceptance.

Consumption requires a later transaction, a committed source message, exact destination/chain binding, expiry validation, destination-protocol authorization and an absent consumed marker in the current authoritative overlay. There is no default-allow verifier or `authorized: bool` parameter. A generic core cannot determine bridge finality, oracle truth or AI-result validity by itself.

The proposed delivery policy for review is: an authorized included delivery consumes once even if destination execution deterministically reverts; delivery fees and the consumed marker survive that contract revert while destination writes roll back. Invalid pre-escrow delivery does not consume. Fatal block failure or reorg undoes consumption with all other candidate effects. Retrying an application action uses a newly authorized message, not replay of a consumed message. This avoids unlimited repeated attempts under one authorization, but applications must explicitly design their retry behavior.

4C's separate specification must fix exact message bytes/hash domain, key layouts, sequence ownership, expiry boundary (including no-expiry representation), delivery failure receipt and source-proof context before implementation. It must also decide committed-message retention and prove that pruning cannot resurrect replay. No automatic expiration cleanup, refunds, bridge verifier or message cancellation semantics are inferred here.

## 12. Durable publication and reorg composition

Journal results are proposals. The later chainstate integration alone composes native UTXO/reserve, execution domain roots, fee/accounting changes, receipts/messages, full undo and accepted tip state in one WAL-enabled synchronous batch.

Accepted state becomes visible only after durable success. On a write failure the acceptance session faults; no domain or receipt may already have been published. Reorg restores the entire parent root set and native/reserve undo together, then recreates authorization capabilities from restored state. Forward-run escrow tickets, frame tokens and consumed-message caches are not replay authority.

Stage 4A tests immutable parent-root readability and all-or-nothing local output. They must not be described as durable crash/reorg tests. RocksDB codecs, retention/GC and chainstate fault injection remain later work.

## 13. Stage 4A verification requirements

Test-first contracts must distinguish:

- child reads parent/root/base; sibling isolation; grandchild commit then parent revert;
- repeated put/delete/put, present-empty versus absence, and deletion shadowing a parent value;
- child merge ordering and identical final root for equivalent legal execution traces;
- root misuse, finalize with open child, unknown/duplicate/unsupported domain;
- exact and one-over depth, total-frame, entry, aggregate-byte and key/value bounds;
- overwrites in multiple frames counted separately; reverted siblings do not reset total-frame count;
- rejection leaves frame maps unchanged and does not allocate from the oversized caller length;
- first domain succeeds but a later domain is corrupt: no bundle and no source mutation;
- inherited checked missing/corrupt old-value handling for replacement, deletion and no-op;
- parent roots stay readable after successful finalization; drop/revert yields no records;
- excluded EVM/native domains never enter the Oregon SMT journal.

Use a small independent reference model that copies bounded maps per frame in tests only. Generate literal trace/final-value/root vectors in Python using the established Oregon BLAKE3 construction and a state oracle independent from Rust. Include nested outcomes and nonempty base snapshots; do not generate expected roots with the production implementation under test. Run consumers on x86_64 and ARM.

Named mutation targets: `child_revert_leaks`, `ancestor_revert_keeps_child`, `delete_falls_through`, `empty_value_becomes_absent`, `wrong_domain_allowed`, `frame_depth_bypass`, `frame_count_refunded`, `retained_bytes_undercounted`, `entry_limit_bypass`, `open_child_finalize_allowed`, `partial_domain_result_published`, `unchecked_base_read`. Each kill requires the intended test to execute and fail; compile errors are not kills. Restoration and clean focused rechecks remain mandatory.

Full exact-head workspace/all-target tests, architecture/dependency scan, inherited 3/3 address, 9/9 envelope, 17/17 state, 13/13 resource and 14/14 fee mutation gates, rustdoc/docs, format and warnings-denied Clippy are required. Any later doc/checkpoint head needs its own CI. No successful ancestor is substituted.

## 14. Review and implementation entry

The first implementation plan should touch `oregon-execution` journal modules/tests and its downward state dependency, independent vectors/mutation scripts and CI wiring. Stage 2 algorithms remain unchanged unless a demonstrated defect requires a separately scoped repair. Names follow durable responsibilities (`journal`, `frame`, `transition` as needed), never task numbers.

Before implementation: review this proposed decomposition and 4A frame/resource/error semantics, then write the 4A test-first implementation plan. The 4B/4C contracts above are direction and explicitly identified future decisions; they are not a substitute for those slices' exact ABI/wire designs. No main integration or activation is included in approval of a design or implementation plan.
