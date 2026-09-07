import unittest

from verify_reserve_proof_controls import CONTROLS, GateError, parse_control


def shaped_output(control, *, verdict="FAILED", failure=None, concrete=True, diagnostic=""):
    description = failure or control["target_failure"]
    playback = ""
    if concrete:
        playback = (
            "let concrete_vals: Vec<Vec<u8>> = vec![\n    vec![1],\n];\n"
            f"kani::concrete_playback_run(concrete_vals, {control['harness']});\n"
        )
    status = "FAILURE" if verdict == "FAILED" else "SUCCESS"
    completion = (
        "Complete - 0 successfully verified harnesses, 1 failures, 1 total."
        if verdict == "FAILED"
        else "Complete - 1 successfully verified harnesses, 0 failures, 1 total."
    )
    return (
        "Kani Rust Verifier 0.67.0 (standalone)\n"
        f"Checking harness {control['harness']}...\n"
        "CBMC 6.8.0 (cbmc-6.8.0)\n"
        "CBMC version 6.8.0 (cbmc-6.8.0) 64-bit x86_64 linux\n"
        "Solving with CaDiCaL 2.0.0\n"
        "RESULTS:\n"
        f"Check 1: {control['harness']}.assertion.1\n"
        f" - Status: {status}\n"
        f" - Description: \"{description}\"\n"
        f" - Location: verification/reserve-conservation/control.rs:1:1 in function {control['harness']}\n"
        "SUMMARY:\n"
        f" ** {1 if verdict == 'FAILED' else 0} of 1 failed\n"
        f"VERIFICATION:- {verdict}\n"
        "Manual Harness Summary:\n"
        f"{completion}\n"
        f"{playback}{diagnostic}"
    )


class ControlParserTests(unittest.TestCase):
    def setUp(self):
        self.control = CONTROLS[1]

    def test_control_inventory_is_exactly_rc01_through_rc10(self):
        self.assertEqual([control["id"] for control in CONTROLS], [f"RC{i:02d}" for i in range(1, 11)])
        self.assertEqual(len({control["harness"] for control in CONTROLS}), 10)

    def test_accepts_only_target_semantic_failure_with_counterexample(self):
        parsed = parse_control(shaped_output(self.control), 1, self.control)
        self.assertEqual(parsed["id"], "RC02")

    def test_rejects_green_verifier_exit(self):
        with self.assertRaises(GateError):
            parse_control(shaped_output(self.control, verdict="SUCCESSFUL"), 0, self.control)

    def test_rejects_compile_or_infrastructure_failure(self):
        with self.assertRaises(GateError):
            parse_control(shaped_output(self.control, diagnostic="error: compilation failed\n"), 1, self.control)

    def test_rejects_unrelated_assertion_failure(self):
        with self.assertRaises(GateError):
            parse_control(shaped_output(self.control, failure="unrelated assertion"), 1, self.control)

    def test_rejects_missing_concrete_playback(self):
        with self.assertRaises(GateError):
            parse_control(shaped_output(self.control, concrete=False), 1, self.control)

    def test_rejects_wrong_harness_binding_in_counterexample(self):
        output = shaped_output(self.control).replace(
            f"kani::concrete_playback_run(concrete_vals, {self.control['harness']});",
            "kani::concrete_playback_run(concrete_vals, bootstrap_negative);",
        )
        with self.assertRaises(GateError):
            parse_control(output, 1, self.control)


if __name__ == "__main__":
    unittest.main()
