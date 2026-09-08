# Oregon Mutation Evidence Publication V1 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish reproducible, machine-readable evidence that Oregon’s six existing production mutation authorities kill all 68 selected V1 mutations at an exact recorded source, without replacing or weakening those authorities.

**Architecture:** Keep the six existing `verify_*_mutations.py` scripts as the sole mutation authorities. Add a small stdlib-only Python verification core for manifest validation and output normalization, a separate publisher/orchestrator for Git/source integrity and process execution, a versioned manifest/schema contract, and a dedicated CI workflow that retains raw logs plus deterministic `result-v1.json` evidence.

**Tech Stack:** Python 3 standard library (`dataclasses`, `hashlib`, `json`, `pathlib`, `re`, `subprocess`, `tempfile`, `unittest`), Rust 1.85.0 through the existing mutation runners, Git, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-09-08-mutation-evidence-publication-v1.md`

## Global Constraints

- Work on isolated branch `work/mutation-evidence-publication-v1-2026-09-08`; never implement on `main`.
- Preserve all six existing mutation runners as the production mutation authorities; do not duplicate their executable replacement strings in a new framework.
- V1 inventory is exactly six authorities and 68 mutations: EA 3, EE 9, CS 17, ER 13, FS 14, RJ 12.
- Stable IDs are `EA-001..003`, `EE-001..009`, `CS-001..017`, `ER-001..013`, `FS-001..014`, `RJ-001..012`.
- Production Rust remains 1.85.0. Do not add a Python package dependency; use the standard library only.
- A compile error, timeout, missing test, unrelated failure, runner digest mismatch, dirty checkout, missing/duplicate/unknown kill record, wrong killing test, wrong summary count, or unresolved Git identity is never a successful mutation kill.
- A successful publication binds to exact commit SHA and tree SHA and emits `result-v1.json` only after all six authorities pass and all 68 records are matched exactly once.
- Raw logs may be retained on failure, but no failure path may leave a canonical passing `result-v1.json`.
- Do not modify transaction/block bytes, consensus, monetary/reserve semantics, storage, networking, mempool behavior, Stage 4 activation, or VM/runtime behavior.
- Reserve formal-model negative controls remain separate from the 68 production mutation claims.
- Final acceptance requires exact-head Mutation Evidence CI plus inherited Oregon Rust CI; a green ancestor is insufficient.
- `main` integration remains a separate explicit owner decision after exact-head acceptance.

---

### Task 1: Add the manifest contract and fail-closed validator

**Files:**
- Create: `verification/mutation-evidence/schema/manifest-v1.schema.json`
- Create: `verification/mutation-evidence/schema/result-v1.schema.json`
- Create: `scripts/mutation_evidence.py`
- Create: `scripts/verify_mutation_evidence_manifest.py`
- Create: `scripts/test_mutation_evidence_manifest.py`

**Interfaces:**
- Produces: `EvidenceError`, `MutationSpec`, `AuthoritySpec`, `load_manifest(path: Path, root: Path) -> tuple[dict, tuple[AuthoritySpec, ...]]`, `sha256_file(path: Path) -> str`, and `canonical_json_bytes(value: object) -> bytes` from `scripts/mutation_evidence.py`.
- Produces CLI: `python3 scripts/verify_mutation_evidence_manifest.py [--manifest PATH]` returning zero only for a fully valid six-authority/68-mutation V1 manifest.

- [ ] **Step 1: Write RED tests for schema/version/cardinality/identity rejection**

Create `scripts/test_mutation_evidence_manifest.py` using the repository’s `unittest` convention. The first tests import the not-yet-existing module and define a helper with a structurally valid in-memory manifest:

```python
from copy import deepcopy
from pathlib import Path
import tempfile
import unittest

from mutation_evidence import EvidenceError, load_manifest

ROOT = Path(__file__).resolve().parents[1]


