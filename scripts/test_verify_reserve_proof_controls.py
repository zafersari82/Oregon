import unittest

from verify_reserve_proof_controls import (
    CONTROLS,
    MODEL_RELATIVE,
    ROOT,
    GateError,
    apply_mutation,
    parse_control,
    parse_positive,
    validate_control_manifest,
)


def playback_block(control, kind, description, suffix):
    return (
        f"Concrete playback unit test for `{control['harness']}`:\n"
        "```\n"
        f"/// Test generated for harness `{control['harness']}` \n"
        "///\n"
        f"/// Check for `{kind}`: \"\"{description}\"\"\n"
        "\n"
        "#[test]\n"
        f"fn kani_concrete_playback_{control['harness']}_{suffix}() {{\n"
        "    let concrete_vals: Vec<Vec<u8>> = vec![\n"
        "        vec![1],\n"
        "    ];\n"
        f"    kani::concrete_playback_run(concrete_vals, {control['harness']});\n"
        "}\n"
        "```\n"
    )


def shaped_control_output(
    control,
    *,
    return_verdict="FAILED",
    failure=None,
    concrete=True,
    cover_playbacks=0,
    diagnostic="",
):
    description = failure or control["target_failure"]
    status = "FAILURE" if return_verdict == "FAILED" else "SUCCESS"
    failed_count = 1 if return_verdict == "FAILED" else 0
    properties = (
        f"Check 1: {control['harness']}.assertion.1\n"
        f" - Status: {status}\n"
        f" - Description: \"{description}\"\n"
        f" - Location: verification/reserve-conservation/control.rs:1:1 in function {control['harness']}\n"
    )
    for index in range(cover_playbacks):
        properties += (
            f"Check {index + 2}: {control['harness']}.cover.{index + 1}\n"
            " - Status: SATISFIED\n"
            f" - Description: \"{control['id']} reachability {index + 1}\"\n"
            f" - Location: verification/reserve-conservation/control.rs:2:1 in function {control['harness']}\n"
        )
    playback = ""
    if concrete:
        playback = playback_block(control, "assertion", description, "assertion")
        for index in range(cover_playbacks):
            playback += playback_block(
                control,
                "cover",
                f"{control['id']} reachability {index + 1}",
                f"cover_{index + 1}",
            )
    completion = (
        "Complete - 0 successfully verified harnesses, 1 failures, 1 total."
        if return_verdict == "FAILED"
        else "Complete - 1 successfully verified harnesses, 0 failures, 1 total."
    )
    return (
        "Kani Rust Verifier 0.67.0 (standalone)\n"
        f"Checking harness {control['harness']}...\n"
        "CBMC 6.8.0 (cbmc-6.8.0)\n"
        "CBMC version 6.8.0 (cbmc-6.8.0) 64-bit x86_64 linux\n"
        "Solving with CaDiCaL 2.0.0\n"
        "RESULTS:\n"
        + properties
        + "SUMMARY:\n"
        f" ** {failed_count} of 1 failed\n"
        + (f"Failed Checks: \"{description}\"\n" if return_verdict == "FAILED" else "")
        + f"VERIFICATION:- {return_verdict}\n"
        + playback
        + diagnostic
        + "Manual Harness Summary:\n"
        + completion
        + "\n"
    )


def shaped_positive_output(control):
    checks = [
        (
            f"{control['harness']}.assertion.1",
            "SUCCESS",
            control["target_failure"],
        )
    ]
    for index in range(control["positive_covers"]):
        checks.append(
            (
                f"{control['harness']}.cover.{index + 1}",
                "SATISFIED",
                f"{control['id']} reachability {index + 1}",
            )
        )
    properties = "".join(
        f"Check {index}: {name}\n"
        f" - Status: {status}\n"
        f" - Description: \"{description}\"\n"
        f" - Location: verification/reserve-conservation/control.rs:1:1 in function {control['harness']}\n"
        for index, (name, status, description) in enumerate(checks, 1)
    )
    return (
        "Kani Rust Verifier 0.67.0 (standalone)\n"
        f"Checking harness {control['harness']}...\n"
        "CBMC 6.8.0 (cbmc-6.8.0)\n"
        "CBMC version 6.8.0 (cbmc-6.8.0) 64-bit x86_64 linux\n"
        "Solving with CaDiCaL 2.0.0\n"
        "RESULTS:\n"
        + properties
        + "SUMMARY:\n"
        " ** 0 of 1 failed\n"
        "VERIFICATION:- SUCCESSFUL\n"
        "Manual Harness Summary:\n"
        "Complete - 1 successfully verified harnesses, 0 failures, 1 total.\n"
    )


