"""Fail-closed contracts for Oregon Mutation Evidence Publication V1."""
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import re


class EvidenceError(RuntimeError):
    """Raised when evidence cannot be accepted without weakening the contract."""


EXPECTED_AUTHORITIES = (
    ("EA", 3),
    ("EE", 9),
    ("CS", 17),
    ("ER", 13),
    ("FS", 14),
    ("RJ", 12),
)
MANIFEST_SCHEMA = "oregon.mutation-evidence.manifest/v1"
RESULT_SCHEMA = "oregon.mutation-evidence.result/v1"
DIGEST_RE = re.compile(r"^[0-9a-f]{64}$")


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


def canonical_json_bytes(value: object) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
        + "\n"
    ).encode("utf-8")


def sha256_file(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise EvidenceError(message)


def _string(mapping: dict, key: str, context: str) -> str:
    value = mapping.get(key)
    _require(isinstance(value, str) and bool(value.strip()), f"{context}: invalid {key}")
    return value


def _resolve_runner(root: Path, runner: str) -> Path:
    root = root.resolve()
    path = (root / runner).resolve()
    _require(path.is_relative_to(root), f"runner escapes repository root: {runner}")
    _require(path.is_file(), f"runner does not exist: {runner}")
    return path


def _parse_mutation(raw: dict, prefix: str, context: str) -> MutationSpec:
    _require(isinstance(raw, dict), f"{context}: mutation must be an object")
    mutation_id = _string(raw, "id", context)
    _require(
        re.fullmatch(rf"{re.escape(prefix)}-\d{{3}}", mutation_id) is not None,
        f"{context}: invalid mutation id {mutation_id}",
    )
    name = _string(raw, "name", context)
    target = _string(raw, "target", context)
    expected_broken_behavior = _string(raw, "expected_broken_behavior", context)
    killing_test = _string(raw, "killing_test", context)
    kill_classification = _string(raw, "kill_classification", context)
    _require(
        kill_classification == "semantic-test-failure",
        f"{context}: unsupported kill classification {kill_classification}",
    )
    allowed = {
        "id",
        "name",
        "target",
        "expected_broken_behavior",
        "killing_test",
        "kill_classification",
    }
    _require(set(raw) == allowed, f"{context}: unexpected or missing mutation fields")
    return MutationSpec(
        id=mutation_id,
        name=name,
        target=target,
        expected_broken_behavior=expected_broken_behavior,
        killing_test=killing_test,
        kill_classification=kill_classification,
    )


def _parse_authority(raw: dict, prefix: str, count: int, root: Path) -> AuthoritySpec:
    context = f"authority {prefix}"
    _require(isinstance(raw, dict), f"{context}: authority must be an object")
    authority_id = _string(raw, "id", context)
    _require(authority_id == prefix, f"{context}: wrong authority id {authority_id}")
    display_name = _string(raw, "display_name", context)
    runner = _string(raw, "runner", context)
    runner_sha256 = _string(raw, "runner_sha256", context)
    _require(DIGEST_RE.fullmatch(runner_sha256) is not None, f"{context}: malformed runner SHA-256")
    _resolve_runner(root, runner)

    command = raw.get("command")
    _require(
        isinstance(command, list) and all(isinstance(item, str) for item in command),
        f"{context}: command must be a string list",
    )
    _require(command == ["python3", runner], f"{context}: command must bind exact runner")

    expected_count = raw.get("expected_count")
    _require(expected_count == count, f"{context}: expected_count must be {count}")
    raw_mutations = raw.get("mutations")
    _require(isinstance(raw_mutations, list), f"{context}: mutations must be a list")
    _require(len(raw_mutations) == count, f"{context}: mutation cardinality must be {count}")

    mutations = tuple(
        _parse_mutation(mutation, prefix, f"{context} mutation {index}")
        for index, mutation in enumerate(raw_mutations, start=1)
    )
    expected_ids = tuple(f"{prefix}-{index:03d}" for index in range(1, count + 1))
    actual_ids = tuple(mutation.id for mutation in mutations)
    _require(actual_ids == expected_ids, f"{context}: stable mutation id inventory/order changed")
    names = [mutation.name for mutation in mutations]
    _require(len(names) == len(set(names)), f"{context}: duplicate semantic mutation name")

    allowed = {
        "id",
        "display_name",
        "runner",
        "runner_sha256",
        "command",
        "expected_count",
        "mutations",
    }
    _require(set(raw) == allowed, f"{context}: unexpected or missing authority fields")
    return AuthoritySpec(
        id=authority_id,
        display_name=display_name,
        runner=runner,
        runner_sha256=runner_sha256,
        command=tuple(command),
        expected_count=expected_count,
        mutations=mutations,
    )


def load_manifest(path: Path, root: Path) -> tuple[dict, tuple[AuthoritySpec, ...]]:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise EvidenceError(f"cannot read mutation evidence manifest: {error}") from error

    _require(isinstance(document, dict), "manifest must be a JSON object")
    _require(document.get("schema") == MANIFEST_SCHEMA, "unexpected manifest schema")
    _require(document.get("manifest_version") == 1, "unexpected manifest version")
    _string(document, "public_claim_boundary", "manifest")
    raw_authorities = document.get("authorities")
    _require(isinstance(raw_authorities, list), "manifest authorities must be a list")
    _require(len(raw_authorities) == len(EXPECTED_AUTHORITIES), "manifest must contain six authorities")

    authorities = tuple(
        _parse_authority(raw, prefix, count, root)
        for raw, (prefix, count) in zip(raw_authorities, EXPECTED_AUTHORITIES, strict=True)
    )
    all_ids = [mutation.id for authority in authorities for mutation in authority.mutations]
    _require(len(all_ids) == 68, "manifest must contain exactly 68 mutations")
    _require(len(all_ids) == len(set(all_ids)), "duplicate mutation id across authorities")

    allowed = {"schema", "manifest_version", "public_claim_boundary", "authorities"}
    _require(set(document) == allowed, "manifest has unexpected or missing top-level fields")
    return document, authorities
