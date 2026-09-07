#!/usr/bin/env python3
"""Behavior tests for the reserve proof runner and Kani output parser."""

from __future__ import annotations

import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import textwrap
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "verify_reserve_proofs.py"
MANIFEST = ROOT / "verification" / "reserve-conservation" / "proof-manifest.json"
BOOTSTRAP_EVIDENCE = (
    ROOT / "verification" / "reserve-conservation" / "evidence" / "bootstrap"
)


def load_runner():
    spec = importlib.util.spec_from_file_location("verify_reserve_proofs", SCRIPT)
    if spec is None or spec.loader is None:
        raise AssertionError("unable to construct reserve proof runner import")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


runner = load_runner()


def bootstrap_manifest() -> dict:
    return json.loads(MANIFEST.read_text())


def observed_output(name: str) -> str:
    document = json.loads((BOOTSTRAP_EVIDENCE / name).read_text())
    return document["output"]


class BootstrapOutputTests(unittest.TestCase):
    def setUp(self) -> None:
        self.manifest = bootstrap_manifest()

    def expectation(self, index: int) -> dict:
        return self.manifest["bootstrap"]["runs"][index]

    def test_accepts_both_real_positive_observations(self) -> None:
        for fixture in ("positive.json", "positive_after_negative.json"):
            with self.subTest(fixture=fixture):
                result = runner.evaluate_output(
                    observed_output(fixture), 0, self.expectation(0), self.manifest
                )
                self.assertEqual(result.failed_properties, ())
                self.assertEqual(
                    result.property_statuses["bootstrap_positive.cover.1"],
                    "SATISFIED",
                )

    def test_accepts_real_negative_only_for_named_semantic_counterexample(self) -> None:
        result = runner.evaluate_output(
            observed_output("negative.json"), 1, self.expectation(1), self.manifest
        )
        self.assertEqual(
            result.failed_properties, ("bootstrap_negative.assertion.1",)
        )
        self.assertEqual(result.concrete_values, (255,))

    def test_rejects_nonzero_exit_when_named_property_did_not_fail(self) -> None:
        output = observed_output("negative.json").replace(
            "bootstrap_negative.assertion.1", "bootstrap_negative.assertion.99"
        )
        with self.assertRaisesRegex(runner.VerificationError, "property inventory"):
            runner.evaluate_output(output, 1, self.expectation(1), self.manifest)

    def test_rejects_wrong_nonzero_exit_even_with_expected_counterexample(self) -> None:
        with self.assertRaisesRegex(runner.VerificationError, "exit code 2"):
            runner.evaluate_output(
                observed_output("negative.json"), 2, self.expectation(1), self.manifest
            )

    def test_rejects_missing_cover_property(self) -> None:
        output = observed_output("positive.json").replace(
            "Check 3: bootstrap_positive.cover.1\n"
            '\t - Status: SATISFIED\n'
            '\t - Description: "maximum input is reachable"\n'
            "\t - Location: ../workspace/scratch/6fb76c82236f/Oregon/verification/"
            "reserve-conservation/bootstrap.rs:11:5 in function bootstrap_positive\n\n",
            "",
        )
        with self.assertRaisesRegex(runner.VerificationError, "property inventory"):
            runner.evaluate_output(output, 0, self.expectation(0), self.manifest)

    def test_rejects_unexpected_property_even_when_it_succeeds(self) -> None:
        output = observed_output("positive.json").replace(
            "\n\nSUMMARY:",
            "\nCheck 4: bootstrap_positive.assertion.99\n"
            "\t - Status: SUCCESS\n"
            '\t - Description: "unexpected"\n\n\nSUMMARY:',
        )
        with self.assertRaisesRegex(runner.VerificationError, "property inventory"):
            runner.evaluate_output(output, 0, self.expectation(0), self.manifest)

    def test_rejects_compiler_or_unsupported_failure_in_negative_control(self) -> None:
        base = observed_output("negative.json")
        for marker in (
            "error: could not compile `reserve-conservation-proof`",
            "unsupported operation reached",
        ):
            with self.subTest(marker=marker):
                with self.assertRaisesRegex(
                    runner.VerificationError, "infrastructure failure"
                ):
                    runner.evaluate_output(
                        base + "\n" + marker, 1, self.expectation(1), self.manifest
                    )

    def test_rejects_failed_unwind_or_memory_safety_check(self) -> None:
        base = observed_output("negative.json")
        for property_id, description in (
            ("bootstrap_negative.unwind.1", "unwinding assertion loop 0"),
            ("bootstrap_negative.pointer_dereference.1", "pointer dereference"),
        ):
            with self.subTest(property_id=property_id):
                output = base.replace(
                    "\n\nSUMMARY:",
                    f"\nCheck 2: {property_id}\n"
                    "\t - Status: FAILURE\n"
                    f'\t - Description: "{description}"\n\n\nSUMMARY:',
                )
                with self.assertRaisesRegex(
                    runner.VerificationError, "safety or unwind property"
                ):
                    runner.evaluate_output(
                        output, 1, self.expectation(1), self.manifest
                    )

    def test_rejects_missing_terminal_and_manual_summaries(self) -> None:
        output = observed_output("positive.json").replace(
            "VERIFICATION:- SUCCESSFUL", "VERIFICATION STATUS OMITTED"
        )
        with self.assertRaisesRegex(runner.VerificationError, "terminal status"):
            runner.evaluate_output(output, 0, self.expectation(0), self.manifest)

        output = observed_output("positive.json").replace(
            "Complete - 1 successfully verified harnesses, 0 failures, 1 total.",
            "Complete - summary omitted.",
        )
        with self.assertRaisesRegex(runner.VerificationError, "harness summary"):
            runner.evaluate_output(output, 0, self.expectation(0), self.manifest)

    def test_rejects_wrong_verifier_and_backend_versions(self) -> None:
        base = observed_output("positive.json")
        for old, new in (("0.67.0", "0.68.0"), ("CBMC 6.8.0", "CBMC 6.9.0")):
            with self.subTest(new=new):
                with self.assertRaisesRegex(runner.VerificationError, "identity"):
                    runner.evaluate_output(
                        base.replace(old, new), 0, self.expectation(0), self.manifest
                    )


