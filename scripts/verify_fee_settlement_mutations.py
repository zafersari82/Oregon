#!/usr/bin/env python3
"""Kill the required inactive Stage 3B fee-settlement/reserve security mutations."""

from dataclasses import dataclass
import hashlib
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
FEES = Path("crates/oregon-execution/src/fees.rs")
COINBASE = Path("crates/oregon-consensus/src/coinbase.rs")
RESERVE = Path("crates/oregon-utxo/src/reserve.rs")
FEE_STATE = Path("crates/oregon-contract-state/src/fee_state.rs")


@dataclass(frozen=True)
class Mutation:
    name: str
    path: Path
    replacements: tuple[tuple[str, str], ...]
    command: tuple[str, ...]
    expected_test: str


def run(command: list[str] | tuple[str, ...], cwd: Path = ROOT) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        list(command),
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )


def cargo_integration(crate: str, target: str, test_name: str) -> tuple[str, ...]:
    return (
        "cargo", "+1.85.0", "test", "--locked", "-p", crate,
        "--test", target, test_name, "--", "--exact",
    )


def cargo_lib(crate: str, test_name: str) -> tuple[str, ...]:
    return (
        "cargo", "+1.85.0", "test", "--locked", "-p", crate,
        "--lib", test_name, "--", "--exact",
    )


BASELINE = [
    ("independent oracle", ("python3", "scripts/generate_fee_settlement_vectors.py", "--check")),
    (
        "primitive vectors",
        (
            "cargo", "+1.85.0", "test", "--locked", "-p", "oregon-primitives",
            "--test", "fee_settlement", "--test", "execution_reserve",
            "--test", "fee_settlement_vectors",
        ),
    ),
    (
        "execution fee vectors",
        (
            "cargo", "+1.85.0", "test", "--locked", "-p", "oregon-execution",
            "--test", "fees", "--test", "fee_vectors",
        ),
    ),
    (
        "producer vectors",
        (
            "cargo", "+1.85.0", "test", "--locked", "-p", "oregon-consensus",
            "--test", "execution_fee_coinbase", "--test", "execution_fee_vectors",
        ),
    ),
    (
        "fee state",
        ("cargo", "+1.85.0", "test", "--locked", "-p", "oregon-contract-state", "--test", "fee_state"),
    ),
    (
        "reserve unit tests",
        ("cargo", "+1.85.0", "test", "--locked", "-p", "oregon-utxo", "reserve::tests"),
    ),
    (
        "reserve vector consumer",
        ("cargo", "+1.85.0", "test", "--locked", "-p", "oregon-utxo", "--test", "reserve_vectors"),
    ),
]


