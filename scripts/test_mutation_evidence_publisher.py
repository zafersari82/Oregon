"""Fail-closed parser and publisher tests for Mutation Evidence V1."""
import hashlib
from pathlib import Path
import subprocess
import tempfile
import unittest

from mutation_evidence import (
    AuthoritySpec,
    EvidenceError,
    KillRecord,
    MutationSpec,
    parse_authority_output,
)
from publish_mutation_evidence import (
    ensure_output_outside_root,
    git_identity,
    git_is_clean,
    run_authority,
)


def authority(prefix="EA", count=3, runner=None, digest=None):
    mutations = tuple(
        MutationSpec(
            id=f"{prefix}-{index:03d}",
            name=f"mutation {index}",
            target=f"target/{index}",
            expected_broken_behavior=f"broken {index}",
            killing_test=f"test_{index}",
            kill_classification="semantic-test-failure",
        )
        for index in range(1, count + 1)
    )
    runner = runner or f"scripts/{prefix.lower()}_runner.py"
    return AuthoritySpec(
        id=prefix,
        display_name=prefix,
        runner=runner,
        runner_sha256=digest or "0" * 64,
        command=("python3", runner),
        expected_count=count,
        mutations=mutations,
    )


def successful_output(spec, syntax="dash"):
    lines = []
    for mutation in spec.mutations:
        if syntax == "dash":
            lines.append(f"KILLED: {mutation.name} — {mutation.killing_test}")
        else:
            lines.append(f"KILLED: {mutation.name} [{mutation.killing_test}]")
    if syntax == "dash":
        lines.append(
            f"{spec.expected_count}/{spec.expected_count} mutations killed; "
            "restored clean suite passed"
        )
    else:
        lines.append(
            f"Execution resource mutations: "
            f"{spec.expected_count}/{spec.expected_count} killed"
        )
    return "\n".join(lines) + "\n"


def init_repo(root: Path):
    subprocess.run(["git", "init", "-q"], cwd=root, check=True)
    (root / "seed.txt").write_text("seed\n")
    subprocess.run(["git", "add", "seed.txt"], cwd=root, check=True)
    subprocess.run(
        [
            "git",
            "-c", "user.name=Oregon Test",
            "-c", "user.email=test@example.invalid",
            "commit", "-qm", "seed",
        ],
        cwd=root,
        check=True,
    )


def write_runner(root: Path, spec: AuthoritySpec, output: str, *, dirty=False, exit_code=0):
    path = root / spec.runner
    path.parent.mkdir(parents=True, exist_ok=True)
    body = ["from pathlib import Path", "import sys"]
    if dirty:
        body.append("Path('dirty.txt').write_text('dirty\\n')")
    body.append(f"print({output!r}, end='')")
    body.append(f"raise SystemExit({exit_code})")
    path.write_text("\n".join(body) + "\n")
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    return authority(spec.id, spec.expected_count, spec.runner, digest)


class AuthorityOutputParserTests(unittest.TestCase):
    def test_accepts_em_dash_records(self):
        spec = authority("EA", 3)
        records = parse_authority_output(spec, successful_output(spec, "dash"), 0)
        self.assertEqual(len(records), 3)
        self.assertEqual(records[0], KillRecord("EA-001", "mutation 1", "test_1"))

    def test_accepts_bracket_records(self):
        spec = authority("ER", 13)
        records = parse_authority_output(spec, successful_output(spec, "bracket"), 0)
        self.assertEqual(len(records), 13)
        self.assertEqual(records[-1].mutation_id, "ER-013")

    def assert_rejected(self, output, returncode=0, spec=None):
        with self.assertRaises(EvidenceError):
            parse_authority_output(spec or authority(), output, returncode)

    def test_duplicate_kill_record_is_rejected(self):
        spec = authority()
        output = successful_output(spec)
        duplicate = "KILLED: mutation 1 — test_1\n"
        self.assert_rejected(output.replace("KILLED: mutation 2 — test_2\n", duplicate), spec=spec)

    def test_unknown_mutation_is_rejected(self):
        spec = authority()
        output = successful_output(spec).replace("mutation 2 — test_2", "unknown — test_2")
        self.assert_rejected(output, spec=spec)

    def test_wrong_killing_test_is_rejected(self):
        spec = authority()
        output = successful_output(spec).replace("mutation 2 — test_2", "mutation 2 — wrong_test")
        self.assert_rejected(output, spec=spec)

    def test_summary_mismatch_is_rejected(self):
        spec = authority()
        output = successful_output(spec).replace("3/3 mutations killed", "2/3 mutations killed")
        self.assert_rejected(output, spec=spec)

    def test_missing_summary_is_rejected(self):
        spec = authority()
        output = "\n".join(successful_output(spec).splitlines()[:-1]) + "\n"
        self.assert_rejected(output, spec=spec)

    def test_missing_kill_is_rejected_even_with_green_summary(self):
        spec = authority()
        output = successful_output(spec).replace("KILLED: mutation 2 — test_2\n", "")
        self.assert_rejected(output, spec=spec)

    def test_nonzero_runner_exit_is_rejected(self):
        spec = authority()
        self.assert_rejected(successful_output(spec), returncode=1, spec=spec)

    def test_compiler_failure_cannot_masquerade_as_kill(self):
        spec = authority()
        for marker in ("could not compile oregon-utxo", "error[E0308]: mismatched types"):
            with self.subTest(marker=marker):
                self.assert_rejected(successful_output(spec) + marker + "\n", spec=spec)

    def test_infrastructure_failure_cannot_masquerade_as_kill(self):
        spec = authority()
        for marker in ("Traceback (most recent call last):", "process timed out"):
            with self.subTest(marker=marker):
                self.assert_rejected(successful_output(spec) + marker + "\n", spec=spec)

    def test_bare_aggregate_is_never_sufficient(self):
        spec = authority()
        self.assert_rejected("3/3 mutations killed; restored clean suite passed\n", spec=spec)