class ManifestAndInventoryTests(unittest.TestCase):
    def test_committed_manifest_is_strictly_incomplete_for_rc_suite(self) -> None:
        manifest = runner.load_manifest(MANIFEST)
        self.assertFalse(manifest["proof_suite"]["complete"])
        with self.assertRaisesRegex(runner.VerificationError, "RC01-RC10 suite is incomplete"):
            runner.select_runs(manifest, bootstrap=False)

    def test_bootstrap_source_has_exact_manifest_inventory(self) -> None:
        manifest = bootstrap_manifest()
        source = ROOT / manifest["bootstrap"]["source"]
        self.assertEqual(
            runner.discover_harnesses(source),
            ("bootstrap_negative", "bootstrap_positive"),
        )
        runner.require_expected_harnesses(
            ("bootstrap_negative", "bootstrap_positive"), manifest["bootstrap"]
        )

    def test_rejects_unexpected_harness_inventory(self) -> None:
        manifest = bootstrap_manifest()
        with self.assertRaisesRegex(runner.VerificationError, "unexpected harness inventory"):
            runner.require_expected_harnesses(
                ("bootstrap_negative", "bootstrap_positive", "unreviewed_harness"),
                manifest["bootstrap"],
            )

    def test_rejects_malformed_or_missing_manifest_fields(self) -> None:
        valid = bootstrap_manifest()
        malformed_documents = (
            [],
            {"schema_version": 99},
            {key: value for key, value in valid.items() if key != "toolchain_lock"},
            {**valid, "bootstrap": {**valid["bootstrap"], "runs": []}},
        )
        for document in malformed_documents:
            with self.subTest(document=document):
                with tempfile.TemporaryDirectory() as directory:
                    path = Path(directory) / "manifest.json"
                    path.write_text(json.dumps(document))
                    with self.assertRaises(runner.VerificationError):
                        runner.load_manifest(path)


