#!/usr/bin/env python3
"""Kill the required Stage 4B runtime/coordinator security mutations."""

from dataclasses import dataclass
from hashlib import sha256
from pathlib import Path
import json
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]

WEIGHT = ROOT / "crates/oregon-execution/src/weight.rs"
CALLS = ROOT / "crates/oregon-execution/src/coordinator/calls.rs"
HOST = ROOT / "crates/oregon-execution/src/coordinator/host.rs"
EFFECTS = ROOT / "crates/oregon-execution/src/coordinator/effects.rs"
SETTLEMENT = ROOT / "crates/oregon-execution/src/coordinator/settlement.rs"
PROPOSAL = ROOT / "crates/oregon-execution/src/coordinator/proposal.rs"
EXECUTION_EFFECT = ROOT / "crates/oregon-primitives/src/execution_effect.rs"
EXECUTION_RECEIPT = ROOT / "crates/oregon-primitives/src/execution_receipt.rs"
EXECUTION_EVENT = ROOT / "crates/oregon-primitives/src/execution_event.rs"
RUNTIME_TYPES = ROOT / "crates/oregon-runtime/src/types.rs"
VECTOR_CORPUS = ROOT / "tests/vectors/runtime-coordinator-v1.json"


@dataclass(frozen=True)
class Edit:
    path: Path
    old: str
    new: str
    count: int = 1


@dataclass(frozen=True)
class TestSpec:
    command: tuple[str, ...]
    expected_failure: str


@dataclass(frozen=True)
class Mutation:
    name: str
    edits: tuple[Edit, ...]
    tests: tuple[TestSpec, ...]


def run(command, cwd=ROOT):
    return subprocess.run(
        list(command),
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )


def unit_test(name):
    return TestSpec(
        (
            "cargo", "+1.85.0", "test", "--locked", "-p", "oregon-execution",
            name, "--", "--exact",
        ),
        f"test {name} ... FAILED",
    )


def integration_test(package, target, name):
    return TestSpec(
        (
            "cargo", "+1.85.0", "test", "--locked", "-p", package,
            "--test", target, name, "--", "--exact",
        ),
        f"test {name} ... FAILED",
    )


def require_clean():
    result = run(("git", "status", "--porcelain", "--untracked-files=all"))
    if result.returncode != 0 or result.stdout.strip():
        raise SystemExit("runtime coordinator mutation gate requires a clean disposable checkout")


def require_success(command, message, cwd=ROOT):
    result = run(command, cwd)
    if result.returncode != 0:
        print(result.stdout)
        raise SystemExit(message)


def require_baseline(cwd=ROOT):
    require_success(
        ("python3", "scripts/generate_runtime_coordinator_vectors.py", "--check"),
        "clean Stage 4B independent-vector baseline failed",
        cwd,
    )
    require_success(
        (
            "cargo", "+1.85.0", "test", "--locked", "-p", "oregon-primitives",
            "--test", "runtime_receipts", "--test", "runtime_receipt_vectors",
        ),
        "clean Stage 4B primitive receipt baseline failed",
        cwd,
    )
    require_success(
        ("cargo", "+1.85.0", "test", "--locked", "-p", "oregon-runtime", "--all-targets"),
        "clean Stage 4B runtime ABI baseline failed",
        cwd,
    )
    require_success(
        ("cargo", "+1.85.0", "test", "--locked", "-p", "oregon-execution", "--all-targets"),
        "clean Stage 4B coordinator baseline failed",
        cwd,
    )


def require_vector_negative_control():
    with tempfile.TemporaryDirectory(prefix="oregon-runtime-vector-negative-") as directory:
        disposable = Path(directory) / "repo"
        shutil.copytree(
            ROOT,
            disposable,
            ignore=shutil.ignore_patterns("target", ".git", ".worktrees"),
        )
        corpus = disposable / VECTOR_CORPUS.relative_to(ROOT)
        original = corpus.read_text()
        original_hash = sha256(corpus.read_bytes()).hexdigest()
        document = json.loads(original)
        old = document["receipt_cases"][0]["receipt_hex"]
        replacement = ("0" if old[0] != "0" else "1") + old[1:]
        document["receipt_cases"][0]["receipt_hex"] = replacement
        corpus.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")

        try:
            generator = run(
                ("python3", "scripts/generate_runtime_coordinator_vectors.py", "--check"),
                disposable,
            )
            if generator.returncode == 0:
                raise SystemExit("independent runtime coordinator oracle accepted a changed receipt byte")

            result = run(
                (
                    "cargo", "+1.85.0", "test", "--locked", "-p", "oregon-primitives",
                    "--test", "runtime_receipt_vectors",
                ),
                disposable,
            )
            expected = (
                result.returncode != 0
                and "FAILED" in result.stdout
                and "could not compile" not in result.stdout
                and "error[E" not in result.stdout
            )
            if not expected:
                print(result.stdout)
                raise SystemExit("Rust runtime receipt vector consumer accepted a changed receipt byte")
            print("REJECTED: changed runtime receipt byte [oracle + Rust consumer]")
        finally:
            corpus.write_text(original)

        if sha256(corpus.read_bytes()).hexdigest() != original_hash:
            raise SystemExit("runtime coordinator vector negative-control restoration failed")


