"""Fail-closed semantic negative-control gate for Reserve Conservation Proof V1."""

import argparse
import json
from pathlib import Path
import re
import tempfile

from verify_reserve_proofs import (
    GateError,
    PROOF,
    ROOT,
    checked,
    digest,
    preflight,
    proof_environment,
    require,
    run,
)

MODEL_RELATIVE = Path("verification/reserve-conservation/src/model.rs")
ARITHMETIC_SOURCE = "verification/reserve-conservation/arithmetic.rs"
STATE_SOURCE = "verification/reserve-conservation/state.rs"

CONTROLS = (
    {
        "id": "RC01",
        "source": ARITHMETIC_SOURCE,
        "harness": "rc01_accepted_arithmetic_matches_mathematical_equation",
        "target_failure": "RC01 accepted arithmetic must equal the independent mathematical equation",
        "allowed_failures": ("RC01 accepted arithmetic must equal the independent mathematical equation",),
        "positive_covers": 4,
        "mutation": "replace checked addition with wrapping addition",
        "old": """    let after_deposit = previous
        .checked_add(input.native_deposit_total)
        .ok_or(ArithmeticError::ArithmeticOverflow)?;
""",
        "new": """    let after_deposit = previous.wrapping_add(input.native_deposit_total);
""",
    },
    {
        "id": "RC02",
        "source": ARITHMETIC_SOURCE,
        "harness": "rc02_accepted_result_equals_claimed_execution_total",
        "target_failure": "RC02 accepted result must equal the caller-supplied execution total",
        "allowed_failures": ("RC02 accepted result must equal the caller-supplied execution total",),
        "positive_covers": 1,
        "mutation": "remove execution-balance equality rejection",
        "old": """    if result != input.new_execution_balance_total {
        return Err(ArithmeticError::ExecutionBalanceMismatch);
    }

""",
        "new": "",
    },
    {
        "id": "RC03",
        "source": STATE_SOURCE,
        "harness": "rc03_successful_result_has_exact_reserve_cardinality",
        "target_failure": "RC03 zero result has no reserve and positive result has exactly one",
        "allowed_failures": ("RC03 zero result has no reserve and positive result has exactly one",),
        "positive_covers": 2,
        "mutation": "create a reserve entry even when the resulting amount is zero",
        "old": """    let created = if transition.new_amount == 0 {
        None
    } else {
        let new_key = transition
            .new_key
            .ok_or(StateError::PreviousReserveMismatch)?;
        Some(ModelSlot {
            key: new_key,
            entry: ModelEntry {
                amount: transition.new_amount,
                creation_height: transition.height,
                is_coinbase: false,
                program: ProgramClass::Reserve,
            },
        })
    };
""",
        "new": """    let created = {
        let new_key = transition.new_key.unwrap_or(0);
        Some(ModelSlot {
            key: new_key,
            entry: ModelEntry {
                amount: transition.new_amount,
                creation_height: transition.height,
                is_coinbase: false,
                program: ProgramClass::Reserve,
            },
        })
    };
""",
    },
    {
        "id": "RC04",
        "source": STATE_SOURCE,
        "harness": "rc04_two_live_reserves_reject_unchanged",
        "target_failure": "RC04 two reserves must reject",
        "allowed_failures": (
            "RC04 two reserves must reject",
            "RC04 rejection must leave every slot unchanged",
        ),
        "positive_covers": 0,
        "mutation": "remove multiple-live-reserve rejection",
        "old": """    if reserve_indices.iter().flatten().count() > 1 {
        return Err(StateError::MultipleLiveReserves);
    }

""",
        "new": "",
    },
    {
        "id": "RC05",
        "source": STATE_SOURCE,
        "harness": "rc05_successful_creation_has_exact_program_value_and_metadata",
        "target_failure": "RC05 created reserve must have exact modeled program, value and metadata",
        "allowed_failures": (
            "RC05 created reserve must have exact modeled program, value and metadata",
            "RC05 creation leaves exactly one reserve",
        ),
        "positive_covers": 1,
        "mutation": "create the reserve with ordinary program classification",
        "old": """                is_coinbase: false,
                program: ProgramClass::Reserve,
""",
        "new": """                is_coinbase: false,
                program: ProgramClass::Ordinary,
""",
    },
    {
        "id": "RC06",
        "source": STATE_SOURCE,
        "harness": "rc06_failed_apply_and_failed_undo_are_atomic",
        "target_failure": "RC06 failed apply must leave every slot unchanged",
        "allowed_failures": ("RC06 failed apply must leave every slot unchanged",),
        "positive_covers": 2,
        "mutation": "remove the old reserve before collision validation",
        "old": """    if let Some(new_key) = transition.new_key {
        if state.slots.iter().flatten().any(|slot| slot.key == new_key) {
            return Err(StateError::OutputCollision);
        }
    }

""",
        "new": """    if let Some(index) = previous_index {
        state.slots[index] = None;
    }

    if let Some(new_key) = transition.new_key {
        if state.slots.iter().flatten().any(|slot| slot.key == new_key) {
            return Err(StateError::OutputCollision);
        }
    }

""",
    },
    {
        "id": "RC07",
        "source": STATE_SOURCE,
        "harness": "rc07_apply_then_untampered_undo_restores_full_pre_state",
        "target_failure": "RC07 apply plus undo restores the whole pre-state",
        "allowed_failures": ("RC07 apply plus undo restores the whole pre-state",),
        "positive_covers": 1,
        "mutation": "omit previous-entry restoration during undo",
        "old": """    if let Some(previous) = undo.previous {
        let Some(index) = overlay.slots.iter().position(Option::is_none) else {
            return Err(StateError::UndoMismatch);
        };
        overlay.slots[index] = Some(previous);
    }

""",
        "new": "",
    },
    {
        "id": "RC08",
        "source": ARITHMETIC_SOURCE,
        "harness": "rc08_accepted_transition_conserves_reserve_arithmetic",
        "target_failure": "RC08 accepted transition must conserve the bounded reserve arithmetic",
        "allowed_failures": ("RC08 accepted transition must conserve the bounded reserve arithmetic",),
        "positive_covers": 1,
        "mutation": "omit execution-fee subtraction",
        "old": """    let result = after_withdrawal
        .checked_sub(input.execution_fee_total)
        .ok_or(ArithmeticError::ArithmeticUnderflow)?;
""",
        "new": """    let result = after_withdrawal;
""",
    },
    {
        "id": "RC09",
        "source": ARITHMETIC_SOURCE,
        "harness": "rc09_invalid_arithmetic_and_endpoints_reject",
        "target_failure": "RC09 constructor rejection class/order must match the independent i128 oracle",
        "allowed_failures": ("RC09 constructor rejection class/order must match the independent i128 oracle",),
        "positive_covers": 4,
        "mutation": "replace checked subtraction with saturation",
        "old": """    let after_withdrawal = after_deposit
        .checked_sub(input.execution_withdrawal_total)
        .ok_or(ArithmeticError::ArithmeticUnderflow)?;
    let result = after_withdrawal
        .checked_sub(input.execution_fee_total)
        .ok_or(ArithmeticError::ArithmeticUnderflow)?;
""",
        "new": """    let after_withdrawal = after_deposit.saturating_sub(input.execution_withdrawal_total);
    let result = after_withdrawal.saturating_sub(input.execution_fee_total);
""",
    },
    {
        "id": "RC10",
        "source": STATE_SOURCE,
        "harness": "rc10_occupied_output_and_tampered_undo_reject_unchanged",
        "target_failure": "RC10 tampered undo must reject",
        "allowed_failures": (
            "RC10 tampered undo must reject",
            "RC10 tampered undo rejection is atomic",
        ),
        "positive_covers": 2,
        "mutation": "skip created-entry equality validation during undo",
        "old": """            if actual != expected {
                return Err(StateError::UndoMismatch);
            }
""",
        "new": "",
    },
)

