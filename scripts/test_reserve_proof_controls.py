"""Regression gates for the reserve-conservation disposable mutation controls."""
from pathlib import Path
import unittest

from verify_reserve_mutation_controls import CONTROL_CASES, GateError, parse_negative_control

ROOT = Path(__file__).resolve().parents[1]
MODEL = ROOT / 'verification/reserve-conservation/src/model.rs'


class MutationControlTests(unittest.TestCase):
    def test_control_inventory_and_model_anchors_are_exact(self):
        self.assertEqual(
            [case['id'] for case in CONTROL_CASES],
            [
                'control_rc01_wrapping_add',
                'control_rc02_missing_execution_equality',
                'control_rc08_omit_fee_subtraction',
                'control_rc09_saturating_subtraction',
            ],
        )
        self.assertEqual(
            [case['harness'] for case in CONTROL_CASES],
            [
                'rc01_arithmetic_matches_integer_equation',
                'rc02_result_matches_claimed_execution_total',
                'rc08_conservation_identity',
                'rc09_invalid_arithmetic_and_endpoints_reject',
            ],
        )
        baseline = MODEL.read_text()
        for case in CONTROL_CASES:
            with self.subTest(control=case['id']):
                self.assertEqual(baseline.count(case['old']), 1)
                self.assertNotEqual(case['old'], case['new'])

    def test_negative_parser_accepts_only_the_selected_rc_counterexample(self):
        control = 'control_rc01_wrapping_add'
        harness = 'rc01_arithmetic_matches_integer_equation'
        output = f'''Kani Rust Verifier 0.67.0 (standalone)
Checking harness {harness}...
CBMC 6.8.0 (cbmc-6.8.0)
Solving with CaDiCaL 2.0.0
RESULTS:
Check 1: core::num::wrapping_add.arithmetic_overflow.1
 - Status: SUCCESS
 - Description: "attempt to compute unchecked_add which would overflow"
 - Location: src/model.rs:1:1 in function core::num::wrapping_add
Check 2: {harness}.assertion.1
 - Status: FAILURE
 - Description: "RC01 accepted arithmetic matches the independent integer equation"
 - Location: proofs.rs:42:9 in function {harness}
Check 3: {harness}.cover.1
 - Status: SATISFIED
 - Description: "RC01 accepted transition is reachable"
 - Location: proofs.rs:48:5 in function {harness}
Check 4: {harness}.cover.2
 - Status: UNSATISFIABLE
 - Description: "RC01 checked-add overflow rejection is reachable"
 - Location: proofs.rs:49:5 in function {harness}
SUMMARY:
 ** 1 of 2 failed
 ** 1 of 2 cover properties satisfied
VERIFICATION:- FAILED
Concrete playback for harness `{harness}`:
```
let concrete_vals: Vec<Vec<u8>> = vec![vec![255]];
kani::concrete_playback_run(concrete_vals, {harness});
```
Manual Harness Summary:
Complete - 0 successfully verified harnesses, 1 failures, 1 total.
'''
        properties = parse_negative_control(output, 1, control)
        self.assertEqual(len(properties), 4)

        wrong = output.replace('RC01 accepted arithmetic', 'RC02 accepted arithmetic')
        with self.assertRaises(GateError):
            parse_negative_control(wrong, 1, control)

        extra_failure = output.replace(
            'Status: SATISFIED\n - Description: "RC01 accepted transition is reachable"',
            'Status: FAILURE\n - Description: "RC01 accepted transition is reachable"',
        )
        with self.assertRaises(GateError):
            parse_negative_control(extra_failure, 1, control)


if __name__ == '__main__':
    unittest.main()