MUTATIONS = (
    Mutation(
        "rollback_refunds_meter",
        (
            Edit(
                WEIGHT,
                "#[derive(Debug, PartialEq, Eq)]\npub struct WeightMeter {",
                "#[derive(Debug, Clone, PartialEq, Eq)]\npub struct WeightMeter {",
            ),
            Edit(
                CALLS,
                "    begin_child_frames(journal, effects, terminal)?;\n\n    match transfer_attached_value(",
                "    let meter_before_child = meter.clone();\n"
                "    begin_child_frames(journal, effects, terminal)?;\n\n"
                "    match transfer_attached_value(",
            ),
            Edit(
                CALLS,
                """        RuntimeCallResultV1::Revert(_) => {
            rollback_child_frames(journal, effects, terminal)?;
            if return_bytes != 0 && effects.retain_return_bytes(return_bytes).is_err() {
""",
                """        RuntimeCallResultV1::Revert(_) => {
            rollback_child_frames(journal, effects, terminal)?;
            *meter = meter_before_child.clone();
            if return_bytes != 0 && effects.retain_return_bytes(return_bytes).is_err() {
""",
            ),
            Edit(
                CALLS,
                """        RuntimeCallResultV1::Trap(_) => {
            rollback_child_frames(journal, effects, terminal)?;
        }
""",
                """        RuntimeCallResultV1::Trap(_) => {
            rollback_child_frames(journal, effects, terminal)?;
            *meter = meter_before_child;
        }
""",
            ),
        ),
        (
            unit_test(
                "coordinator::test_backends::child_revert_and_trap_are_catchable_but_ancestor_revert_discards_effects"
            ),
        ),
    ),
    Mutation(
        "backend_chooses_cheaper_resource_domain",
        (
            Edit(
                HOST,
                "            ExecutionDomain::Evm => ResourceDomain::Evm,\n",
                "            ExecutionDomain::Evm => ResourceDomain::Wasm,\n",
            ),
        ),
        (unit_test("coordinator::tests::meter::backend_cannot_choose_a_cheaper_resource_domain"),),
    ),
    Mutation(
        "read_only_child_clears_parent_restriction",
        (
            Edit(
                CALLS,
                "        read_only: parent.read_only() || spec.read_only(),\n",
                "        read_only: spec.read_only(),\n",
            ),
        ),
        (
            unit_test(
                "coordinator::calls_tests::read_only_is_monotonic_across_nested_calls_and_blocks_writes"
            ),
        ),
    ),
    Mutation(
        "event_survives_reverted_frame",
        (
            Edit(
                EFFECTS,
                """        self.frames.pop();
        self.live_event_count = next_event_count;
        self.live_retained_bytes = next_retained_bytes;
""",
                """        let child = self
            .frames
            .pop()
            .ok_or(CoordinatorError::AccountingInvariant)?;
        let parent = self
            .frames
            .last_mut()
            .ok_or(CoordinatorError::AccountingInvariant)?;
        parent.events.extend(child.events);
        self.live_event_count = next_event_count;
        self.live_retained_bytes = next_retained_bytes;
""",
            ),
        ),
        (
            unit_test(
                "coordinator::test_backends::child_revert_and_trap_are_catchable_but_ancestor_revert_discards_effects"
            ),
        ),
    ),
    Mutation(
        "attached_value_survives_reverted_frame",
        (
            Edit(
                CALLS,
                """    let journal_result = journal.revert_frame();
    let effects_result = effects.revert();
""",
                """    let journal_result = journal.commit_frame();
    let effects_result = effects.revert();
""",
            ),
        ),
        (
            unit_test(
                "coordinator::calls_tests::nested_revert_and_trap_discard_child_value_and_events"
            ),
        ),
    ),
    Mutation(
        "escrow_reservation_remains_spendable",
        (
            Edit(
                SETTLEMENT,
                """    let available = current
        .checked_sub(max_escrow)
        .ok_or(CoordinatorError::InsufficientExecutionFunding)?;
    write_balance(journal, payer, available)
""",
                """    let _available = current
        .checked_sub(max_escrow)
        .ok_or(CoordinatorError::InsufficientExecutionFunding)?;
    write_balance(journal, payer, current)
""",
            ),
        ),
        (
            unit_test(
                "coordinator::escrow_tests::max_escrow_is_not_spendable_inside_execution_child"
            ),
        ),
    ),
    Mutation(
        "execution_charge_does_not_reduce_execution_total",
        (
            Edit(
                SETTLEMENT,
                "    write_total_execution_balance(journal, next_total_execution_balance)\n",
                "    write_total_execution_balance(journal, total_execution_balance)\n",
            ),
        ),
        (
            unit_test(
                "coordinator::tests::settlement::committed_reverted_and_trapped_equal_weight_charge_equally_and_map_frames"
            ),
        ),
    ),
    Mutation(
        "trap_maps_to_committed_fee_outcome",
        (
            Edit(
                SETTLEMENT,
                """            RuntimeCallResultV1::Trap(code) => (
                CoordinatorOutcomeV1::Trapped(code),
                ExecutionOutcome::Reverted,
                false,
                meter.consumed(),
            ),
""",
                """            RuntimeCallResultV1::Trap(code) => (
                CoordinatorOutcomeV1::Trapped(code),
                ExecutionOutcome::Committed,
                false,
                meter.consumed(),
            ),
""",
            ),
        ),
        (
            unit_test(
                "coordinator::tests::settlement::committed_reverted_and_trapped_equal_weight_charge_equally_and_map_frames"
            ),
        ),
    ),
    Mutation(
        "resource_exhaustion_masked_by_backend_success",
        (
            Edit(
                SETTLEMENT,
                "    let exhausted = *terminal == CoordinatorTerminalV1::ResourceExhausted || meter.is_exhausted();\n",
                "    let exhausted = false;\n",
            ),
        ),
        (
            unit_test(
                "coordinator::tests::settlement::resource_exhaustion_forces_full_weight_revert_and_cannot_be_masked_by_success"
            ),
        ),
    ),
    Mutation(
        "generic_system_domain_state_write_exposed",
        (
            Edit(
                HOST,
                "            Some(journal) => journal.put(CommitmentDomainId::Wasm, &scoped_key, value),\n",
                "            Some(journal) => {\n"
                "                journal.put(CommitmentDomainId::ExecutionAccounting, &scoped_key, value)\n"
                "            }\n",
            ),
        ),
        (
            unit_test(
                "coordinator::calls_tests::two_wasm_targets_using_the_same_local_key_do_not_collide"
            ),
        ),
    ),
    Mutation(
        "evm_effect_accepted_as_oregon_smt",
        (
            Edit(
                EXECUTION_EFFECT,
                "        CommitmentDomainId::Evm if scheme_id == CommitmentSchemeId::EvmCommitmentV1 => Ok(()),\n",
                "        CommitmentDomainId::Evm\n"
                "            if matches!(scheme_id, CommitmentSchemeId::EvmCommitmentV1 | CommitmentSchemeId::OregonSmtV1) =>\n"
                "        {\n"
                "            Ok(())\n"
                "        }\n",
            ),
        ),
        (
            integration_test(
                "oregon-primitives",
                "runtime_receipts",
                "evm_effect_cannot_masquerade_as_oregon_smt",
            ),
        ),
    ),
    Mutation(
        "receipt_state_root_inserted_into_phase_a",
        (
            Edit(
                PROPOSAL,
                """    if phase_a
        .roots
        .iter()
        .any(|roots| roots.domain == CommitmentDomainId::ExecutionReceipts)
    {
        return Err(CoordinatorError::PhaseAReceiptDomain);
    }
""",
                """    if false
        && phase_a
            .roots
            .iter()
            .any(|roots| roots.domain == CommitmentDomainId::ExecutionReceipts)
    {
        return Err(CoordinatorError::PhaseAReceiptDomain);
    }
""",
            ),
            Edit(
                EXECUTION_EFFECT,
                """        CommitmentDomainId::ExecutionReceipts => {
            Err(ExecutionReceiptError::ReceiptDomainInStateEffects)
        }
""",
                """        CommitmentDomainId::ExecutionReceipts
            if scheme_id == CommitmentSchemeId::OregonSmtV1 =>
        {
            Ok(())
        }
        CommitmentDomainId::ExecutionReceipts => {
            Err(ExecutionReceiptError::ReceiptDomainInStateEffects)
        }
""",
            ),
        ),
        (
            unit_test(
                "coordinator::tests::proposal::receipt_domain_in_phase_a_is_rejected_instead_of_self_committed"
            ),
        ),
    ),
    Mutation(
        "phase_a_result_escapes_after_phase_b_failure",
        (
            Edit(
                PROPOSAL,
                """    let existing_fee = phase_b
        .read(CommitmentDomainId::ExecutionReceipts, &fee_key)
        .map_err(CoordinatorError::ReceiptState)?;
    let existing_execution = phase_b
        .read(CommitmentDomainId::ExecutionReceipts, &execution_key)
        .map_err(CoordinatorError::ReceiptState)?;
""",
                """    let existing_fee = phase_b
        .read(CommitmentDomainId::ExecutionReceipts, &fee_key)
        .unwrap_or(None);
    let existing_execution = phase_b
        .read(CommitmentDomainId::ExecutionReceipts, &execution_key)
        .unwrap_or(None);
""",
            ),
            Edit(
                PROPOSAL,
                """    let phase_b = phase_b
        .finalize(JournalIntentV1::Committed)
        .map_err(CoordinatorError::ReceiptState)?;
""",
                """    let phase_b = match phase_b.finalize(JournalIntentV1::Committed) {
        Ok(result) => result,
        Err(_) => phase_a.clone(),
    };
""",
            ),
        ),
        (
            unit_test(
                "coordinator::tests::proposal::phase_b_corrupt_source_discards_the_local_phase_a_result"
            ),
        ),
    ),
    Mutation(
        "duplicate_receipt_finalization_succeeds",
        (
            Edit(
                PROPOSAL,
                "    if existing_fee.is_some() || existing_execution.is_some() {\n",
                "    if false && (existing_fee.is_some() || existing_execution.is_some()) {\n",
            ),
        ),
        (
            unit_test(
                "coordinator::tests::proposal::existing_fee_or_execution_receipt_key_is_fatal_duplicate_finalization"
            ),
        ),
    ),
    Mutation(
        "non_trapped_receipt_accepts_nonzero_trap_code",
        (
            Edit(
                EXECUTION_RECEIPT,
                """    let trapped = parts.outcome == ExecutionReceiptOutcomeV1::Trapped;
    if trapped != (parts.trap_code != 0) {
        return Err(ExecutionReceiptError::InvalidTrapCode);
    }
""",
                """    let trapped = parts.outcome == ExecutionReceiptOutcomeV1::Trapped;
    if trapped && parts.trap_code == 0 {
        return Err(ExecutionReceiptError::InvalidTrapCode);
    }
""",
            ),
        ),
        (
            integration_test(
                "oregon-primitives",
                "runtime_receipts",
                "non_trapped_receipt_requires_zero_trap_code",
            ),
        ),
    ),
    Mutation(
        "event_return_overflow_truncates",
        (
            Edit(
                EXECUTION_EVENT,
                """    pub fn new(
        emitter: ExecutionAddress,
        topics: Vec<Hash256>,
        data: Vec<u8>,
    ) -> Result<Self, ExecutionReceiptError> {
""",
                """    pub fn new(
        emitter: ExecutionAddress,
        topics: Vec<Hash256>,
        mut data: Vec<u8>,
    ) -> Result<Self, ExecutionReceiptError> {
""",
            ),
            Edit(
                EXECUTION_EVENT,
                """        if data.len() > MAX_EXECUTION_EVENT_DATA_BYTES_V1 {
            return Err(ExecutionReceiptError::EventDataTooLarge);
        }
""",
                """        if data.len() > MAX_EXECUTION_EVENT_DATA_BYTES_V1 {
            data.truncate(MAX_EXECUTION_EVENT_DATA_BYTES_V1);
        }
""",
            ),
            Edit(
                RUNTIME_TYPES,
                """    pub fn success(return_data: Vec<u8>) -> Result<Self, RuntimeAbiError> {
        let result = Self::Success(return_data);
        result.validate()?;
        Ok(result)
    }

    pub fn revert(return_data: Vec<u8>) -> Result<Self, RuntimeAbiError> {
        let result = Self::Revert(return_data);
        result.validate()?;
        Ok(result)
    }
""",
                """    pub fn success(mut return_data: Vec<u8>) -> Result<Self, RuntimeAbiError> {
        return_data.truncate(MAX_RUNTIME_RETURN_DATA_BYTES);
        Ok(Self::Success(return_data))
    }

    pub fn revert(mut return_data: Vec<u8>) -> Result<Self, RuntimeAbiError> {
        return_data.truncate(MAX_RUNTIME_RETURN_DATA_BYTES);
        Ok(Self::Revert(return_data))
    }
""",
            ),
        ),
        (
            integration_test(
                "oregon-primitives",
                "runtime_receipts",
                "event_one_over_limits_are_rejected",
            ),
            integration_test(
                "oregon-runtime",
                "runtime_abi",
                "return_data_exact_limit_is_accepted_and_one_over_is_rejected",
            ),
        ),
    ),
)


