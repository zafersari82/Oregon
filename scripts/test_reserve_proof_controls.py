"""Regression gates for the reserve-conservation mutation controls."""
from pathlib import Path
import re
import unittest

from verify_reserve_proofs import GateError, parse_negative_control

ROOT = Path(__file__).resolve().parents[1]
CONTROL_SOURCE = ROOT / 'verification/reserve-conservation/mutation_controls.rs'


class MutationControlTests(unittest.TestCase):
    def test_control_harness_inventory_is_exact(self):
        self.assertTrue(CONTROL_SOURCE.is_file(), 'mutation control source must exist')
        harnesses = re.findall(r'^fn (control_rc\d+_[a-z0-9_]+)\(', CONTROL_SOURCE.read_text(), re.M)
        self.assertEqual(
            harnesses,
            [
                'control_rc01_wrapping_add',
                'control_rc02_missing_execution_equality',
                'control_rc08_omit_fee_subtraction',
                'control_rc09_saturating_subtraction',
            ],
        )

    def test_negative_parser_accepts_only_the_intended_counterexample(self):
        harness = 'control_rc01_wrapping_add'
        output = f'''Kani Rust Verifier 0.67.0 (standalone)
Checking harness {harness}...
CBMC 6.8.0 (cbmc-6.8.0)
Solving with CaDiCaL 2.0.0
RESULTS:
Check 1: core::num::wrapping_add.arithmetic_overflow.1
 - Status: SUCCESS
 - Description: "attempt to compute unchecked_add which would overflow"
 - Location: mutation_controls.rs:1:1 in function core::num::wrapping_add
Check 2: {harness}.assertion.1
 - Status: FAILURE
 - Description: "RC01 control: wrapping addition must not satisfy the integer equation on overflow"
 - Location: mutation_controls.rs:20:5 in function {harness}
SUMMARY:
 ** 1 of 2 failed
VERIFICATION:- FAILED
Concrete playback for harness `{harness}`:
```
let concrete_vals: Vec<Vec<u8>> = vec![vec![255]];
kani::concrete_playback_run(concrete_vals, {harness});
```
Manual Harness Summary:
Complete - 0 successfully verified harnesses, 1 failures, 1 total.
'''
        properties = parse_negative_control(output, 1, harness)
        self.assertEqual(len(properties), 2)

        wrong = output.replace('RC01 control:', 'RC02 control:')
        with self.assertRaises(GateError):
            parse_negative_control(wrong, 1, harness)


if __name__ == '__main__':
    unittest.main()
