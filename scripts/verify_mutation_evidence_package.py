#!/usr/bin/env python3
"""Recheck retained Mutation Evidence V1 files against the checked-out source."""
import argparse
import json
from pathlib import Path
import sys

from mutation_evidence import (
    EvidenceError, EXPECTED_AUTHORITIES, canonical_json_bytes, load_manifest,
    parse_authority_output, sha256_file,
)
from publish_mutation_evidence import ROOT, build_result, git_identity, validate_result_v1


def verify_package(output: Path, root: Path) -> dict:
    """Bind all eight regular files to source, manifest and semantic kill records.

    This verifies retained evidence integrity. It does not execute mutations or
    authenticate an arbitrary external author's claimed process exit status.
    CI's successful publisher step remains the execution authority.
    """
    expected = {"manifest-v1.json", "result-v1.json"} | {
        prefix.lower() + ".log" for prefix, _ in EXPECTED_AUTHORITIES
    }
    try:
        entries = list(output.iterdir())
        if {p.name for p in entries} != expected:
            raise EvidenceError("package must contain exactly the eight V1 evidence files")
        if any(p.is_symlink() or not p.is_file() for p in entries):
            raise EvidenceError("package entries must be regular files, not symlinks")
        manifest = output / "manifest-v1.json"
        if manifest.read_bytes() != (root / "verification/mutation-evidence/manifest-v1.json").read_bytes():
            raise EvidenceError("package manifest does not match checked-out source")
        _, specs = load_manifest(manifest, root)
        raw_result = (output / "result-v1.json").read_bytes()
        result = json.loads(raw_result)
        validate_result_v1(result)
        commit, tree = git_identity(root)
        if result["source"] != {"commit": commit, "tree": tree}:
            raise EvidenceError("package source does not match checked-out commit/tree")
        runs = []
        for spec in specs:
            digest = sha256_file(root / spec.runner)
            if digest != spec.runner_sha256:
                raise EvidenceError(f"authority {spec.id} runner digest mismatch")
            log = output / (spec.id.lower() + ".log")
            records = parse_authority_output(spec, log.read_text(encoding="utf-8"), 0)
            runs.append({
                "id": spec.id, "runner": spec.runner, "runner_sha256": digest,
                "command": list(spec.command), "records": records,
                "raw_log": {"filename": log.name, "sha256": sha256_file(log)},
            })
        rebuilt = build_result(
            manifest_sha256=sha256_file(manifest), commit_sha=commit, tree_sha=tree,
            authorities=specs, authority_runs=runs, ci_identity=result["ci"],
        )
        if raw_result != canonical_json_bytes(rebuilt):
            raise EvidenceError("retained result differs from reconstructed manifest/log evidence")
        return rebuilt
    except (OSError, UnicodeError, ValueError) as error:
        raise EvidenceError(f"cannot verify retained package: {error}") from error


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        result = verify_package(args.output, ROOT)
    except EvidenceError as error:
        print(f"Mutation evidence package failed: {error}", file=sys.stderr)
        return 1
    print(f"Verified eight evidence files, six log digests, 68 semantic records; source {result['source']['commit']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