PROPERTY_PATTERN = re.compile(
    r"Check (\d+): ([^\n]+)\n\s+- Status: ([A-Z]+)\n"
    r"\s+- Description: ([^\n]+)\n\s+- Location: ([^\n]+)\n"
)
INFRA_PATTERN = re.compile(r"(?im)^.*(?:error:|error\[|unsupported operation|timed out).*$")


def _property_inventory(output):
    require(output.count("RESULTS:\n") == 1, "missing or repeated RESULTS marker")
    require(output.count("SUMMARY:\n") == 1, "missing or repeated SUMMARY marker")
    section = output.split("RESULTS:\n", 1)[1].split("SUMMARY:\n", 1)[0]
    matches = list(PROPERTY_PATTERN.finditer(section))
    require(matches, "empty property inventory")
    require(not PROPERTY_PATTERN.sub("", section).strip(), "unparsed property output")
    require(
        re.findall(r"^Check [^\n]*$", output, re.M)
        == re.findall(r"^Check [^\n]*$", section, re.M),
        "property output outside RESULTS section",
    )
    return [
        {
            "number": int(match[1]),
            "property": match[2],
            "status": match[3],
            "description": match[4].strip('"'),
            "location": match[5],
        }
        for match in matches
    ]


def _common_output_checks(output, returncode, harness, expected_returncode):
    require(returncode == expected_returncode, "unexpected verifier exit status")
    require(not INFRA_PATTERN.search(output), "infrastructure or unsupported-operation diagnostic")
    for marker in (
        "Kani Rust Verifier 0.67.0 (standalone)",
        "Checking harness " + harness + "...",
        "RESULTS:",
        "SUMMARY:",
        "Manual Harness Summary:",
    ):
        require(output.count(marker) == 1, "missing or repeated output marker: " + marker)
    require(
        re.findall(r"^Checking harness (.+)\.\.\.$", output, re.M) == [harness],
        "unexpected harness inventory",
    )
    require(
        re.findall(r"^CBMC[^\n]*$", output, re.M)
        == [
            "CBMC 6.8.0 (cbmc-6.8.0)",
            "CBMC version 6.8.0 (cbmc-6.8.0) 64-bit x86_64 linux",
        ],
        "wrong or repeated backend inventory",
    )
    solvers = re.findall(r"^Solving with (.+)$", output, re.M)
    require(solvers and set(solvers) == {"CaDiCaL 2.0.0"}, "wrong or absent solver")