def minimal_manifest():
    prefixes = [('EA', 3), ('EE', 9), ('CS', 17), ('ER', 13), ('FS', 14), ('RJ', 12)]
    authorities = []
    for prefix, count in prefixes:
        runner = f'scripts/{prefix.lower()}_runner.py'
        authorities.append({
            'id': prefix,
            'display_name': prefix,
            'runner': runner,
            'runner_sha256': '0' * 64,
            'command': ['python3', runner],
            'expected_count': count,
            'mutations': [
                {
                    'id': f'{prefix}-{index:03d}',
                    'name': f'{prefix} mutation {index}',
                    'target': f'target/{prefix.lower()}',
                    'expected_broken_behavior': f'{prefix} broken {index}',
                    'killing_test': f'{prefix.lower()}_test_{index}',
                    'kill_classification': 'semantic-test-failure',
                }
                for index in range(1, count + 1)
            ],
        })
    return {
        'schema': 'oregon.mutation-evidence.manifest/v1',
        'manifest_version': 1,
        'public_claim_boundary': 'selected semantic mutations only; not formal proof',
        'authorities': authorities,
    }
```

Add tests that mutate this document and expect `EvidenceError` for duplicate mutation ID, one missing mutation, one extra mutation, five authorities, wrong prefix, malformed digest, nonexistent runner, and command/runner mismatch. Create temporary runner files where a valid path is required.

- [ ] **Step 2: Run the tests and verify RED**

Run:

```bash
cd scripts
python3 -m unittest test_mutation_evidence_manifest.py -v
```

Expected: FAIL because `mutation_evidence` does not yet exist.

- [ ] **Step 3: Implement the minimal typed manifest model and validator**

Create `scripts/mutation_evidence.py` with these public types/signatures:

```python
from dataclasses import dataclass
from hashlib import sha256
import json
from pathlib import Path
import re


class EvidenceError(RuntimeError):
    pass


@dataclass(frozen=True)
class MutationSpec:
    id: str
    name: str
    target: str
    expected_broken_behavior: str
    killing_test: str
    kill_classification: str


@dataclass(frozen=True)
class AuthoritySpec:
    id: str
    display_name: str
    runner: str
    runner_sha256: str
    command: tuple[str, ...]
    expected_count: int
    mutations: tuple[MutationSpec, ...]


def canonical_json_bytes(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False) + '\n').encode()


def sha256_file(path: Path) -> str:
    with path.open('rb') as stream:
        return __import__('hashlib').file_digest(stream, 'sha256').hexdigest()
```

Implement `load_manifest` so it rejects every Global Constraint above that is knowable from the manifest alone. Hard-code the V1 authority order/count contract:

```python
EXPECTED_AUTHORITIES = (
    ('EA', 3), ('EE', 9), ('CS', 17),
    ('ER', 13), ('FS', 14), ('RJ', 12),
)
```

Require exactly 64 lowercase hex characters for every runner digest. Require command tuple exactly `('python3', authority.runner)`. Require IDs to match `rf'^{prefix}-\d{{3}}$'` and the complete exact expected ID set for each authority. Require every runner path to resolve beneath `root` and exist as a regular file.

- [ ] **Step 4: Add the two JSON Schema documents**

Use Draft 2020-12 metadata and `additionalProperties: false` for manifest, authority, mutation, result, source, totals and per-mutation result objects. The schema constants must match the code exactly:

```json
{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"oregon.mutation-evidence.manifest/v1"}
```

and

```json
{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"oregon.mutation-evidence.result/v1"}
```

The runtime does not import `jsonschema`; the schemas are reviewable public contracts and the stdlib validator enforces the security-critical subset.

- [ ] **Step 5: Add the manifest CLI and make the RED tests GREEN**

Create `scripts/verify_mutation_evidence_manifest.py`:

```python
#!/usr/bin/env python3
import argparse
from pathlib import Path
from mutation_evidence import EvidenceError, load_manifest

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / 'verification/mutation-evidence/manifest-v1.json'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--manifest', type=Path, default=DEFAULT)
    args = parser.parse_args()
    _, authorities = load_manifest(args.manifest, ROOT)
    print(f'Mutation evidence manifest valid: {len(authorities)} authorities, '
          f'{sum(a.expected_count for a in authorities)} mutations')


if __name__ == '__main__':
    main()
```

Run:

```bash
cd scripts
python3 -m unittest test_mutation_evidence_manifest.py -v
```

Expected: PASS.

- [ ] **Step 6: Commit Task 1**

```bash
git add scripts/mutation_evidence.py scripts/verify_mutation_evidence_manifest.py \
  scripts/test_mutation_evidence_manifest.py verification/mutation-evidence/schema
