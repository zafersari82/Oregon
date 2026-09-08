"""Fail-closed parser and publisher tests for Mutation Evidence V1."""
from copy import deepcopy
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from mutation_evidence import (
    AuthoritySpec,
    EvidenceError,
    KillRecord,
    MutationSpec,
    parse_authority_output,
)
from publish_mutation_evidence import (
    ROOT,
    build_result,
    ensure_output_outside_root,
    git_identity,
    git_is_clean,
    main,
    run_authority,
    validate_result_v1,
    write_result_atomic,
)


V1_COUNTS = (("EA", 3), ("EE", 9), ("CS", 17), ("ER", 13), ("FS", 14), ("RJ", 12))


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


def full_specs():
    return tuple(authority(prefix, count) for prefix, count in V1_COUNTS)


def fake_run(spec: AuthoritySpec):
    return {
        "id": spec.id,
        "runner": spec.runner,
        "runner_sha256": spec.runner_sha256,
        "command": list(spec.command),
        "raw_log": {"filename": f"{spec.id.lower()}.log", "sha256": "a" * 64},
        "records": tuple(
            KillRecord(mutation.id, mutation.name, mutation.killing_test)
            for mutation in spec.mutations
        ),
    }


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


class ResultV1Tests(unittest.TestCase):
    def result(self, specs=None, runs=None):
        specs = specs or full_specs()
        runs = runs or tuple(fake_run(spec) for spec in specs)
        return build_result(
            manifest_sha256="b" * 64,
            commit_sha="c" * 40,
            tree_sha="d" * 40,
            authorities=specs,
            authority_runs=runs,
            ci_identity={"workflow": "test", "run_id": "1"},
        )

    def test_partial_five_authority_success_is_rejected(self):
        specs = full_specs()[:-1]
        with self.assertRaises(EvidenceError):
            self.result(specs, tuple(fake_run(spec) for spec in specs))

    def test_complete_result_is_exactly_68_of_68(self):
        result = self.result()
        self.assertEqual(result["totals"], {"killed": 68, "total": 68})
        self.assertEqual(result["overall_status"], "passed")
        self.assertEqual(len(result["authorities"]), 6)
        self.assertEqual(sum(len(item["mutations"]) for item in result["authorities"]), 68)
        self.assertTrue(
            all(mutation["status"] == "killed" for item in result["authorities"] for mutation in item["mutations"])
        )

    def test_stale_schema_is_rejected(self):
        result = self.result()
        result["schema"] = "oregon.mutation-evidence.result/v0"
        with self.assertRaises(EvidenceError):
            validate_result_v1(result)

    def test_missing_source_identity_is_rejected(self):
        result = self.result()
        del result["source"]["tree"]
        with self.assertRaises(EvidenceError):
            validate_result_v1(result)

    def test_invalid_manifest_digest_is_rejected(self):
        result = self.result()
        result["manifest_sha256"] = "bad"
        with self.assertRaises(EvidenceError):
            validate_result_v1(result)

    def test_atomic_writer_is_deterministic(self):
        result = self.result()
        reordered = deepcopy(result)
        reordered["ci"] = {"run_id": "1", "workflow": "test"}
        with tempfile.TemporaryDirectory() as left, tempfile.TemporaryDirectory() as right:
            left_path = write_result_atomic(result, Path(left))
            right_path = write_result_atomic(reordered, Path(right))
            self.assertEqual(left_path.name, "result-v1.json")
            self.assertEqual(left_path.read_bytes(), right_path.read_bytes())

    def test_invalid_result_is_never_written(self):
        result = self.result()
        result["totals"] = {"killed": 67, "total": 68}
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            with self.assertRaises(EvidenceError):
                write_result_atomic(result, output)
            self.assertFalse((output / "result-v1.json").exists())


class PublicationCliTests(unittest.TestCase):
    def test_publication_retains_exact_manifest_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            manifest = ROOT / "verification/mutation-evidence/manifest-v1.json"
            with patch("publish_mutation_evidence.git_is_clean", return_value=True), patch(
                "publish_mutation_evidence.run_authority",
                side_effect=lambda root, spec, log_dir: fake_run(spec),
            ):
                status = main(["--output", str(output)])
            self.assertEqual(status, 0)
            self.assertTrue((output / "manifest-v1.json").is_file())
            self.assertEqual((output / "manifest-v1.json").read_bytes(), manifest.read_bytes())
            result = json.loads((output / "result-v1.json").read_text())
            self.assertEqual(hashlib.sha256((output / "manifest-v1.json").read_bytes()).hexdigest(), result["manifest_sha256"])

    def test_output_inside_checkout_is_rejected_before_any_authority_runs(self):
        with patch("publish_mutation_evidence.run_authority") as run:
            status = main(["--output", str(ROOT / "mutation-evidence-test-output")])
        self.assertNotEqual(status, 0)
        run.assert_not_called()

    def test_five_passes_then_failure_leave_no_canonical_result(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            (output / "result-v1.json").write_text('{"stale":true}\n')
            calls = 0

            def fail_sixth(root, spec, log_dir):
                nonlocal calls
                calls += 1
                if calls == 6:
                    raise EvidenceError("sixth authority failed")
                return fake_run(spec)

            with patch("publish_mutation_evidence.run_authority", side_effect=fail_sixth):
                status = main(["--output", str(output)])

            self.assertNotEqual(status, 0)
            self.assertEqual(calls, 6)
            self.assertFalse((output / "result-v1.json").exists())

    def test_six_passes_write_canonical_68_of_68_result(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            with patch("publish_mutation_evidence.run_authority", side_effect=lambda root, spec, log_dir: fake_run(spec)):
                status = main(["--output", str(output)])

            self.assertEqual(status, 0)
            result_path = output / "result-v1.json"
            self.assertTrue(result_path.is_file())
            result = json.loads(result_path.read_text())
            self.assertEqual(result["overall_status"], "passed")
            self.assertEqual(result["totals"], {"killed": 68, "total": 68})
            self.assertEqual(result["source"]["commit"], git_identity(ROOT)[0])

    def test_all_runner_digests_are_preflighted_before_first_authority(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch("publish_mutation_evidence.sha256_file", return_value="0" * 64), patch(
                "publish_mutation_evidence.run_authority"
            ) as run:
                status = main(["--output", directory])

        self.assertNotEqual(status, 0)
        run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