def parse_control(output, returncode, control):
    """Accept a killed control only for the intended semantic failure plus counterexample."""
    _common_output_checks(output, returncode, control["harness"], 1)
    properties = _property_inventory(output)
    require(
        all(item["status"] in {"SUCCESS", "SATISFIED", "FAILURE"} for item in properties),
        "unknown, unsatisfied, or unsupported property status",
    )
    failures = [item for item in properties if item["status"] == "FAILURE"]
    require(failures, "negative control did not produce a semantic property failure")
    require(
        all(item["property"].startswith(control["harness"] + ".") for item in failures),
        "failure belongs to a different function or harness",
    )
    descriptions = [item["description"] for item in failures]
    require(control["target_failure"] in descriptions, "target semantic assertion did not fail")
    require(
        set(descriptions).issubset(set(control["allowed_failures"])),
        "unrelated assertion failure in control",
    )
    summaries = re.findall(r"^ \*\* (\d+) of (\d+) failed$", output, re.M)
    require(len(summaries) == 1 and int(summaries[0][0]) == len(failures), "inconsistent failure summary")
    require(re.findall(r"^VERIFICATION:- (.+)$", output, re.M) == ["FAILED"], "wrong control verdict")
    completion = "Complete - 0 successfully verified harnesses, 1 failures, 1 total."
    require(re.findall(r"^Complete - [^\n]*$", output, re.M) == [completion], "unexpected completion inventory")
    require(output.rstrip().endswith(completion), "incomplete control output")
    require(output.count("let concrete_vals: Vec<Vec<u8>> = vec![") == 1, "missing or repeated concrete playback")
    playback_body = output.split("let concrete_vals: Vec<Vec<u8>> = vec![", 1)[1].split("];", 1)[0]
    require("vec![" in playback_body, "empty concrete counterexample")
    bindings = re.findall(r"kani::concrete_playback_run\(concrete_vals,\s*(\w+)\);", output)
    require(bindings == [control["harness"]], "counterexample is not bound to target harness")
    return {
        "id": control["id"],
        "harness": control["harness"],
        "failures": [
            {"property": item["property"], "description": item["description"]} for item in failures
        ],
        "counterexample_bound": True,
    }