def disposable_path(source_path: Path, disposable: Path) -> Path:
    return disposable / source_path.relative_to(ROOT)


def apply_edit(edit: Edit, disposable: Path, originals: dict[Path, str]):
    path = disposable_path(edit.path, disposable)
    if path not in originals:
        originals[path] = path.read_text()
    text = path.read_text()
    found = text.count(edit.old)
    if found != edit.count:
        raise SystemExit(
            f"{edit.path.relative_to(ROOT)} mutation anchor count {found}, expected {edit.count}"
        )
    path.write_text(text.replace(edit.old, edit.new, edit.count))


def test_failed_as_expected(spec: TestSpec, cwd: Path):
    result = run(spec.command, cwd)
    compile_failure = (
        "could not compile" in result.stdout
        or "error[E" in result.stdout
        or "error: could not compile" in result.stdout
    )
    expected_failure = result.returncode != 0 and spec.expected_failure in result.stdout
    if compile_failure or not expected_failure:
        print(result.stdout)
        if compile_failure:
            raise SystemExit("mutation did not compile; compiler failure is not a killed mutation")
        raise SystemExit(
            f"mutation survived or failed for the wrong reason; expected {spec.expected_failure!r}"
        )


def restore(originals: dict[Path, str], hashes: dict[Path, str]):
    for path, text in originals.items():
        path.write_text(text)
    for path, expected in hashes.items():
        if sha256(path.read_bytes()).hexdigest() != expected:
            raise SystemExit(f"mutation restoration failed: {path.name}")


