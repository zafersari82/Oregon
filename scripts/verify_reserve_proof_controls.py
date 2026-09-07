"""Fail-closed semantic negative-control gate for Reserve Conservation Proof V1."""

from verify_reserve_proofs import GateError, require

CONTROLS = (
    {"id": "RC01", "harness": "rc01_accepted_arithmetic_matches_mathematical_equation", "target_failure": "RC01 accepted arithmetic must equal the independent mathematical equation"},
    {"id": "RC02", "harness": "rc02_accepted_result_equals_claimed_execution_total", "target_failure": "RC02 accepted result must equal the caller-supplied execution total"},
    {"id": "RC03", "harness": "rc03_successful_result_has_exact_reserve_cardinality", "target_failure": "RC03 zero result has no reserve and positive result has exactly one"},
    {"id": "RC04", "harness": "rc04_two_live_reserves_reject_unchanged", "target_failure": "RC04 two reserves must reject"},
    {"id": "RC05", "harness": "rc05_successful_creation_has_exact_program_value_and_metadata", "target_failure": "RC05 created reserve must have exact modeled program, value and metadata"},
    {"id": "RC06", "harness": "rc06_failed_apply_and_failed_undo_are_atomic", "target_failure": "RC06 failed apply must leave every slot unchanged"},
    {"id": "RC07", "harness": "rc07_apply_then_untampered_undo_restores_full_pre_state", "target_failure": "RC07 apply plus undo restores the whole pre-state"},
    {"id": "RC08", "harness": "rc08_accepted_transition_conserves_reserve_arithmetic", "target_failure": "RC08 accepted transition must conserve the bounded reserve arithmetic"},
    {"id": "RC09", "harness": "rc09_invalid_arithmetic_and_endpoints_reject", "target_failure": "RC09 constructor rejection class/order must match the independent i128 oracle"},
    {"id": "RC10", "harness": "rc10_occupied_output_and_tampered_undo_reject_unchanged", "target_failure": "RC10 tampered undo must reject"},
)


def parse_control(output, returncode, control):
    """RED-stage parser: intentionally incomplete until fail-closed tests prove the gap."""
    marker = "Checking harness " + control["harness"] + "..."
    require(marker in output, "missing target harness")
    return {"id": control["id"], "failures": []}
