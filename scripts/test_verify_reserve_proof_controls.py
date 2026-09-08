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
        + (
            f" ** {cover_playbacks} of {cover_playbacks} cover properties satisfied\n"
            if cover_playbacks
            else ""
        )
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
        + (
            f" ** {control['positive_covers']} of {control['positive_covers']} cover properties satisfied\n"
            if control["positive_covers"]
            else ""
        )
        + "VERIFICATION:- SUCCESSFUL\n"
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
        ).replace(" ** 1 of 1 failed\n", " ** 1 of 2 failed\n", 1)
        parsed = parse_control(output, 1, control)
        self.assertEqual(parsed["id"], "RC03")
        self.assertTrue(parsed["counterexample_bound"])

    def test_accepts_multiline_success_property_description(self):
        control = CONTROLS[2]
        output = shaped_control_output(control).replace(
            "SUMMARY:\n",
            (
                f"Check 2: {control['harness']}.safety_check.2\n"
                " - Status: SUCCESS\n"
                " - Description: \"Expected the distance between the pointers, in bytes, to be a\n"
                "                  multiple of the size of `T`\"\n"
                f" - Location: verification/reserve-conservation/control.rs:2:1 in function {control['harness']}\n"
                "SUMMARY:\n"
            ),
            1,
        ).replace(" ** 1 of 1 failed\n", " ** 1 of 2 failed\n", 1)
        parsed = parse_control(output, 1, control)
        self.assertEqual(parsed["id"], "RC03")
        self.assertTrue(parsed["counterexample_bound"])

    def test_accepts_counted_unreachable_internal_property(self):
        control = CONTROLS[2]
        output = shaped_control_output(control).replace(
            "SUMMARY:\n",
            (
                "Check 2: std::fmt::Arguments::from_str.assertion.1\n"
                " - Status: UNREACHABLE\n"
                " - Description: \"attempt to shift left with overflow\"\n"
                " - Location: library/core/src/fmt/mod.rs:820:38 in function std::fmt::Arguments::from_str\n"
                "SUMMARY:\n"
            ),
            1,
        ).replace(" ** 1 of 1 failed\n", " ** 1 of 2 failed (1 unreachable)\n", 1)
        parsed = parse_control(output, 1, control)
        self.assertEqual(parsed["id"], "RC03")
        self.assertEqual(parsed["unreachable"], 1)

    def test_accepts_unsatisfiable_cover_when_target_assertion_has_counterexample(self):
        control = CONTROLS[2]
        output = shaped_control_output(control).replace(
            "SUMMARY:\n",
            (
                f"Check 2: {control['harness']}.cover.1\n"
                " - Status: UNSATISFIABLE\n"
                " - Description: \"RC03 zero-result success is reachable\"\n"
                f" - Location: verification/reserve-conservation/state.rs:1:1 in function {control['harness']}\n"
                "SUMMARY:\n"
            ),
            1,
        ).replace(
            " ** 1 of 1 failed\n",
            " ** 1 of 1 failed\n ** 0 of 1 cover properties satisfied\n",
            1,
        )
        parsed = parse_control(output, 1, control)
        self.assertEqual(parsed["id"], "RC03")
        self.assertTrue(parsed["counterexample_bound"])

    def test_accepts_counted_unreachable_cover_only_in_negative_control(self):
        control = CONTROLS[2]
        output = shaped_control_output(control).replace(
            "SUMMARY:\n",
            (
                f"Check 2: {control['harness']}.cover.1\n"
                " - Status: UNREACHABLE\n"
                " - Description: \"RC03 positive result is reachable\"\n"
                f" - Location: verification/reserve-conservation/state.rs:1:1 in function {control['harness']}\n"
                "SUMMARY:\n"
            ),
            1,
        ).replace(" ** 1 of 1 failed\n", " ** 1 of 1 failed\n ** 0 of 1 cover properties satisfied (1 unreachable)\n", 1)
        self.assertTrue(parse_control(output, 1, control)["counterexample_bound"])
        with self.assertRaises(GateError):
            parse_control(output.replace("(1 unreachable)", "(2 unreachable)"), 1, control)

    def test_positive_rejects_unreachable_cover(self):
        output = shaped_positive_output(self.control).replace("Status: SATISFIED", "Status: UNREACHABLE")
        output = output.replace("1 of 1 cover properties satisfied", "0 of 1 cover properties satisfied (1 unreachable)")
        with self.assertRaises(GateError):
            parse_positive(output, 0, self.control)

    def test_rejects_unreachable_summary_count_mismatch(self):
        control = CONTROLS[2]
        output = shaped_control_output(control).replace(
            " ** 1 of 1 failed\n", " ** 1 of 1 failed (1 unreachable)\n", 1
        )
        with self.assertRaises(GateError):
            parse_control(output, 1, control)

    def test_accepts_complete_positive_rerun(self):
        parsed = parse_positive(shaped_positive_output(self.control), 0, self.control)
        self.assertEqual(parsed["id"], "RC02")
        self.assertEqual(parsed["covers"], 1)

    def test_missing_cover_playback_reports_exact_missing_evidence(self):
        control = CONTROLS[5]
        output = shaped_control_output(control, cover_playbacks=2)
        output = output.replace(
            playback_block(control, "cover", "RC06 reachability 2", "cover_2"), ""
        )
        with self.assertRaisesRegex(GateError, "missing=.*RC06 reachability 2"):
            parse_control(output, 1, control)

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