git commit -m "test: define mutation evidence manifest contract"
```

---

### Task 2: Normalize authority output and reject false kills

**Files:**
- Modify: `scripts/mutation_evidence.py`
- Create: `scripts/test_mutation_evidence_publisher.py`

**Interfaces:**
- Consumes: `AuthoritySpec`, `MutationSpec`, `EvidenceError`.
- Produces: `KillRecord` and `parse_authority_output(authority: AuthoritySpec, output: str, returncode: int) -> tuple[KillRecord, ...]`.

- [ ] **Step 1: Write RED parser tests for both existing output syntaxes**

In `scripts/test_mutation_evidence_publisher.py`, build one-authority fixtures and cover both real runner formats:

```python
KILLED: unknown-kind acceptance — unknown_kinds_are_rejected_instead_of_becoming_another_namespace
3/3 mutations killed; restored clean address suite passed
```

and:

```python
KILLED: conversion floors instead of ceiling [conversion_rounds_up]
Execution resource mutations: 13/13 killed
```

Also assert `EvidenceError` for duplicate kill line, unknown mutation name, correct name/wrong test, summary mismatch, nonzero runner exit, `could not compile`, `error[E0308]`, missing summary and missing one expected kill.

- [ ] **Step 2: Run RED parser tests**

```bash
cd scripts
python3 -m unittest test_mutation_evidence_publisher.py -v
```

Expected: FAIL because `KillRecord`/`parse_authority_output` do not exist.

- [ ] **Step 3: Implement strict parsing**

Add:

```python
@dataclass(frozen=True)
class KillRecord:
    mutation_id: str
    name: str
    killing_test: str
    status: str = 'killed'