class ControlParserTests(unittest.TestCase):
    def setUp(self):
        self.control = CONTROLS[1]

    def test_control_inventory_is_exactly_rc01_through_rc10(self):
        self.assertEqual(
            [control["id"] for control in CONTROLS],
            [f"RC{i:02d}" for i in range(1, 11)],
        )
        self.assertEqual(len({control["harness"] for control in CONTROLS}), 10)

    def test_all_mutation_anchors_are_unique_on_the_baseline_model(self):
        model = (ROOT / MODEL_RELATIVE).read_text()
        validate_control_manifest(model)
        for control in CONTROLS:
            with self.subTest(control=control["id"]):
                mutated = apply_mutation(model, control)
                self.assertNotEqual(mutated, model)

    def test_accepts_only_target_semantic_failure_with_counterexample(self):
        parsed = parse_control(shaped_control_output(self.control), 1, self.control)
        self.assertEqual(parsed["id"], "RC02")
        self.assertTrue(parsed["counterexample_bound"])

    def test_accepts_target_assertion_playback_with_additional_cover_playbacks(self):
        control = CONTROLS[0]
        parsed = parse_control(
            shaped_control_output(control, cover_playbacks=control["positive_covers"]),
            1,
            control,
        )
        self.assertEqual(parsed["id"], "RC01")
        self.assertTrue(parsed["counterexample_bound"])
        self.assertEqual(parsed["playbacks"], 1 + control["positive_covers"])

    def test_accepts_successful_kani_unreachable_property_description(self):
        control = CONTROLS[2]
        output = shaped_control_output(control).replace(
            "SUMMARY:\n",
            (
                f"Check 2: {control['harness']}.unreachable.1\n"
                " - Status: SUCCESS\n"
                " - Description: \"internal error: entered unreachable code\"\n"
                f" - Location: verification/reserve-conservation/control.rs:2:1 in function {control['harness']}\n"
                "SUMMARY:\n"
            ),
            1,
        )
        parsed = parse_control(output, 1, control)
        self.assertEqual(parsed["id"], "RC03")
        self.assertTrue(parsed["counterexample_bound"])

    def test_accepts_complete_positive_rerun(self):
        parsed = parse_positive(shaped_positive_output(self.control), 0, self.control)
        self.assertEqual(parsed["id"], "RC02")
        self.assertEqual(parsed["covers"], 1)

    def test_rejects_green_verifier_exit(self):
        with self.assertRaises(GateError):
            parse_control(
                shaped_control_output(self.control, return_verdict="SUCCESSFUL"),
                0,
                self.control,
            )

    def test_rejects_compile_or_infrastructure_failure(self):
        with self.assertRaises(GateError):
            parse_control(
                shaped_control_output(self.control, diagnostic="error: compilation failed\n"),
                1,
                self.control,
            )

    def test_rejects_unrelated_assertion_failure(self):
        with self.assertRaises(GateError):
            parse_control(
                shaped_control_output(self.control, failure="unrelated assertion"),
                1,
                self.control,
            )

    def test_rejects_missing_concrete_playback(self):
        with self.assertRaises(GateError):
            parse_control(
                shaped_control_output(self.control, concrete=False),
                1,
                self.control,
            )

    def test_rejects_wrong_harness_binding_in_counterexample(self):
        output = shaped_control_output(self.control).replace(
            f"kani::concrete_playback_run(concrete_vals, {self.control['harness']});",
            "kani::concrete_playback_run(concrete_vals, bootstrap_negative);",
        )
        with self.assertRaises(GateError):
            parse_control(output, 1, self.control)

    def test_rejects_missing_target_assertion_on_positive_rerun(self):
        output = shaped_positive_output(self.control).replace(
            self.control["target_failure"],
            "different assertion",
        )
        with self.assertRaises(GateError):
            parse_positive(output, 0, self.control)


if __name__ == "__main__":
    unittest.main()
