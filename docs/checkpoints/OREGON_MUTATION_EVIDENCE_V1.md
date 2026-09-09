# Oregon Mutation Evidence V1 — acceptance checkpoint

Recorded: 2026-09-09 UTC. Verified implementation; main integration remains a
separate decision. This is Workstream B of the trust roadmap.

## Exact implementation evidence

- Repository/PR: https://github.com/zafersari82/Oregon/pull/25
- Main base: `5785089e0cc2de18b1b50ef112afbe2f8265a8d8`.
- Verified implementation SHA: `1de457fa985e0af1c8149a8e8c51ead4e80679a7`.
- Verified tree: `8f95631380407804bec28bcbf866fb8749989171`.
- Design: `docs/superpowers/specs/2026-09-08-mutation-evidence-publication-v1.md`.
- Plan: `docs/superpowers/plans/2026-09-08-mutation-evidence-publication-v1.md`.
- Manifest SHA-256: `8655998518f0c0eb2640fed564a004adcfc68906e14a318324d00e318059beee`.

| Workflow | Run | Job | Result |
| --- | --- | --- | --- |
| Mutation Evidence | 34273405259 | 102220420829 | SUCCESS |
| Rust CI | 34273405269 | 102220421102 | SUCCESS |
| Reserve Conservation Proofs | 34273405267 | 102220421133 | SUCCESS |
| Reserve Verifier Bootstrap | 34273405305 | 102220420999 | SUCCESS |

All results above refer to the exact implementation SHA. Rust CI retains the
workspace/all-target tests, architecture checks, six inherited mutation gates,
docs/rustdoc, formatting and warnings-denied Clippy. Reserve model proofs remain
a distinct assurance claim.

## Retained package

- Artifact ID: `10074897462`.
- Artifact: https://github.com/zafersari82/Oregon/actions/runs/34273405259/artifacts/10074897462
- GitHub ZIP SHA-256: `d1c60868da0de730c256a7a41d0e3b9bbc63a52e8953b4a2b41c1c958e136168`.
- Files: `manifest-v1.json`, `result-v1.json`, and `ea.log`, `ee.log`,
  `cs.log`, `er.log`, `fs.log`, `rj.log`.
- Canonical result: six authorities, 68/68 killed, source clean before/after.
- The CI upload reports exactly eight files.

After actual mutation execution, a separate CI step rereads the retained files.
It checks the manifest against checkout, all runner identities, all six raw-log
SHA-256 values, all semantic kill records and the exact source commit/tree, then
reconstructs and byte-compares the canonical result. Its successful job log
explicitly records eight files, six log digests and 68 semantic records.

This package verification occurred in CI before upload. The connector's artifact
download reference returned HTTP 403 locally; no local independent ZIP-byte
inspection is claimed. The ZIP digest above is GitHub's artifact digest, not a
digest computed by this local session.

## Authority inventory

| Authority | Killed | Runner SHA-256 |
| --- | --- | --- |
| EA | 3/3 | `2f0fcdf7ea01cde07e339753fc214b08903a1b028887ad4afbd25f2623f8a3e4` |
| EE | 9/9 | `7a33b49275ce33631ca8f2c66e72149d80b8a21530c0fad5e5fc8d7419d073e8` |
| CS | 17/17 | `3750ff917765530ec84c31b21dbcd8e6994ba1cb021f6481a4dffe8c3c312cbc` |
| ER | 13/13 | `417b0c7864687770dd2585995bc4d72a05ec175f13b04ca56d941af34cfe2675` |
| FS | 14/14 | `84ffcf67468343e1192c4ab13377d954f2fb7ec96c9910b3ba5d1e78571faba4` |
| RJ | 12/12 | `208ae54352012f6d562e7eb08bbda5070a5645213f5c769c858d0194321b0d93` |

Stable IDs, target files/rules, intended faulty behavior and killing tests are in
the bound manifest. The six existing runners retain sole authority over applying
and killing production mutations; neither publisher nor package verifier
reimplements mutation edits.

## Regression and integrity evidence

The exact-source CI passed 50 Python tests. Manifest/parser/publisher tests cover
malformed inventories, wrong tests, infrastructure errors, source restoration,
partial publication and deterministic output. The retained-package tests add
missing/extra files, symlinks, corrupt logs, aggregate-only forged logs with updated
digests, wrong source and mismatched semantic results. Seven negative package
cases demonstrated the insufficiency of shape-only validation before the package
checker was implemented; all eight package tests pass with the checker.

The package checker verifies evidence consistency, not the authenticity of an
arbitrary external author's process claims. Successful, exact-head execution of
the pinned authority runners in CI remains essential.

## Reproduction

Use a clean checkout of the recorded source, recursive RandomX submodules and
Rust 1.85.0 with the native prerequisites from the dedicated workflow:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 scripts/verify_mutation_evidence_manifest.py
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts -p 'test_mutation_evidence*.py'
PYTHONDONTWRITEBYTECODE=1 python3 scripts/publish_mutation_evidence.py --output /tmp/oregon-evidence
PYTHONDONTWRITEBYTECODE=1 python3 scripts/verify_mutation_evidence_package.py --output /tmp/oregon-evidence
```

The output directory must be outside checkout. For retained evidence, first check
out its recorded source, then use the package-verification command.

## Accepted bounded claim

Oregon publishes reproducible evidence that the named tests/gates detect all 68
selected V1 fault injections across six production mutation authorities at the
recorded implementation source.

This is not formal proof, exhaustive fault coverage, proof of absence of bugs,
production VM readiness or activation. It does not include reserve-model negative
controls in the 68 production mutations and does not change production monetary,
reserve, execution, storage, network or transaction semantics.

## Documentation successor and integration

This checkpoint's successor changes documentation only. It must receive its own
exact-head CI before integration. Record that successor's actual run/job/artifact
identities in PR #25 rather than creating an endless sequence of documentation
commits solely to embed their own CI IDs. The verified implementation evidence
above never substitutes for the final PR-head checks.

A separate explicit main-integration decision remains required by AGENTS.md and
plan Task 10. No merge or protocol activation is established by this checkpoint.
See `OREGON_PLATFORM_COMPLETION_STATUS.md` for the overall platform status;
68/68 is a mutation result, not a platform completion percentage.
