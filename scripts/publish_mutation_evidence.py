#!/usr/bin/env python3
"""Publish exact-source evidence from Oregon's existing mutation authorities."""
from pathlib import Path
import re
import subprocess

from mutation_evidence import (
    AuthoritySpec,
    EvidenceError,
    parse_authority_output,
    sha256_file,
)


GIT_SHA_RE = re.compile(r"^[0-9a-f]{40}$")


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
        "raw_log": str(log_path),
        "records": records,
    }