class ProcessAndToolchainTests(unittest.TestCase):
    def test_missing_tool_fails_closed(self) -> None:
        with self.assertRaisesRegex(runner.VerificationError, "tool not found"):
            runner.run_process(
                ["/path/that/does/not/exist/kani", "--version"],
                cwd=ROOT,
                timeout_seconds=1,
            )

    def test_timeout_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            executable = Path(directory) / "slow-tool"
            executable.write_text("#!/bin/sh\nexec sleep 5\n")
            executable.chmod(0o755)
            with self.assertRaisesRegex(runner.VerificationError, "timed out"):
                runner.run_process(
                    [str(executable)], cwd=ROOT, timeout_seconds=0.01
                )

    def test_toolchain_rejects_wrong_version_before_proofs(self) -> None:
        manifest = bootstrap_manifest()
        lock = json.loads((ROOT / manifest["toolchain_lock"]).read_text())
        with tempfile.TemporaryDirectory() as directory:
            temp = Path(directory)
            kani = temp / "kani"
            kani.write_text("#!/bin/sh\necho 'kani 0.68.0'\n")
            kani.chmod(0o755)
            with self.assertRaisesRegex(runner.VerificationError, "Kani version"):
                runner.require_toolchain(
                    lock,
                    kani_executable=kani,
                    toolchain_dir=temp / "toolchain",
                    rustup_executable=temp / "rustup",
                    timeout_seconds=1,
                )

    def test_binary_digest_mismatch_fails_closed(self) -> None:
        lock = json.loads(
            (ROOT / "verification" / "reserve-conservation" / "toolchain-lock.json").read_text()
        )
        with tempfile.TemporaryDirectory() as directory:
            toolchain = Path(directory)
            binary = toolchain / "bin" / "cbmc"
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b"wrong binary")
            with self.assertRaisesRegex(runner.VerificationError, "digest mismatch"):
                runner.require_binary_digests(lock, toolchain)


class CommandLineTests(unittest.TestCase):
    def test_default_command_refuses_incomplete_rc_suite_before_tool_lookup(self) -> None:
        result = subprocess.run(
            [sys.executable, str(SCRIPT)],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            env={"PATH": ""},
        )
        self.assertEqual(result.returncode, 2, result.stdout)
        self.assertIn("RC01-RC10 suite is incomplete", result.stdout)

    def test_bootstrap_executor_requires_each_exact_result_in_sequence(self) -> None:
        manifest = bootstrap_manifest()
        runs = manifest["bootstrap"]["runs"]
        fixture_for_harness = {
            "bootstrap_positive": observed_output("positive.json"),
            "bootstrap_negative": observed_output("negative.json"),
        }
        calls: list[str] = []

        def execute(command, *, cwd, timeout_seconds):
            del cwd, timeout_seconds
            harness = command[command.index("--harness") + 1]
            calls.append(harness)
            return runner.ProcessResult(
                exit_code=0 if harness == "bootstrap_positive" else 1,
                output=fixture_for_harness[harness],
            )

        results = runner.execute_runs(
            runs,
            manifest,
            kani_executable=Path("kani"),
            root=ROOT,
            execute=execute,
        )
        self.assertEqual(
            calls,
            ["bootstrap_positive", "bootstrap_negative", "bootstrap_positive"],
        )
        self.assertEqual(len(results), 3)

    def test_bootstrap_executor_does_not_accept_generic_nonzero(self) -> None:
        manifest = copy.deepcopy(bootstrap_manifest())
        negative = manifest["bootstrap"]["runs"][1]

        def execute(command, *, cwd, timeout_seconds):
            del command, cwd, timeout_seconds
            return runner.ProcessResult(exit_code=1, output="compiler crashed")

        with self.assertRaises(runner.VerificationError):
            runner.execute_runs(
                (negative,),
                manifest,
                kani_executable=Path("kani"),
                root=ROOT,
                execute=execute,
            )


if __name__ == "__main__":
    unittest.main()
