#!/usr/bin/env python3
"""Publish exact-source evidence from Oregon's existing mutation authorities."""
from pathlib import Path
import re
import subprocess

from mutation_evidence import (
    AuthoritySpec,
    DIGEST_RE,
    EXPECTED_AUTHORITIES,
    EvidenceError,
    RESULT_SCHEMA,
    canonical_json_bytes,
    parse_authority_output,
    sha256_file,
)


GIT_SHA_RE = re.compile(r"^[0-9a-f]{40}$")
REPOSITORY = "zafersari82/Oregon"


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise EvidenceError(message)


def run_command(command, root: Path) -> subprocess.CompletedProcess[str]:
    """Run one evidence command with combined captured output."""
    try:
        return subprocess.run(
            list(command),
            cwd=root,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=3600,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise EvidenceError(f"command execution failed: {' '.join(command)}: {error}") from error


def _git_value(root: Path, expression: str) -> str:
    result = run_command(("git", "rev-parse", "--verify", expression), root)
    if result.returncode != 0:
        raise EvidenceError(f"cannot resolve Git identity {expression}: {result.stdout.strip()}")
    value = result.stdout.strip()
    if GIT_SHA_RE.fullmatch(value) is None:
        raise EvidenceError(f"invalid Git identity for {expression}: {value!r}")
    return value


def git_identity(root: Path) -> tuple[str, str]:
    """Return exact commit and tree SHA-1 identities for a checkout."""
    return _git_value(root, "HEAD"), _git_value(root, "HEAD^{tree}")


def git_is_clean(root: Path) -> bool:
    result = run_command(("git", "status", "--porcelain", "--untracked-files=all"), root)
    return result.returncode == 0 and not result.stdout.strip()


def ensure_output_outside_root(root: Path, output: Path) -> Path:
    """Reject evidence output inside the checkout so it cannot dirty source state."""
    resolved_root = root.resolve()
    resolved_output = output.resolve()
    if resolved_output == resolved_root or resolved_output.is_relative_to(resolved_root):
        raise EvidenceError("mutation evidence output must be outside the repository checkout")
    return resolved_output


def run_authority(root: Path, authority: AuthoritySpec, log_dir: Path) -> dict:
    """Run one bound authority and accept it only if source remains clean."""
    root = root.resolve()
    log_dir = ensure_output_outside_root(root, log_dir)
    if not git_is_clean(root):
        raise EvidenceError(f"authority {authority.id} requires a clean checkout before execution")

    runner_path = (root / authority.runner).resolve()
    if not runner_path.is_relative_to(root) or not runner_path.is_file():
        raise EvidenceError(f"authority {authority.id} runner is missing or outside checkout")
    observed_digest = sha256_file(runner_path)
    if observed_digest != authority.runner_sha256:
        raise EvidenceError(
            f"authority {authority.id} runner digest mismatch: "
            f"expected {authority.runner_sha256}, observed {observed_digest}"
        )

    log_dir.mkdir(parents=True, exist_ok=True)
    result = run_command(authority.command, root)
    log_path = log_dir / f"{authority.id.lower()}.log"
    log_path.write_text(result.stdout, encoding="utf-8")
    records = parse_authority_output(authority, result.stdout, result.returncode)

    if not git_is_clean(root):
        raise EvidenceError(f"authority {authority.id} did not restore a clean checkout")

    return {
        "id": authority.id,
        "runner": authority.runner,
        "runner_sha256": observed_digest,
        "command": list(authority.command),
        "raw_log": {
            "filename": log_path.name,
            "sha256": sha256_file(log_path),
        },
        "records": records,
    }


def _validate_hex(value, pattern: re.Pattern, label: str) -> None:
    _require(isinstance(value, str) and pattern.fullmatch(value) is not None, f"invalid {label}")


def validate_result_v1(result: dict) -> None:
    """Validate the security-critical Result V1 contract without extra dependencies."""
    _require(isinstance(result, dict), "result must be an object")
    expected_top = {
        "schema", "manifest_sha256", "repository", "source", "checkout",
        "toolchain", "ci", "authorities", "totals", "overall_status",
    }
    _require(set(result) == expected_top, "result has unexpected or missing top-level fields")
    _require(result["schema"] == RESULT_SCHEMA, "unexpected result schema")
    _validate_hex(result["manifest_sha256"], DIGEST_RE, "manifest SHA-256")
    _require(result["repository"] == REPOSITORY, "unexpected repository identity")

    source = result["source"]
    _require(isinstance(source, dict) and set(source) == {"commit", "tree"}, "invalid source identity")
    _validate_hex(source["commit"], GIT_SHA_RE, "commit SHA")
    _validate_hex(source["tree"], GIT_SHA_RE, "tree SHA")

    checkout = result["checkout"]
    _require(
        checkout == {"before_clean": True, "after_clean": True},
        "result requires clean checkout before and after",
    )
    _require(result["toolchain"] == {"rust": "1.85.0"}, "unexpected toolchain contract")
    _require(isinstance(result["ci"], dict), "ci identity must be an object")
    _require(result["overall_status"] == "passed", "overall result is not passed")
    _require(result["totals"] == {"killed": 68, "total": 68}, "aggregate result must be 68/68")

    authorities = result["authorities"]
    _require(isinstance(authorities, list) and len(authorities) == 6, "result must contain six authorities")
    all_ids = []
    for raw, (prefix, count) in zip(authorities, EXPECTED_AUTHORITIES, strict=True):
        _require(isinstance(raw, dict), f"authority {prefix} result must be an object")
        expected_fields = {"id", "runner", "runner_sha256", "command", "raw_log", "mutations", "totals"}
        _require(set(raw) == expected_fields, f"authority {prefix} result fields changed")
        _require(raw["id"] == prefix, f"authority result order/id mismatch for {prefix}")
        _require(isinstance(raw["runner"], str) and raw["runner"], f"authority {prefix} missing runner")
        _validate_hex(raw["runner_sha256"], DIGEST_RE, f"authority {prefix} runner SHA-256")
        _require(
            isinstance(raw["command"], list) and raw["command"] == ["python3", raw["runner"]],
            f"authority {prefix} command is not bound to runner",
        )
        raw_log = raw["raw_log"]
        _require(
            isinstance(raw_log, dict) and set(raw_log) == {"filename", "sha256"},
            f"authority {prefix} invalid raw log identity",
        )
        _require(isinstance(raw_log["filename"], str) and raw_log["filename"], f"authority {prefix} log filename missing")
        _validate_hex(raw_log["sha256"], DIGEST_RE, f"authority {prefix} log SHA-256")
        _require(raw["totals"] == {"killed": count, "total": count}, f"authority {prefix} totals mismatch")

        mutations = raw["mutations"]
        _require(isinstance(mutations, list) and len(mutations) == count, f"authority {prefix} mutation count mismatch")
        expected_ids = [f"{prefix}-{index:03d}" for index in range(1, count + 1)]
        actual_ids = []
        for mutation in mutations:
            _require(isinstance(mutation, dict), f"authority {prefix} mutation result must be object")
            _require(
                set(mutation) == {"id", "name", "target", "killing_test", "status"},
                f"authority {prefix} mutation result fields changed",
            )
            _require(mutation["status"] == "killed", f"mutation {mutation.get('id')} is not killed")
            for key in ("id", "name", "target", "killing_test"):
                _require(isinstance(mutation[key], str) and mutation[key], f"authority {prefix}: invalid {key}")
            actual_ids.append(mutation["id"])
            all_ids.append(mutation["id"])
        _require(actual_ids == expected_ids, f"authority {prefix} stable ID inventory changed")

    _require(len(all_ids) == 68 and len(all_ids) == len(set(all_ids)), "result mutation inventory is not unique 68")


def _normalize_raw_log(raw_log) -> dict:
    _require(isinstance(raw_log, dict), "authority run raw_log must be normalized")
    _require(set(raw_log) == {"filename", "sha256"}, "authority run raw_log fields changed")
    _require(isinstance(raw_log["filename"], str) and raw_log["filename"], "raw log filename missing")
    _validate_hex(raw_log["sha256"], DIGEST_RE, "raw log SHA-256")
    return {"filename": raw_log["filename"], "sha256": raw_log["sha256"]}


def build_result(
    *,
    manifest_sha256: str,
    commit_sha: str,
    tree_sha: str,
    authorities,
    authority_runs,
    ci_identity=None,
) -> dict:
    """Build Result V1 only from a complete six-authority/68-kill evidence set."""
    _validate_hex(manifest_sha256, DIGEST_RE, "manifest SHA-256")
    _validate_hex(commit_sha, GIT_SHA_RE, "commit SHA")
    _validate_hex(tree_sha, GIT_SHA_RE, "tree SHA")
    authorities = tuple(authorities)
    authority_runs = tuple(authority_runs)
    _require(
        tuple((item.id, item.expected_count) for item in authorities) == EXPECTED_AUTHORITIES,
        "result requires the exact six-authority V1 manifest inventory",
    )
    _require(len(authority_runs) == 6, "result requires six completed authority runs")

    normalized = []
    total_killed = 0
    for spec, run in zip(authorities, authority_runs, strict=True):
        _require(isinstance(run, dict), f"authority {spec.id} run is not an object")
        _require(run.get("id") == spec.id, f"authority {spec.id} run identity mismatch")
        _require(run.get("runner") == spec.runner, f"authority {spec.id} runner mismatch")
        _require(run.get("runner_sha256") == spec.runner_sha256, f"authority {spec.id} runner digest mismatch")
        _require(run.get("command") == list(spec.command), f"authority {spec.id} command mismatch")
        raw_log = _normalize_raw_log(run.get("raw_log"))
        records = tuple(run.get("records", ()))
        _require(len(records) == spec.expected_count, f"authority {spec.id} incomplete kill records")
        by_id = {record.mutation_id: record for record in records}
        _require(len(by_id) == len(records), f"authority {spec.id} duplicate result records")

        mutation_results = []
        for mutation in spec.mutations:
            record = by_id.get(mutation.id)
            _require(record is not None, f"authority {spec.id} missing {mutation.id}")
            _require(record.name == mutation.name, f"authority {spec.id} semantic name mismatch for {mutation.id}")
            _require(record.killing_test == mutation.killing_test, f"authority {spec.id} killing test mismatch for {mutation.id}")
            _require(record.status == "killed", f"authority {spec.id} non-killed record {mutation.id}")
            mutation_results.append({
                "id": mutation.id,
                "name": mutation.name,
                "target": mutation.target,
                "killing_test": mutation.killing_test,
                "status": "killed",
            })

        count = spec.expected_count
        total_killed += count
        normalized.append({
            "id": spec.id,
            "runner": spec.runner,
            "runner_sha256": spec.runner_sha256,
            "command": list(spec.command),
            "raw_log": raw_log,
            "mutations": mutation_results,
            "totals": {"killed": count, "total": count},
        })

    _require(total_killed == 68, "result must contain exactly 68 killed mutations")
    result = {
        "schema": RESULT_SCHEMA,
        "manifest_sha256": manifest_sha256,
        "repository": REPOSITORY,
        "source": {"commit": commit_sha, "tree": tree_sha},
        "checkout": {"before_clean": True, "after_clean": True},
        "toolchain": {"rust": "1.85.0"},
        "ci": dict(ci_identity or {}),
        "authorities": normalized,
        "totals": {"killed": 68, "total": 68},
        "overall_status": "passed",
    }
    validate_result_v1(result)
    return result


def write_result_atomic(result: dict, output_dir: Path) -> Path:
    """Write canonical Result V1 only after complete validation."""
    validate_result_v1(result)
    output_dir.mkdir(parents=True, exist_ok=True)
    final_path = output_dir / "result-v1.json"
    temporary = output_dir / ".result-v1.json.tmp"
    temporary.write_bytes(canonical_json_bytes(result))
    temporary.replace(final_path)
    return final_path