def parse_positive(output, returncode, control):
    """Fail closed when the post-control baseline rerun is not a complete green harness."""
    _common_output_checks(output, returncode, control["harness"], 0)
    properties = _property_inventory(output)
    require(
        all(item["status"] in {"SUCCESS", "SATISFIED"} for item in properties),
        "positive rerun contains a failed, unknown, or unsatisfied property",
    )
    require(
        control["target_failure"] in [item["description"] for item in properties],
        "target semantic assertion missing from positive rerun",
    )
    covers = [item for item in properties if item["status"] == "SATISFIED" and ".cover." in item["property"]]
    require(len(covers) == control["positive_covers"], "unexpected positive reachability inventory")
    summaries = re.findall(r"^ \*\* (\d+) of (\d+) failed$", output, re.M)
    require(len(summaries) == 1 and int(summaries[0][0]) == 0, "positive assertion summary is not green")
    require(re.findall(r"^VERIFICATION:- (.+)$", output, re.M) == ["SUCCESSFUL"], "wrong positive verdict")
    completion = "Complete - 1 successfully verified harnesses, 0 failures, 1 total."
    require(re.findall(r"^Complete - [^\n]*$", output, re.M) == [completion], "unexpected positive completion inventory")
    require(output.rstrip().endswith(completion), "incomplete positive output")
    return {
        "id": control["id"],
        "harness": control["harness"],
        "properties": len(properties),
        "covers": len(covers),
    }


def apply_mutation(model_text, control):
    """Apply one exact, reviewable model weakening and reject source drift."""
    count = model_text.count(control["old"])
    require(count == 1, f"{control['id']} mutation anchor count is {count}, expected exactly one")
    mutated = model_text.replace(control["old"], control["new"], 1)
    require(mutated != model_text, f"{control['id']} mutation made no change")
    return mutated


def validate_control_manifest(model_text):
    require([control["id"] for control in CONTROLS] == [f"RC{i:02d}" for i in range(1, 11)], "control ID inventory drift")
    require(len({control["harness"] for control in CONTROLS}) == 10, "control harnesses are not unique")
    for control in CONTROLS:
        require(control["source"] in {ARITHMETIC_SOURCE, STATE_SOURCE}, "unknown proof source")
        require(control["target_failure"] in control["allowed_failures"], "target failure not allowed")
        apply_mutation(model_text, control)


def _clean_root():
    require(not checked(["git", "status", "--porcelain"], ROOT), "baseline checkout is dirty")
    checked(["git", "diff", "--exit-code"], ROOT)


