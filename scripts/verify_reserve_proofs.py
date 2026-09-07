#!/usr/bin/env python3
"""Run the exact manifest-listed Oregon reserve proofs and validate Kani results."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
from typing import Callable, Iterable, Mapping, Sequence


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = (
    ROOT / "verification" / "reserve-conservation" / "proof-manifest.json"
)
SUPPORTED_SCHEMA_VERSION = 1
PROPERTY_STATUSES = frozenset(
    {"SUCCESS", "FAILURE", "SATISFIED", "UNSATISFIED", "UNDETERMINED"}
)
INFRASTRUCTURE_FAILURE_MARKERS = (
    "could not compile",
    "compilation failed",
    "failed to compile",
    "internal compiler error",
    "unsupported operation",
    "unsupported construct",
    "not currently supported",
    "verification:- inconclusive",
    "verification:- error",
    "compiler crashed",
)
SAFETY_PROPERTY_MARKERS = (
    "unwind",
    "pointer dereference",
    "pointer_dereference",
    "memory safety",
    "memory_safety",
    "bounds check",
    "bounds_check",
)


class VerificationError(RuntimeError):
    """A fail-closed proof configuration, tool, execution, or result error."""


@dataclass(frozen=True)
class ProcessResult:
    exit_code: int
    output: str


@dataclass(frozen=True)
class EvaluatedResult:
    harness: str
    property_statuses: Mapping[str, str]
    failed_properties: tuple[str, ...]
    concrete_values: tuple[int, ...]
    terminal_status: str


def _require_object(value: object, context: str) -> dict:
    if not isinstance(value, dict):
        raise VerificationError(f"{context} must be a JSON object")
    return value


def _require_string(value: object, context: str) -> str:
    if not isinstance(value, str) or not value:
        raise VerificationError(f"{context} must be a nonempty string")
    return value


def _require_string_list(value: object, context: str) -> list[str]:
    if not isinstance(value, list) or any(
        not isinstance(item, str) or not item for item in value
    ):
        raise VerificationError(f"{context} must be a list of nonempty strings")
    if len(value) != len(set(value)):
        raise VerificationError(f"{context} contains duplicates")
    return value


def _validate_run(run: object, context: str, expected_harnesses: set[str]) -> None:
    document = _require_object(run, context)
    _require_string(document.get("label"), f"{context}.label")
    _require_string(document.get("source"), f"{context}.source")
    harness = _require_string(document.get("harness"), f"{context}.harness")
    if harness not in expected_harnesses:
        raise VerificationError(f"{context}.harness is not in expected_harnesses")
    arguments = document.get("arguments")
    if not isinstance(arguments, list) or any(
        not isinstance(argument, str) for argument in arguments
    ):
        raise VerificationError(f"{context}.arguments must be a list of strings")
    if type(document.get("expected_exit_code")) is not int:
        raise VerificationError(f"{context}.expected_exit_code must be an integer")
    terminal = document.get("terminal_status")
    if terminal not in {"SUCCESSFUL", "FAILED"}:
        raise VerificationError(f"{context}.terminal_status is invalid")
    properties = _require_object(document.get("properties"), f"{context}.properties")
    if not properties:
        raise VerificationError(f"{context}.properties must not be empty")
    for property_id, status in properties.items():
        _require_string(property_id, f"{context}.property id")
        if status not in PROPERTY_STATUSES:
            raise VerificationError(f"{context}.properties has invalid status {status!r}")
    failed = _require_string_list(
        document.get("expected_failed_properties"),
        f"{context}.expected_failed_properties",
    )
    actual_failed = sorted(
        property_id
        for property_id, status in properties.items()
        if status == "FAILURE"
    )
    if sorted(failed) != actual_failed:
        raise VerificationError(f"{context} failed-property declaration is inconsistent")
    concrete = document.get("expected_concrete_values")
    if not isinstance(concrete, list) or any(type(value) is not int for value in concrete):
        raise VerificationError(
            f"{context}.expected_concrete_values must be a list of integers"
        )
    summary = _require_object(
        document.get("manual_summary"), f"{context}.manual_summary"
    )
    for field in ("successful_harnesses", "failures", "total"):
        if type(summary.get(field)) is not int or summary[field] < 0:
            raise VerificationError(f"{context}.manual_summary.{field} is invalid")
    expected_exit = document["expected_exit_code"]
    if failed and expected_exit == 0:
        raise VerificationError(f"{context} expects failed properties with exit code zero")
    if not failed and expected_exit != 0:
        raise VerificationError(f"{context} expects nonzero exit without a named failure")


def _validate_section(section: object, context: str, *, allow_empty: bool) -> dict:
    document = _require_object(section, context)
    expected = _require_string_list(
        document.get("expected_harnesses"), f"{context}.expected_harnesses"
    )
    runs = document.get("runs")
    if not isinstance(runs, list) or (not runs and not allow_empty):
        raise VerificationError(f"{context}.runs must be a nonempty list")
    for index, run in enumerate(runs):
        _validate_run(run, f"{context}.runs[{index}]", set(expected))
    exercised = {run["harness"] for run in runs}
    if exercised != set(expected):
        raise VerificationError(
            f"{context}.runs must exercise every expected harness and no others"
        )
    return document


def load_manifest(path: Path) -> dict:
    try:
        document = json.loads(path.read_text())
    except FileNotFoundError as error:
        raise VerificationError(f"manifest not found: {path}") from error
    except (OSError, json.JSONDecodeError) as error:
        raise VerificationError(f"cannot read manifest {path}: {error}") from error
    manifest = _require_object(document, "manifest")
    if manifest.get("schema_version") != SUPPORTED_SCHEMA_VERSION:
        raise VerificationError(
            f"unsupported manifest schema_version: {manifest.get('schema_version')!r}"
        )
    _require_string(manifest.get("toolchain_lock"), "manifest.toolchain_lock")
    timeout = manifest.get("timeout_seconds")
    if type(timeout) is not int or timeout <= 0:
        raise VerificationError("manifest.timeout_seconds must be a positive integer")
    _validate_section(manifest.get("bootstrap"), "manifest.bootstrap", allow_empty=False)
    proof_suite = _require_object(manifest.get("proof_suite"), "manifest.proof_suite")
    if type(proof_suite.get("complete")) is not bool:
        raise VerificationError("manifest.proof_suite.complete must be a boolean")
    if proof_suite["complete"]:
        sources = _require_string_list(
            proof_suite.get("sources"), "manifest.proof_suite.sources"
        )
        if not sources:
            raise VerificationError("a complete proof suite must list model sources")
        _validate_section(proof_suite, "manifest.proof_suite", allow_empty=False)
    else:
        _require_string(
            proof_suite.get("incomplete_reason"),
            "manifest.proof_suite.incomplete_reason",
        )
        if proof_suite.get("sources") != []:
            raise VerificationError("an incomplete proof suite must not list sources")
        _validate_section(proof_suite, "manifest.proof_suite", allow_empty=True)
    return manifest


def select_runs(manifest: Mapping[str, object], *, bootstrap: bool) -> Sequence[dict]:
    if bootstrap:
        return manifest["bootstrap"]["runs"]  # type: ignore[index]
    proof_suite = manifest["proof_suite"]  # type: ignore[assignment]
    if not proof_suite["complete"]:  # type: ignore[index]
        reason = proof_suite["incomplete_reason"]  # type: ignore[index]
        raise VerificationError(f"RC01-RC10 suite is incomplete: {reason}")
    return proof_suite["runs"]  # type: ignore[index]


def discover_harnesses(source: Path) -> tuple[str, ...]:
    try:
        lines = source.read_text().splitlines()
    except FileNotFoundError as error:
        raise VerificationError(f"proof source not found: {source}") from error
    except OSError as error:
        raise VerificationError(f"cannot read proof source {source}: {error}") from error

    harnesses: list[str] = []
    waiting_for_function = False
    function_pattern = re.compile(
        r"^(?:pub(?:\([^)]*\))?\s+)?(?:unsafe\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\("
    )
    for line in lines:
        stripped = line.strip()
        if stripped == "#[kani::proof]":
            if waiting_for_function:
                raise VerificationError(f"malformed Kani proof declaration in {source}")
            waiting_for_function = True
            continue
        if not waiting_for_function or not stripped or stripped.startswith("//"):
            continue
        if stripped.startswith("#["):
            continue
        match = function_pattern.match(stripped)
        if match is None:
            raise VerificationError(f"malformed Kani proof declaration in {source}")
        harnesses.append(match.group(1))
        waiting_for_function = False
    if waiting_for_function:
        raise VerificationError(f"Kani proof declaration has no function in {source}")
    if len(harnesses) != len(set(harnesses)):
        raise VerificationError(f"duplicate Kani harness name in {source}")
    return tuple(sorted(harnesses))


def require_expected_harnesses(
    discovered: Iterable[str], section: Mapping[str, object]
) -> None:
    actual = tuple(sorted(discovered))
    expected = tuple(sorted(section["expected_harnesses"]))  # type: ignore[arg-type]
    if actual != expected:
        raise VerificationError(
            "unexpected harness inventory: "
            f"expected {list(expected)!r}, discovered {list(actual)!r}"
        )


def _property_blocks(output: str) -> dict[str, tuple[str, str]]:
    pattern = re.compile(
        r"(?m)^Check\s+\d+:\s+([^\n]+)\n"
        r"\s+- Status:\s+([A-Z]+)\n"
        r"\s+- Description:\s+([^\n]*)"
    )
    properties: dict[str, tuple[str, str]] = {}
    for property_id, status, description in pattern.findall(output):
        property_id = property_id.strip()
        if property_id in properties:
            raise VerificationError(f"duplicate property result: {property_id}")
        if status not in PROPERTY_STATUSES:
            raise VerificationError(f"unknown property status {status!r}: {property_id}")
        properties[property_id] = (status, description.strip())
    if not properties:
        raise VerificationError("Kani output contains no property results")
    return properties


def evaluate_output(
    output: str,
    exit_code: int,
    expectation: Mapping[str, object],
    manifest: Mapping[str, object],
) -> EvaluatedResult:
    expected_exit = expectation["expected_exit_code"]
    if exit_code != expected_exit:
        raise VerificationError(
            f"harness {expectation['harness']} returned exit code {exit_code}; "
            f"expected {expected_exit}"
        )

    lower_output = output.lower()
    for marker in INFRASTRUCTURE_FAILURE_MARKERS:
        if marker in lower_output:
            raise VerificationError(
                f"infrastructure failure marker in {expectation['harness']}: {marker}"
            )

    lock_path = resolve_repo_path(str(manifest["toolchain_lock"]), ROOT)
    lock = load_json_object(lock_path, "toolchain lock")
    verifier_identity = f"Kani Rust Verifier {lock['version']} (standalone)"
    cbmc_version = str(lock["binaries"]["bin/cbmc"]["version"]).split()[0]
    if verifier_identity not in output or f"CBMC {cbmc_version}" not in output:
        raise VerificationError("Kani output does not contain the pinned tool identity")

    harnesses = tuple(re.findall(r"(?m)^Checking harness ([A-Za-z0-9_]+)\.\.\.$", output))
    if harnesses != (expectation["harness"],):
        raise VerificationError(
            f"Kani output harness inventory {harnesses!r} does not match "
            f"{expectation['harness']!r}"
        )

    properties = _property_blocks(output)
    for property_id, (status, description) in properties.items():
        if status == "FAILURE" and any(
            marker in f"{property_id} {description}".lower()
            for marker in SAFETY_PROPERTY_MARKERS
        ):
            raise VerificationError(
                f"safety or unwind property failed: {property_id}"
            )

    expected_properties = expectation["properties"]
    if set(properties) != set(expected_properties):  # type: ignore[arg-type]
        raise VerificationError(
            "property inventory mismatch for "
            f"{expectation['harness']}: expected {sorted(expected_properties)!r}, "
            f"observed {sorted(properties)!r}"
        )
    statuses = {property_id: status for property_id, (status, _) in properties.items()}
    if statuses != expected_properties:
        raise VerificationError(
            f"property status mismatch for {expectation['harness']}: "
            f"expected {expected_properties!r}, observed {statuses!r}"
        )

    terminal_matches = re.findall(r"(?m)^VERIFICATION:-\s+([A-Z]+)\s*$", output)
    if terminal_matches != [expectation["terminal_status"]]:
        raise VerificationError(
            f"terminal status mismatch for {expectation['harness']}: "
            f"observed {terminal_matches!r}"
        )

    summary_matches = re.findall(
        r"(?m)^Complete - (\d+) successfully verified harnesses, "
        r"(\d+) failures, (\d+) total\.\s*$",
        output,
    )
    expected_summary = expectation["manual_summary"]
    wanted_summary = (
        str(expected_summary["successful_harnesses"]),  # type: ignore[index]
        str(expected_summary["failures"]),  # type: ignore[index]
        str(expected_summary["total"]),  # type: ignore[index]
    )
    if summary_matches != [wanted_summary]:
        raise VerificationError(
            f"manual harness summary mismatch for {expectation['harness']}"
        )

    failed = tuple(sorted(
        property_id for property_id, status in statuses.items() if status == "FAILURE"
    ))
    expected_failed = tuple(sorted(expectation["expected_failed_properties"]))  # type: ignore[arg-type]
    if failed != expected_failed:
        raise VerificationError(
            f"semantic failure mismatch: expected {expected_failed!r}, observed {failed!r}"
        )

    concrete_values = tuple(int(value) for value in re.findall(r"(?m)^\s*//\s*(\d+)\s*$", output))
    expected_values = tuple(expectation["expected_concrete_values"])  # type: ignore[arg-type]
    if concrete_values != expected_values:
        raise VerificationError(
            f"counterexample mismatch for {expectation['harness']}: "
            f"expected {expected_values!r}, observed {concrete_values!r}"
        )
    if expected_values and (
        f"Concrete playback unit test for `{expectation['harness']}`:" not in output
    ):
        raise VerificationError("named concrete playback evidence is missing")

    return EvaluatedResult(
        harness=str(expectation["harness"]),
        property_statuses=statuses,
        failed_properties=failed,
        concrete_values=concrete_values,
        terminal_status=str(expectation["terminal_status"]),
    )


def run_process(
    command: Sequence[str], *, cwd: Path, timeout_seconds: float
) -> ProcessResult:
    try:
        result = subprocess.run(
            list(command),
            cwd=cwd,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=timeout_seconds,
            check=False,
        )
    except FileNotFoundError as error:
        raise VerificationError(f"tool not found: {command[0]}") from error
    except PermissionError as error:
        raise VerificationError(f"tool is not executable: {command[0]}") from error
    except subprocess.TimeoutExpired as error:
        raise VerificationError(
            f"command timed out after {timeout_seconds} seconds: {command!r}"
        ) from error
    except OSError as error:
        raise VerificationError(f"cannot execute {command[0]}: {error}") from error
    return ProcessResult(exit_code=result.returncode, output=result.stdout)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as error:
        raise VerificationError(f"cannot hash required binary {path}: {error}") from error
    return digest.hexdigest()


def require_binary_digests(lock: Mapping[str, object], toolchain_dir: Path) -> None:
    binaries = _require_object(lock.get("binaries"), "toolchain lock.binaries")
    if not binaries:
        raise VerificationError("toolchain lock has no binary digests")
    for relative, raw_metadata in binaries.items():
        _require_string(relative, "toolchain binary path")
        metadata = _require_object(raw_metadata, f"toolchain binary {relative}")
        expected = _require_string(metadata.get("sha256"), f"{relative}.sha256")
        binary = toolchain_dir / relative
        if not binary.is_file():
            raise VerificationError(f"required toolchain binary is missing: {binary}")
        actual = sha256_file(binary)
        if actual != expected:
            raise VerificationError(
                f"binary digest mismatch for {binary}: expected {expected}, got {actual}"
            )
        if not os.access(binary, os.X_OK):
            raise VerificationError(f"required toolchain binary is not executable: {binary}")


def require_toolchain(
    lock: Mapping[str, object],
    *,
    kani_executable: Path,
    toolchain_dir: Path,
    rustup_executable: Path,
    timeout_seconds: float,
) -> None:
    version = _require_string(lock.get("version"), "toolchain lock.version")
    kani_version = run_process(
        [str(kani_executable), "--version"], cwd=ROOT, timeout_seconds=timeout_seconds
    )
    expected_kani_version = f"kani {version}"
    if kani_version.exit_code != 0 or kani_version.output.strip() != expected_kani_version:
        raise VerificationError(
            f"Kani version mismatch: expected {expected_kani_version!r}, "
            f"got exit {kani_version.exit_code} output {kani_version.output.strip()!r}"
        )

    require_binary_digests(lock, toolchain_dir)

    cbmc_metadata = _require_object(
        _require_object(lock.get("binaries"), "toolchain lock.binaries").get("bin/cbmc"),
        "toolchain lock bin/cbmc",
    )
    expected_cbmc = _require_string(cbmc_metadata.get("version"), "bin/cbmc.version")
    cbmc = run_process(
        [str(toolchain_dir / "bin" / "cbmc"), "--version"],
        cwd=ROOT,
        timeout_seconds=timeout_seconds,
    )
    if cbmc.exit_code != 0 or expected_cbmc not in cbmc.output:
        raise VerificationError(
            f"CBMC version mismatch: expected {expected_cbmc!r}, got {cbmc.output.strip()!r}"
        )

    rust_toolchain = _require_string(
        lock.get("rust_toolchain"), "toolchain lock.rust_toolchain"
    )
    target = _require_string(lock.get("platform"), "toolchain lock.platform")
    suffix = f"-{target}"
    if not rust_toolchain.endswith(suffix):
        raise VerificationError("toolchain lock rust_toolchain does not match platform")
    rustup_name = rust_toolchain[: -len(suffix)]
    rustc = run_process(
        [str(rustup_executable), "run", rustup_name, "rustc", "--version"],
        cwd=ROOT,
        timeout_seconds=timeout_seconds,
    )
    expected_rustc = _require_string(
        lock.get("rustc_version"), "toolchain lock.rustc_version"
    )
    if rustc.exit_code != 0 or rustc.output.strip() != expected_rustc:
        raise VerificationError(
            f"rustc version mismatch: expected {expected_rustc!r}, "
            f"got exit {rustc.exit_code} output {rustc.output.strip()!r}"
        )


def load_json_object(path: Path, context: str) -> dict:
    try:
        value = json.loads(path.read_text())
    except FileNotFoundError as error:
        raise VerificationError(f"{context} not found: {path}") from error
    except (OSError, json.JSONDecodeError) as error:
        raise VerificationError(f"cannot read {context} {path}: {error}") from error
    return _require_object(value, context)


def resolve_repo_path(relative: str, root: Path) -> Path:
    candidate = (root / relative).resolve()
    try:
        candidate.relative_to(root.resolve())
    except ValueError as error:
        raise VerificationError(f"manifest path escapes repository: {relative}") from error
    return candidate


def execute_runs(
    runs: Sequence[Mapping[str, object]],
    manifest: Mapping[str, object],
    *,
    kani_executable: Path,
    root: Path,
    execute: Callable[..., ProcessResult] = run_process,
) -> tuple[EvaluatedResult, ...]:
    results: list[EvaluatedResult] = []
    for run in runs:
        source = resolve_repo_path(str(run["source"]), root)
        command = [
            str(kani_executable),
            str(source),
            "--harness",
            str(run["harness"]),
            *run["arguments"],  # type: ignore[misc]
        ]
        observed = execute(
            command,
            cwd=root,
            timeout_seconds=manifest["timeout_seconds"],
        )
        results.append(
            evaluate_output(observed.output, observed.exit_code, run, manifest)
        )
    return tuple(results)


def _host_platform() -> str:
    machine = {"AMD64": "x86_64", "arm64": "aarch64"}.get(
        platform.machine(), platform.machine()
    )
    if sys.platform != "linux":
        return f"{machine}-unknown-{sys.platform}-gnu"
    return f"{machine}-unknown-linux-gnu"


def _inventory_for_section(section: Mapping[str, object], root: Path) -> tuple[str, ...]:
    if "source" in section:
        sources = [section["source"]]
    else:
        sources = section["sources"]
    discovered: list[str] = []
    for relative in sources:  # type: ignore[union-attr]
        discovered.extend(discover_harnesses(resolve_repo_path(str(relative), root)))
    return tuple(sorted(discovered))


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--bootstrap",
        action="store_true",
        help="run only the pinned positive/negative verifier smoke sequence",
    )
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--kani", type=Path, default=Path("kani"))
    parser.add_argument("--rustup", type=Path, default=Path("rustup"))
    parser.add_argument("--toolchain-dir", type=Path)
    args = parser.parse_args(argv)

    try:
        manifest = load_manifest(args.manifest.resolve())
        runs = select_runs(manifest, bootstrap=args.bootstrap)
        section = manifest["bootstrap" if args.bootstrap else "proof_suite"]
        require_expected_harnesses(_inventory_for_section(section, ROOT), section)

        lock_path = resolve_repo_path(str(manifest["toolchain_lock"]), ROOT)
        lock = load_json_object(lock_path, "toolchain lock")
        expected_platform = _require_string(lock.get("platform"), "toolchain lock.platform")
        if _host_platform() != expected_platform:
            raise VerificationError(
                f"unsupported proof platform: expected {expected_platform}, got {_host_platform()}"
            )
        toolchain_dir = args.toolchain_dir
        if toolchain_dir is None:
            toolchain_dir = Path.home() / ".kani" / f"kani-{lock['version']}"
        require_toolchain(
            lock,
            kani_executable=args.kani,
            toolchain_dir=toolchain_dir,
            rustup_executable=args.rustup,
            timeout_seconds=manifest["timeout_seconds"],
        )
        results = execute_runs(
            runs,
            manifest,
            kani_executable=args.kani,
            root=ROOT,
        )
        output = {
            "schema_version": 1,
            "scope": "bootstrap" if args.bootstrap else "reserve-conservation-proofs",
            "results": [
                {
                    "harness": result.harness,
                    "terminal_status": result.terminal_status,
                    "property_statuses": dict(result.property_statuses),
                    "failed_properties": list(result.failed_properties),
                    "concrete_values": list(result.concrete_values),
                }
                for result in results
            ],
        }
        print(json.dumps(output, indent=2, sort_keys=True))
        return 0
    except VerificationError as error:
        print(f"reserve proof verification failed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