MUTATIONS = [
    Mutation(
        "max_fee_below_base_fee_bypass",
        FEES,
        ((
            "if max_fee_per_weight < base_fee_per_weight {",
            "if false && max_fee_per_weight < base_fee_per_weight {",
        ),),
        cargo_integration("oregon-execution", "fees", "max_fee_below_base_fee_is_rejected"),
        "max_fee_below_base_fee_is_rejected",
    ),
    Mutation(
        "fee_product_unchecked",
        FEES,
        ((
            "let max_escrow_wide = u128::from(max_weight) * u128::from(max_fee_per_weight);",
            "let max_escrow_wide = u128::from(max_weight.wrapping_mul(max_fee_per_weight));",
        ),),
        cargo_integration("oregon-execution", "fees", "max_escrow_uses_wide_arithmetic_and_supply_bound"),
        "max_escrow_uses_wide_arithmetic_and_supply_bound",
    ),
    Mutation(
        "refund_off_by_one",
        FEES,
        ((
            """let refund_wide = u128::from(terms.max_escrow)
            .checked_sub(charged_wide)
            .ok_or(FeeError::ArithmeticOverflow)?;""",
            """let refund_wide = u128::from(terms.max_escrow)
            .checked_sub(charged_wide)
            .and_then(|refund| refund.checked_add(1))
            .ok_or(FeeError::ArithmeticOverflow)?;""",
        ),),
        cargo_integration("oregon-execution", "fees", "reverted_execution_still_charges_consumed_weight_once"),
        "reverted_execution_still_charges_consumed_weight_once",
    ),
    Mutation(
        "revert_fee_zeroed",
        FEES,
        ((
            "let charged = u64::try_from(charged_wide).map_err(|_| FeeError::ArithmeticOverflow)?;",
            """let charged = if outcome == ExecutionOutcome::Committed {
            u64::try_from(charged_wide).map_err(|_| FeeError::ArithmeticOverflow)?
        } else {
            0
        };""",
        ),),
        cargo_integration("oregon-execution", "fees", "reverted_execution_still_charges_consumed_weight_once"),
        "reverted_execution_still_charges_consumed_weight_once",
    ),
    Mutation(
        "duplicate_settlement_allowed",
        FEES,
        ((
            "if self.settled.contains(&escrow_id) {",
            "if false && self.settled.contains(&escrow_id) {",
        ),),
        cargo_integration("oregon-execution", "fees", "reverted_execution_still_charges_consumed_weight_once"),
        "reverted_execution_still_charges_consumed_weight_once",
    ),
    Mutation(
        "stale_capability_allowed",
        FEES,
        ((
            "if self.consumed_sources.contains(&source_id) {",
            "if false && self.consumed_sources.contains(&source_id) {",
        ),),
        cargo_integration("oregon-execution", "fees", "duplicate_capability_and_stale_source_sequence_fail_closed"),
        "duplicate_capability_and_stale_source_sequence_fail_closed",
    ),
    Mutation(
        "reserve_program_bypass",
        COINBASE,
        ((
            "|| final_output.locking_program.as_slice() == EXECUTION_RESERVE_LOCKING_PROGRAM_V1",
            "|| false && final_output.locking_program.as_slice() == EXECUTION_RESERVE_LOCKING_PROGRAM_V1",
        ),),
        cargo_integration("oregon-consensus", "execution_fee_coinbase", "reserve_program_cannot_receive_execution_fee_payout"),
        "reserve_program_cannot_receive_execution_fee_payout",
    ),
    Mutation(
        "reserve_underflow_allowed",
        RESERVE,
        ((
            """let after_withdrawal = after_deposit
            .checked_sub(parts.execution_withdrawal_total)
            .ok_or(ReserveTransitionError::ArithmeticUnderflow)?;""",
            """let after_withdrawal =
            after_deposit.saturating_sub(parts.execution_withdrawal_total);""",
        ),),
        cargo_lib("oregon-utxo", "reserve::tests::reserve_equation_rejects_underflow_overflow_and_total_mismatch"),
        "reserve::tests::reserve_equation_rejects_underflow_overflow_and_total_mismatch",
    ),
    Mutation(
        "reserve_backing_equality_bypass",
        RESERVE,
        ((
            "if new_reserve_amount != parts.new_execution_balance_total {",
            "if false && new_reserve_amount != parts.new_execution_balance_total {",
        ),),
        cargo_lib("oregon-utxo", "reserve::tests::reserve_equation_rejects_underflow_overflow_and_total_mismatch"),
        "reserve::tests::reserve_equation_rejects_underflow_overflow_and_total_mismatch",
    ),
    Mutation(
        "fee_accumulator_double_count",
        COINBASE,
        ((
            """let total_fee_units = native_fees
        .base_units()
        .checked_add(execution_fees.base_units())
        .ok_or(ConsensusError::ArithmeticOverflow)?;""",
            """let total_fee_units = native_fees
        .base_units()
        .checked_add(execution_fees.base_units())
        .and_then(|fees| fees.checked_add(execution_fees.base_units()))
        .ok_or(ConsensusError::ArithmeticOverflow)?;""",
        ),),
        cargo_integration("oregon-consensus", "execution_fee_vectors", "independent_producer_vectors_pin_execution_fee_boundaries"),
        "independent_producer_vectors_pin_execution_fee_boundaries",
    ),
    Mutation(
        "producer_payout_omitted",
        COINBASE,
        ((
            "if execution_fees.base_units() != 0 {",
            "if false && execution_fees.base_units() != 0 {",
        ),),
        cargo_integration("oregon-consensus", "execution_fee_coinbase", "nonzero_execution_fees_require_an_exact_final_dedicated_output"),
        "nonzero_execution_fees_require_an_exact_final_dedicated_output",
    ),
    Mutation(
        "fee_state_domain_substitution",
        FEE_STATE,
        ((
            """StateWriteSet::new(
            CommitmentDomainId::FeeState,""",
            """StateWriteSet::new(
            CommitmentDomainId::ExecutionAccounting,""",
        ),),
        cargo_integration("oregon-contract-state", "fee_state", "identical_fee_state_writes_are_deterministic_and_domain_separated"),
        "identical_fee_state_writes_are_deterministic_and_domain_separated",
    ),
    Mutation(
        "transition_root_cycle_regression",
        RESERVE,
        ((
            """bytes.extend_from_slice(parts.producer_coinbase_txid.as_bytes());
    bytes""",
            """bytes.extend_from_slice(parts.producer_coinbase_txid.as_bytes());
    bytes.extend_from_slice(&[0u8; 32]);
    bytes""",
        ),),
        cargo_lib("oregon-utxo", "reserve::tests::transition_preimage_and_new_outpoint_are_exact"),
        "reserve::tests::transition_preimage_and_new_outpoint_are_exact",
    ),
    Mutation(
        "reserve_undo_mismatch",
        RESERVE,
        (
            (
                """if live.len() != 1
                || live[0].0 != *expected_outpoint
                || live[0].1 != *expected_entry
                || state.get(expected_outpoint) != Some(expected_entry)
            {""",
                """if live.len() != 1
                || live[0].0 != *expected_outpoint
                || state.get(expected_outpoint).is_none()
            {""",
            ),
            (
                """if overlay.reserve_remove_entry(outpoint).as_ref() != Some(entry) {
            return Err(ReserveTransitionError::UndoMismatch);
        }""",
                """if overlay.reserve_remove_entry(outpoint).is_none() {
            return Err(ReserveTransitionError::UndoMismatch);
        }""",
            ),
        ),
        cargo_lib("oregon-utxo", "reserve::tests::undo_rejects_tampered_forward_state"),
        "reserve::tests::undo_rejects_tampered_forward_state",
    ),
]