def _run_control(control, kani_driver, env, output_dir, model_text):
    with tempfile.TemporaryDirectory(prefix=f"oregon-{control['id'].lower()}-") as directory:
        worktree = Path(directory) / "checkout"
        checked(["git", "worktree", "add", "--detach", str(worktree), "HEAD"], ROOT, timeout=60)
        try:
            model_path = worktree / MODEL_RELATIVE
            model_path.write_text(apply_mutation(model_text, control))
            status = checked(["git", "status", "--porcelain"], worktree)
            require(status == "M " + str(MODEL_RELATIVE), f"{control['id']} changed unexpected paths: {status!r}")
            command = [
                str(kani_driver),
                str(worktree / control["source"]),
                "--harness",
                control["harness"],
                "-Z",
                "concrete-playback",
                "--concrete-playback=print",
            ]
            result = run(command, worktree, 180, env=env)
            log_path = output_dir / f"{control['id'].lower()}-control.log"
            log_path.write_text(result["output"])
            require(not result["timed_out"], f"{control['id']} verifier timed out")
            parsed = parse_control(result["output"], result["returncode"], control)
            parsed.update(
                {
                    "mutation": control["mutation"],
                    "mutated_model_sha256": digest(model_path),
                    "command": result["command"],
                    "log": log_path.name,
                }
            )
            return parsed
        finally:
            removal = run(["git", "worktree", "remove", "--force", str(worktree)], ROOT, 60)
            require(
                not removal["timed_out"] and removal["returncode"] == 0,
                f"failed to remove {control['id']} disposable worktree: {removal['output']}",
            )
            checked(["git", "worktree", "prune"], ROOT)
            _clean_root()


def _positive_rerun(control, kani_driver, env, output_dir):
    command = [
        str(kani_driver),
        str(ROOT / control["source"]),
        "--harness",
        control["harness"],
    ]
    result = run(command, ROOT, 180, env=env)
    log_path = output_dir / f"{control['id'].lower()}-positive-rerun.log"
    log_path.write_text(result["output"])
    require(not result["timed_out"], f"{control['id']} positive rerun timed out")
    parsed = parse_positive(result["output"], result["returncode"], control)
    parsed.update({"command": result["command"], "log": log_path.name})
    return parsed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--kani-home", required=True, type=Path)
    parser.add_argument("--archive", required=True, type=Path)
    parser.add_argument("--rustc", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path, help="new evidence directory")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    evidence = {
        "schema_version": 1,
        "scope": "reserve-conservation-rc01-rc10-semantic-controls",
        "accepted": False,
        "controls": [],
        "positive_rerun": [],
    }
    try:
        evidence["source_sha"] = checked(["git", "rev-parse", "HEAD"], ROOT)
        evidence["source_tree"] = checked(["git", "rev-parse", "HEAD^{tree}"], ROOT)
        _clean_root()
        kani_home, archive, rustc = (path.resolve() for path in (args.kani_home, args.archive, args.rustc))
        evidence["tool_lock"] = preflight(kani_home, archive, rustc)
        model_path = ROOT / MODEL_RELATIVE
        model_text = model_path.read_text()
        validate_control_manifest(model_text)
        evidence["source_digests"] = {
            str(path.relative_to(ROOT)): digest(path)
            for path in (
                model_path,
                ROOT / ARITHMETIC_SOURCE,
                ROOT / STATE_SOURCE,
                ROOT / "crates/oregon-utxo/src/reserve.rs",
                ROOT / "crates/oregon-primitives/src/amount.rs",
                ROOT / "crates/oregon-primitives/src/execution_reserve.rs",
                PROOF / "toolchain-lock.json",
                Path(__file__).resolve(),
                ROOT / ".github/workflows/oregon-reserve-arithmetic-proofs.yml",
            )
        }
        env = proof_environment(kani_home, evidence["tool_lock"]["rust_toolchain"])
        kani_driver = kani_home / "bin/kani-driver"
        for control in CONTROLS:
            evidence["controls"].append(
                _run_control(control, kani_driver, env, args.output, model_text)
            )
        _clean_root()
        for control in CONTROLS:
            evidence["positive_rerun"].append(
                _positive_rerun(control, kani_driver, env, args.output)
            )
        _clean_root()
        evidence["accepted"] = True
    except (GateError, OSError, ValueError) as error:
        evidence["error"] = str(error)
    (args.output / "summary.json").write_text(json.dumps(evidence, indent=2) + "\n")
    print(
        json.dumps(
            {
                "accepted": evidence["accepted"],
                "scope": evidence["scope"],
                "controls": len(evidence["controls"]),
                "positive_rerun": len(evidence["positive_rerun"]),
                "error": evidence.get("error"),
            }
        )
    )
    return 0 if evidence["accepted"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