class SourceIntegrityTests(unittest.TestCase):
    def test_git_identity_returns_exact_commit_and_tree(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            init_repo(root)
            commit, tree = git_identity(root)
            self.assertRegex(commit, r"^[0-9a-f]{40}$")
            self.assertRegex(tree, r"^[0-9a-f]{40}$")

    def test_git_identity_rejects_non_repository(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(EvidenceError):
                git_identity(Path(directory))

    def test_git_cleanliness_detects_dirty_checkout(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            init_repo(root)
            self.assertTrue(git_is_clean(root))
            (root / "dirty.txt").write_text("dirty\n")
            self.assertFalse(git_is_clean(root))

    def test_output_directory_inside_checkout_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaises(EvidenceError):
                ensure_output_outside_root(root, root / "evidence")

    def test_stale_runner_digest_is_rejected_before_execution(self):
        with tempfile.TemporaryDirectory() as repo, tempfile.TemporaryDirectory() as logs:
            root = Path(repo)
            init_repo(root)
            spec = authority()
            path = root / spec.runner
            path.parent.mkdir(parents=True)
            path.write_text("raise SystemExit('must not run')\n")
            subprocess.run(["git", "add", spec.runner], cwd=root, check=True)
            subprocess.run(
                ["git", "-c", "user.name=Oregon Test", "-c", "user.email=test@example.invalid", "commit", "-qm", "runner"],
                cwd=root,
                check=True,
            )
            with self.assertRaises(EvidenceError):
                run_authority(root, spec, Path(logs))

    def test_authority_success_requires_clean_restoration(self):
        with tempfile.TemporaryDirectory() as repo, tempfile.TemporaryDirectory() as logs:
            root = Path(repo)
            init_repo(root)
            base = authority()
            spec = write_runner(root, base, successful_output(base))
            subprocess.run(["git", "add", spec.runner], cwd=root, check=True)
            subprocess.run(
                ["git", "-c", "user.name=Oregon Test", "-c", "user.email=test@example.invalid", "commit", "-qm", "runner"],
                cwd=root,
                check=True,
            )
            result = run_authority(root, spec, Path(logs))
            self.assertEqual(result["id"], "EA")
            self.assertEqual(len(result["records"]), 3)
            self.assertTrue((Path(logs) / "ea.log").is_file())
            self.assertTrue(git_is_clean(root))

    def test_dirty_after_authority_execution_is_rejected(self):
        with tempfile.TemporaryDirectory() as repo, tempfile.TemporaryDirectory() as logs:
            root = Path(repo)
            init_repo(root)
            base = authority()
            spec = write_runner(root, base, successful_output(base), dirty=True)
            subprocess.run(["git", "add", spec.runner], cwd=root, check=True)
            subprocess.run(
                ["git", "-c", "user.name=Oregon Test", "-c", "user.email=test@example.invalid", "commit", "-qm", "runner"],
                cwd=root,
                check=True,
            )
            with self.assertRaises(EvidenceError):
                run_authority(root, spec, Path(logs))

    def test_nonzero_authority_process_is_rejected(self):
        with tempfile.TemporaryDirectory() as repo, tempfile.TemporaryDirectory() as logs:
            root = Path(repo)
            init_repo(root)
            base = authority()
            spec = write_runner(root, base, successful_output(base), exit_code=1)
            subprocess.run(["git", "add", spec.runner], cwd=root, check=True)
            subprocess.run(
                ["git", "-c", "user.name=Oregon Test", "-c", "user.email=test@example.invalid", "commit", "-qm", "runner"],
                cwd=root,
                check=True,
            )
            with self.assertRaises(EvidenceError):
                run_authority(root, spec, Path(logs))


if __name__ == "__main__":
    unittest.main()