def main():
    require_clean()
    if len(MUTATIONS) != 16:
        raise SystemExit(f"expected exactly 16 Stage 4B mutations, found {len(MUTATIONS)}")

    require_baseline()
    require_vector_negative_control()

    killed = 0
    with tempfile.TemporaryDirectory(prefix="oregon-runtime-coordinator-mutants-") as directory:
        disposable = Path(directory) / "repo"
        shutil.copytree(
            ROOT,
            disposable,
            ignore=shutil.ignore_patterns("target", ".git", ".worktrees"),
        )
        run(("git", "init", "-q"), disposable)

        for index, mutation in enumerate(MUTATIONS, start=1):
            originals: dict[Path, str] = {}
            touched = {disposable_path(edit.path, disposable) for edit in mutation.edits}
            hashes = {path: sha256(path.read_bytes()).hexdigest() for path in touched}
            try:
                for edit in mutation.edits:
                    apply_edit(edit, disposable, originals)

                for spec in mutation.tests:
                    test_failed_as_expected(spec, disposable)

                killed += 1
                print(f"KILLED {index:02d}/16: {mutation.name}")
            finally:
                restore(originals, hashes)

            for spec in mutation.tests:
                require_success(
                    spec.command,
                    f"clean baseline did not recover after mutation {mutation.name}",
                    disposable,
                )

    require_baseline()
    require_clean()
    if killed != 16:
        raise SystemExit(f"runtime coordinator mutation gate killed {killed}/16 mutations")
    print("PASS: runtime coordinator mutation gate killed 16/16 compiled mutations")


if __name__ == "__main__":
    main()