```

`parse_authority_output` must:

1. reject any nonzero `returncode`;
2. reject compiler/infrastructure markers `could not compile`, `error[E`, `Traceback (most recent call last)`, `timed out`;
3. accept only anchored `KILLED:` records in the em-dash or bracket form;
4. map `(name, killing_test)` one-to-one to manifest mutations;
5. reject duplicates/unknowns/wrong test names;
6. require exactly `authority.expected_count` records;
7. accept the authority’s final summary only when its numerator and denominator both equal `expected_count`.

Use explicit regular expressions; do not accept a bare aggregate count as sufficient evidence.

- [ ] **Step 4: Run parser tests GREEN**

```bash
cd scripts
python3 -m unittest test_mutation_evidence_publisher.py -v
```

Expected: PASS.

- [ ] **Step 5: Commit Task 2**

```bash
git add scripts/mutation_evidence.py scripts/test_mutation_evidence_publisher.py
git commit -m "feat: parse mutation authority evidence fail closed"
```

---

### Task 3: Add Git/source integrity and authority execution orchestration

**Files:**
- Create: `scripts/publish_mutation_evidence.py`
- Modify: `scripts/test_mutation_evidence_publisher.py`

**Interfaces:**
- Produces: `git_identity(root: Path) -> tuple[str, str]`, `git_is_clean(root: Path) -> bool`, `run_command(command: tuple[str, ...], root: Path) -> subprocess.CompletedProcess[str]`, and `run_authority(root: Path, authority: AuthoritySpec, log_dir: Path) -> dict`.

- [ ] **Step 1: Write RED tests for dirty/unresolved/drifted execution state**

Use temporary Git repositories and `unittest.mock.patch` to prove rejection of:

- dirty checkout before execution;
- `git rev-parse HEAD` failure;
- `git rev-parse HEAD^{tree}` failure;
- stale runner SHA-256;
- authority subprocess nonzero;
- clean-before but dirty-after execution;
- source-restoration failure represented by dirty checkout after the runner;
- output directory located inside the repository worktree.

- [ ] **Step 2: Run tests RED**

```bash
cd scripts
python3 -m unittest test_mutation_evidence_publisher.py -v
```

Expected: FAIL on missing publisher functions.

- [ ] **Step 3: Implement exact Git identity and clean-checkout checks**

`git_identity` must run:

```bash
git rev-parse --verify HEAD
git rev-parse --verify 'HEAD^{tree}'
```

and require lowercase 40-hex output for both. `git_is_clean` must run:

```bash
git status --porcelain --untracked-files=all
```

and return true only for empty stdout and zero exit.

`run_authority` must verify `sha256_file(root / authority.runner) == authority.runner_sha256` before subprocess execution, write the raw combined output to `<log_dir>/<authority.id.lower()>.log`, call `parse_authority_output`, then re-check `git_is_clean(root)` before returning normalized records.

- [ ] **Step 4: Make execution tests GREEN**

```bash
cd scripts
python3 -m unittest test_mutation_evidence_publisher.py -v
```

Expected: PASS.

- [ ] **Step 5: Commit Task 3**

```bash
git add scripts/publish_mutation_evidence.py scripts/test_mutation_evidence_publisher.py
git commit -m "feat: bind mutation evidence to clean git source"
```

---

### Task 4: Generate deterministic Result V1 only after complete success

**Files:**
- Modify: `scripts/publish_mutation_evidence.py`
- Modify: `scripts/mutation_evidence.py`
- Modify: `scripts/test_mutation_evidence_publisher.py`

**Interfaces:**
- Produces: `build_result(...) -> dict`, `validate_result_v1(result: dict) -> None`, and `write_result_atomic(result: dict, output_dir: Path) -> Path`.

- [ ] **Step 1: Write RED tests for partial publication and deterministic JSON**

Add tests proving:

- five successful authorities never produce `result-v1.json`;
- a sixth failed authority leaves no canonical result;
- a complete six-authority/68-record run yields aggregate `{ "killed": 68, "total": 68 }`;
- every per-mutation status is exactly `killed`;
- sorting is deterministic across two equivalent input dictionaries;
- stale result schema/version is rejected;
- missing commit/tree/manifest digest is rejected.

- [ ] **Step 2: Run RED**

```bash
cd scripts
python3 -m unittest test_mutation_evidence_publisher.py -v
```

Expected: FAIL on missing result functions.

- [ ] **Step 3: Implement Result V1**

Use this top-level shape:

```python
{
    'schema': 'oregon.mutation-evidence.result/v1',
    'manifest_sha256': manifest_sha256,
    'repository': 'zafersari82/Oregon',
    'source': {'commit': commit_sha, 'tree': tree_sha},
    'checkout': {'before_clean': True, 'after_clean': True},
    'toolchain': {'rust': '1.85.0'},
    'ci': ci_identity,
    'authorities': authority_results,
    'totals': {'killed': 68, 'total': 68},
    'overall_status': 'passed',
}
```

Each authority result includes verified runner digest, command, raw log filename/digest, per-mutation stable ID/name/target/killing test/status and authority totals. `write_result_atomic` writes to a temporary sibling file and `Path.replace()` only after `validate_result_v1` passes.

- [ ] **Step 4: Run GREEN and all mutation-evidence unit tests**

```bash
cd scripts
python3 -m unittest test_mutation_evidence_manifest.py test_mutation_evidence_publisher.py -v
```

Expected: PASS.

- [ ] **Step 5: Commit Task 4**

```bash
git add scripts/mutation_evidence.py scripts/publish_mutation_evidence.py \
  scripts/test_mutation_evidence_publisher.py
git commit -m "feat: emit deterministic mutation evidence result"
```

---

### Task 5: Populate the reviewed six-authority/68-mutation manifest

**Files:**
- Create: `verification/mutation-evidence/manifest-v1.json`
- Modify: `scripts/test_mutation_evidence_manifest.py`

**Interfaces:**
- Consumes the exact six current authority scripts.
- Produces the immutable V1 public inventory and pinned runner SHA-256 values.

- [ ] **Step 1: Compute exact runner SHA-256 values from the implementation branch**

Run:

```bash
sha256sum \
  scripts/verify_execution_address_mutations.py \
  scripts/verify_execution_envelope_mutations.py \
  scripts/verify_contract_state_mutations.py \
  scripts/verify_execution_resource_mutations.py \
  scripts/verify_fee_settlement_mutations.py \
  scripts/verify_journal_mutations.py
```

Record those six exact 64-hex values in `manifest-v1.json`; do not use Git blob SHA-1 values.

- [ ] **Step 2: Populate all stable IDs in current runner order**

Use these exact ranges and authority bindings:

```text
EA-001..EA-003 -> scripts/verify_execution_address_mutations.py
EE-001..EE-009 -> scripts/verify_execution_envelope_mutations.py
CS-001..CS-017 -> scripts/verify_contract_state_mutations.py
ER-001..ER-013 -> scripts/verify_execution_resource_mutations.py
FS-001..FS-014 -> scripts/verify_fee_settlement_mutations.py
RJ-001..RJ-012 -> scripts/verify_journal_mutations.py
```

For each mutation copy the current semantic mutation name and intended killing-test identifier exactly from the authoritative runner. `target` names the production file/rule area, and `expected_broken_behavior` describes the weakened behavior in one sentence. Do not copy replacement-source strings into the manifest.

Every authority command is exactly:

```json
["python3", "scripts/verify_execution_address_mutations.py"]
```

with the corresponding runner path substituted for the other five authorities.

- [ ] **Step 3: Add a repository-manifest acceptance test**

In `scripts/test_mutation_evidence_manifest.py` add:

```python
def test_repository_manifest_is_exact_v1_inventory(self):
    path = ROOT / 'verification/mutation-evidence/manifest-v1.json'
    _, authorities = load_manifest(path, ROOT)
    self.assertEqual([a.id for a in authorities], ['EA', 'EE', 'CS', 'ER', 'FS', 'RJ'])
    self.assertEqual([a.expected_count for a in authorities], [3, 9, 17, 13, 14, 12])
    self.assertEqual(sum(a.expected_count for a in authorities), 68)
    for authority in authorities:
        self.assertEqual(sha256_file(ROOT / authority.runner), authority.runner_sha256)
```

- [ ] **Step 4: Run validator and tests**

```bash
python3 scripts/verify_mutation_evidence_manifest.py
python3 -m unittest scripts/test_mutation_evidence_manifest.py scripts/test_mutation_evidence_publisher.py -v
```

Expected: manifest reports `6 authorities, 68 mutations`; all tests PASS.

- [ ] **Step 5: Commit Task 5**

```bash
git add verification/mutation-evidence/manifest-v1.json scripts/test_mutation_evidence_manifest.py
git commit -m "feat: publish mutation evidence v1 inventory"
```

---

### Task 6: Complete the publisher CLI and public reproduction README

**Files:**
- Modify: `scripts/publish_mutation_evidence.py`
- Create: `verification/mutation-evidence/README.md`
- Modify: `scripts/test_mutation_evidence_publisher.py`

**Interfaces:**
- CLI: `python3 scripts/publish_mutation_evidence.py --output PATH [--manifest PATH]`.
- Successful output directory contains six raw logs plus `result-v1.json`; failed execution contains diagnostics/raw logs but no passing canonical result.

- [ ] **Step 1: Add RED CLI tests**

Patch `run_authority` to simulate five passes plus one failure and assert `main()` exits nonzero and `result-v1.json` is absent. Patch six passes and assert a canonical result is written. Verify output path inside the checkout is rejected before any authority runs.

- [ ] **Step 2: Run RED**

```bash
cd scripts
python3 -m unittest test_mutation_evidence_publisher.py -v
```

Expected: FAIL until CLI orchestration is complete.

- [ ] **Step 3: Implement full CLI orchestration**

Required flow:

```text
resolve root/manifest/output
reject output under repository root
load+validate manifest
require clean checkout
resolve commit/tree
verify all runner digests before first mutation process
run authorities in manifest order
require clean checkout after each authority
build/validate result only after all six succeed
write result-v1.json atomically
print "Mutation Evidence V1: 68/68 killed"
```

Populate CI identity from standard environment variables only when present: `GITHUB_WORKFLOW`, `GITHUB_RUN_ID`, `GITHUB_RUN_ATTEMPT`, `GITHUB_JOB`, `GITHUB_SHA`.

- [ ] **Step 4: Write the public README**

Document exactly these reproduction commands:

```bash
python3 scripts/verify_mutation_evidence_manifest.py
python3 scripts/publish_mutation_evidence.py --output /tmp/oregon-mutation-evidence
python3 -m unittest scripts/test_mutation_evidence_manifest.py scripts/test_mutation_evidence_publisher.py -v
```

State explicitly that mutation testing shows selected injected faults are detected; it is stronger evidence than line coverage for those selected faults but is not formal proof, exhaustive fault coverage or proof of absence of bugs.

- [ ] **Step 5: Run unit suite GREEN**

```bash
python3 -m unittest scripts/test_mutation_evidence_manifest.py scripts/test_mutation_evidence_publisher.py -v
```

Expected: PASS.

- [ ] **Step 6: Commit Task 6**

```bash
git add scripts/publish_mutation_evidence.py scripts/test_mutation_evidence_publisher.py \
  verification/mutation-evidence/README.md
git commit -m "feat: add mutation evidence publication command"
```

---

### Task 7: Add dedicated exact-head CI evidence retention

**Files:**
- Create: `.github/workflows/oregon-mutation-evidence.yml`

**Interfaces:**
- Workflow name: `Oregon Mutation Evidence`.
- Push branch: `work/mutation-evidence-publication-v1-2026-09-08`.
- PR base: `main`.
- Artifact name: `mutation-evidence-${{ github.event.pull_request.head.sha || github.sha }}`.

- [ ] **Step 1: Create the dedicated workflow**

Use pinned actions already accepted in the repository:

```yaml
name: Oregon Mutation Evidence

on:
  push:
    branches: [work/mutation-evidence-publication-v1-2026-09-08]
  pull_request:
    branches: [main]

permissions:
  contents: read

jobs:
  evidence:
    runs-on: ubuntu-24.04
    timeout-minutes: 120
    env:
      PYTHONDONTWRITEBYTECODE: '1'
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
        with:
          ref: ${{ github.event.pull_request.head.sha || github.sha }}
          persist-credentials: false
      - name: Install Rust 1.85.0
        run: rustup toolchain install 1.85.0 --profile minimal
      - name: Install native prerequisites
        run: sudo apt-get update && sudo apt-get install -y clang libclang-dev
      - name: Test mutation evidence fail-closed logic
        run: python3 -m unittest scripts/test_mutation_evidence_manifest.py scripts/test_mutation_evidence_publisher.py -v
      - name: Validate published manifest
        run: python3 scripts/verify_mutation_evidence_manifest.py
      - name: Reproduce all published mutations
        run: python3 scripts/publish_mutation_evidence.py --output "$RUNNER_TEMP/mutation-evidence"
      - name: Show publication result
        if: always()
        run: |
          if [ -f "$RUNNER_TEMP/mutation-evidence/result-v1.json" ]; then
            cat "$RUNNER_TEMP/mutation-evidence/result-v1.json"
          fi
      - name: Retain mutation evidence
        if: always()
        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02
        with:
          name: mutation-evidence-${{ github.event.pull_request.head.sha || github.sha }}
          path: ${{ runner.temp }}/mutation-evidence/
          if-no-files-found: error
```

Do not add a path filter; relevant runner/source/test changes must not bypass this workflow.

- [ ] **Step 2: Run local syntax/contract checks available without GitHub Actions**

```bash
python3 -m unittest scripts/test_mutation_evidence_manifest.py scripts/test_mutation_evidence_publisher.py -v
python3 scripts/verify_mutation_evidence_manifest.py
```

Expected: PASS.

- [ ] **Step 3: Commit Task 7**

```bash
git add .github/workflows/oregon-mutation-evidence.yml
git commit -m "ci: retain reproducible mutation evidence"
```

---

### Task 8: Execute the real 68/68 authority run and verify exact-head CI

**Files:**
- No source changes expected unless a real failure reveals an implementation defect.
- Evidence is generated outside the repository checkout and retained by CI.

**Interfaces:**
- Full local/CI reproduction command: `python3 scripts/publish_mutation_evidence.py --output <outside-repo-path>`.

- [ ] **Step 1: Run all six real authorities from a clean checkout**

```bash
git status --porcelain --untracked-files=all
rm -rf /tmp/oregon-mutation-evidence
python3 scripts/publish_mutation_evidence.py --output /tmp/oregon-mutation-evidence
python3 - <<'PY'
import json
from pathlib import Path
result = json.loads(Path('/tmp/oregon-mutation-evidence/result-v1.json').read_text())
assert result['overall_status'] == 'passed'
assert result['totals'] == {'killed': 68, 'total': 68}
assert len(result['authorities']) == 6
assert sum(len(a['mutations']) for a in result['authorities']) == 68
print(result['source'])
PY
git status --porcelain --untracked-files=all
```

Expected: publisher prints `Mutation Evidence V1: 68/68 killed`; both Git status checks are empty.

- [ ] **Step 2: Open a draft PR to `main` from the implementation branch**

PR body must state: no production semantic changes; six existing authorities remain authoritative; V1 publishes 68 selected mutations; mutation evidence is not formal proof; exact-head Mutation Evidence + Oregon Rust CI required before acceptance; no main-integration authorization is implied.

- [ ] **Step 3: Verify exact-head workflows for the current PR head**

Require at minimum:

- `Oregon Mutation Evidence` — SUCCESS at the exact PR head;
- `Oregon Rust CI` — SUCCESS at the exact PR head;
- any other workflow triggered by files actually changed in this slice — SUCCESS or documented non-applicability.

For the mutation-evidence run, retrieve the artifact and verify it contains all six raw logs and `result-v1.json`. Confirm the result’s `source.commit` equals the exact PR-head SHA and its totals are 68/68.

- [ ] **Step 4: If CI exposes a real defect, return to the owning task**

Do not weaken parser or kill criteria to make CI green. Fix the root cause, add a regression test, commit, and repeat exact-head verification. A rerun of an unchanged flaky infrastructure job is allowed only when logs show infrastructure failure rather than semantic/test failure.

---

### Task 9: Record acceptance checkpoint and continuation boundary

**Files:**
- Create: `docs/checkpoints/OREGON_MUTATION_EVIDENCE_V1.md`
- Modify: `HANDOFF.md`
- Modify: `docs/superpowers/plans/2026-09-08-mutation-evidence-publication-v1.md`

**Interfaces:**
- Checkpoint becomes the authoritative acceptance record for the exact final candidate source.

- [ ] **Step 1: Record exact accepted evidence**

`OREGON_MUTATION_EVIDENCE_V1.md` must record:

- exact PR-head commit and tree;
- manifest SHA-256;
- all six runner SHA-256 values;
- authority cardinalities 3/9/17/13/14/12 and aggregate 68/68;
- Mutation Evidence workflow run/job/artifact identifiers;
- Oregon Rust CI run/job identifiers;
- result artifact digest if GitHub exposes it;
- source-clean/restoration status;
- reproduction commands;
- exact bounded public claim and explicit non-claims.

- [ ] **Step 2: Update `HANDOFF.md`**

Point continuation to the exact final candidate, PR, checkpoint, CI evidence and the next incomplete action. If all acceptance gates are green, the next action is the separate owner decision on integrating the accepted PR into `main`; do not ask for any earlier design approval again.

- [ ] **Step 3: Mark completed plan tasks accurately**

Change only genuinely completed checkboxes to `[x]`. Do not mark exact-head CI or checkpoint items complete based on an ancestor.

- [ ] **Step 4: Commit closure documentation**

```bash
git add docs/checkpoints/OREGON_MUTATION_EVIDENCE_V1.md HANDOFF.md \
  docs/superpowers/plans/2026-09-08-mutation-evidence-publication-v1.md
git commit -m "docs: checkpoint mutation evidence v1"
```

- [ ] **Step 5: Verify the final documentation head again**

Because the checkpoint/HANDOFF commit moves the PR head, require fresh exact-head `Oregon Mutation Evidence` and `Oregon Rust CI` success for that final source before requesting `main` integration.

---

### Task 10: Stop at the `main` integration boundary

**Files:** none until an explicit owner integration decision is received.

- [ ] **Step 1: Present the exact final candidate**

Report final PR number, head SHA/tree, Mutation Evidence run/job/artifact, Oregon Rust CI run/job, 68/68 result, manifest digest, runner digests and checkpoint path.

- [ ] **Step 2: Request the separate explicit `main` integration decision**

Do not merge, enable auto-merge, rebase, squash or move `main` without that decision. If approved, merge using an expected-head guard and then verify the actual `main` merge source with all workflows that trigger on main.