def require_clean() -> None:
    result = run(["git", "status", "--porcelain", "--untracked-files=all"])
    if result.returncode != 0 or result.stdout.strip():
        raise SystemExit("Stage 3B mutation gate requires a clean checkout")


def require_baseline(cwd: Path = ROOT) -> None:
    for name, command in BASELINE:
        result = run(command, cwd)
        if result.returncode != 0:
            print(result.stdout)
            raise SystemExit(f"clean Stage 3B baseline failed: {name}")


def mutate_once(disposable: Path, mutation: Mutation) -> None:
    source = disposable / mutation.path
    original = source.read_bytes()
    original_hash = hashlib.sha256(original).digest()
    text = original.decode()

    for old, new in mutation.replacements:
        matches = text.count(old)
        if matches != 1:
            raise SystemExit(
                f"mutation site count={matches} for {mutation.name}: {mutation.path}"
            )
        text = text.replace(old, new, 1)

    source.write_text(text)
    try:
        result = run(mutation.command, disposable)
        expected_failure = (
            result.returncode == 101
            and f"test {mutation.expected_test} ... FAILED" in result.stdout
            and "could not compile" not in result.stdout
        )
        if not expected_failure:
            print(result.stdout)
            raise SystemExit(f"mutant survived or failed incorrectly: {mutation.name}")
        print(f"KILLED: {mutation.name} [{mutation.expected_test}]")
    finally:
        source.write_bytes(original)

    if hashlib.sha256(source.read_bytes()).digest() != original_hash:
        raise SystemExit(f"source restoration failed: {mutation.name}")

    clean = run(mutation.command, disposable)
    if clean.returncode != 0:
        print(clean.stdout)
        raise SystemExit(f"restored focused test failed: {mutation.name}")


def main() -> None:
    require_clean()
    require_baseline()

    killed = 0
    with tempfile.TemporaryDirectory(prefix="oregon-fee-mutants-") as directory:
        disposable = Path(directory) / "repo"
        shutil.copytree(ROOT, disposable, ignore=shutil.ignore_patterns(".git", "target"))
        for mutation in MUTATIONS:
            mutate_once(disposable, mutation)
            killed += 1

    require_baseline()
    require_clean()
    print(f"Stage 3B fee-settlement mutations: {killed}/{len(MUTATIONS)} killed")


if __name__ == "__main__":
    main()
