# Oregon — Trust & Verification Roadmap

**Status:** queued future work; documentation only; no protocol activation

**Date:** 2026-09-06

**Start condition:** begin this program only after the current Stage 4A bounded execution-journal slice has its own exact-head verification, checkpoint, separate owner integration decision, and successful integration into `main`.

This roadmap records the next trust-building work requested by the owner. It does not change consensus rules, transaction/block bytes, activation state, mining economics, reserve semantics, or the accepted Stage 4 decomposition. Stage 4B and Stage 4C remain separate runtime design/implementation slices; this document is a verification and public-evidence track, not permission to activate them.

## 1. Goal

Oregon should be able to make unusually strong security claims only when those claims are backed by reproducible evidence. The next trust program therefore turns three existing technical strengths into verifiable artifacts:

1. publish reproducible mutation-testing evidence for consensus/runtime-critical rules;
2. formally prove the 1:1 execution-reserve conservation invariant over the selected model;
3. expose the RandomX CPU-mining implementation to a public, adversarial testnet/challenge once node and testnet readiness gates are met.

The common rule is: **no marketing claim outruns the exact artifact and CI evidence that proves it.**

## 2. Workstream A — formal 1:1 reserve-conservation proof

### Existing foundation

Stage 3B already specifies and implements checked reserve arithmetic and the fail-closed equality:

`new_reserve = previous_reserve + native_deposit_total - execution_withdrawal_total - execution_fee_total`

and

`new_reserve == new_execution_balance_total`.

The implementation also rejects overflow, underflow, invalid previous reserve state, multiple live reserve outputs and reserve/execution-balance mismatch.

### Next work

Create a separately reviewed formal-verification slice, using a Rust-compatible model checker/prover such as Kani unless a later design selects a better pinned tool. The tool/version and proof boundary must be explicitly fixed before implementation.

Minimum proof obligations:

- accepted reserve arithmetic cannot wrap or saturate;
- any accepted transition satisfies `new_reserve == new_execution_balance_total`;
- zero execution reserve corresponds to no live reserve UTXO in the verified model;
- a valid accepted state contains at most one live execution reserve output;
- reserve creation/removal preserves the exact canonical reserve locking-program/value relationship;
- rejected transitions do not partially publish reserve state;
- apply/undo round trips restore the modeled pre-state when their preconditions hold;
- deposits, withdrawals and execution-funded fees conserve native backing according to the Stage 3B equation.

If full `UtxoState` verification is impractical in the first proof slice, use a deliberately small, documented abstraction and clearly distinguish which properties are mathematically proven from which remain covered by executable tests/mutation gates. Do not describe a bounded model as a proof of unmodeled storage or runtime behavior.

### Acceptance evidence

This workstream is complete only when:

- proof harnesses are committed and reviewable;
- the verifier runs reproducibly in pinned CI;
- exact-head CI is green;
- expected failing/negative proof controls demonstrate that materially weakened invariants are rejected;
- a checkpoint states the exact proven scope and explicit non-proven scope;
- no public statement says “all possible Oregon states are formally proven” unless the proof boundary actually supports that statement.

## 3. Workstream B — reproducible mutation-proof publication

### Existing foundation

Oregon already has independent mutation gates for multiple critical surfaces, including execution addresses, execution envelopes, contract state, execution resources and fee settlement. These gates deliberately inject or simulate material rule mutations and require the test suite to kill them.

### Next work

Turn those internal CI gates into a reproducible public engineering artifact instead of a generic “we test heavily” claim.

Required output:

- a versioned mutation-evidence manifest;
- mutation ID and target rule for every published mutation;
- exact source/commit under test;
- expected broken behavior;
- the test/vector/gate that kills the mutation;
- deterministic command or CI job needed to reproduce it;
- machine-readable pass/fail summary;
- links/identifiers for exact-head CI evidence;
- a concise technical write-up explaining why mutation testing is stronger evidence than line coverage alone, without claiming formal proof.

The publication should use the existing mutation scripts wherever possible rather than creating a second competing mutation framework.

### Acceptance evidence

This workstream is complete only when a clean checkout can reproduce the published mutation result from documented commands/CI, every advertised mutation is killed, and the public report is generated from or cross-checked against machine-readable repository evidence.

## 4. Workstream C — RandomX public testnet and adversarial challenge

### Existing foundation

Oregon already contains the real RandomX integration, light/full engines, deterministic architecture vectors, full/light parity checks, and x86_64/ARM CI coverage.

### Start gate

Do not launch a public mining/reorg event merely because the hashing engine exists. This workstream starts only when the node/testnet path needed for outside participants is sufficiently integrated and separately verified. Mainnet funds or production users are never used as the experiment surface.

### Next work

Prepare a reproducible public CPU-mining/testnet package containing:

- deterministic testnet/genesis and network configuration;
- documented node and CPU-miner startup path;
- ordinary-laptop/desktop participation instructions where hardware permits;
- x86_64 and ARM verification instructions;
- telemetry/observability sufficient to measure participation, forks and reorg behavior without collecting unnecessary personal data;
- a clearly scoped adversarial “break the testnet/reorg assumptions” challenge;
- responsible disclosure instructions and explicit in-scope/out-of-scope rules;
- deterministic incident capture so a discovered consensus/reorg bug can be reproduced from logs/vectors;
- a post-event report containing observed hashrate distribution, architecture mix, reorg attempts, accepted findings and resulting fixes.

Any monetary bounty is a separate owner/funding decision. The repository may define the technical challenge before a monetary reward exists; it must not promise funds that have not been explicitly approved.

### Acceptance evidence

This workstream is complete only when an external participant can reproduce setup from the public instructions, mine/validate on the intended testnet, submit a scoped adversarial finding, and the project can independently reproduce or reject the finding from retained evidence.

## 5. Execution order after Stage 4A integration

When Stage 4A is integrated into `main`, resume with this trust program in the following order unless a later owner decision changes it:

1. **Formal reserve proof first.** It creates the strongest new assurance and forces the exact proof boundary to be stated.
2. **Mutation evidence publication second.** Existing gates are already strong; package them into reproducible, versioned public evidence and extend gaps discovered during review.
3. **RandomX public testnet/challenge third, gated by node/testnet readiness.** Prepare the technical challenge early, but do not launch until external participation can be supported safely and reproducibly.

Workstreams A and B may share documentation/CI plumbing but must keep their claims distinct: mutation testing demonstrates that tests detect selected injected faults; formal verification proves only the explicitly modeled properties.

## 6. Non-goals and safety boundaries

This roadmap does **not**:

- activate Stage 4A, Stage 4B, Stage 4C, EVM, WASM or the universal envelope;
- alter current consensus, transaction, block, storage, networking or mempool behavior;
- claim the Stage 3B reserve invariant is already formally verified;
- claim existing mutation gates constitute a mathematical proof;
- claim RandomX alone makes a network ASIC-proof or perfectly decentralized;
- authorize a public mainnet stress test;
- authorize monetary bug-bounty payments;
- replace the repository’s design → plan → test-first → exact-head CI → checkpoint → separate integration discipline.

## 7. Continuation rule

After Stage 4A reaches `main`, future work should open this roadmap before choosing the next implementation task. The first implementation target is Workstream A’s formal reserve-conservation proof design unless the owner explicitly reprioritizes the roadmap.
