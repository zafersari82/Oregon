"""Kill the required Stage 3A resource and fee security mutations."""

from pathlib import Path
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
WEIGHT = ROOT / "crates/oregon-execution/src/weight.rs"
FEES = ROOT / "crates/oregon-consensus/src/execution_resources.rs"


def cargo_test(crate: str, test_target: str, test_name: str | None = None,
               cwd: Path = ROOT) -> subprocess.CompletedProcess[str]:
    command = ["cargo", "+1.85.0", "test", "--locked", "-p", crate, "--test", test_target]
    if test_name is not None:
        command.extend([test_name, "--", "--exact"])
    return subprocess.run(command, cwd=cwd, text=True, stdout=subprocess.PIPE,
                          stderr=subprocess.STDOUT, check=False)


def run(command: list[str], cwd: Path = ROOT) -> subprocess.CompletedProcess[str]:
    return subprocess.run(command, cwd=cwd, text=True, stdout=subprocess.PIPE,
                          stderr=subprocess.STDOUT, check=False)


def require_clean() -> None:
    result = run(["git", "status", "--porcelain", "--untracked-files=all"])
    if result.returncode != 0 or result.stdout.strip():
        raise SystemExit("mutation gate requires a clean disposable checkout")


def require_baseline() -> None:
    for crate, target in [("oregon-consensus", "execution_resources"),
                          ("oregon-execution", "weight")]:
        result = cargo_test(crate, target)
        if result.returncode != 0:
            print(result.stdout)
            raise SystemExit("clean Stage 3A test baseline failed")


MUTATIONS = [
    ("conversion floors instead of ceiling", WEIGHT,
     "let rounded = quotient + u128::from(remainder != 0);",
     "let rounded = quotient;", "oregon-execution", "weight",
     "conversion_rounds_up"),
    ("cumulative conversion uses the new value twice", WEIGHT,
     "let Some(delta) = new_weight.checked_sub(old_weight) else {",
     "let Some(delta) = new_weight.checked_sub(new_weight) else {",
     "oregon-execution", "weight", "split_charges_share_cumulative_rounding"),
    ("common work is not charged", WEIGHT,
     "self.consumed += weight;", "self.consumed += 0;",
     "oregon-execution", "weight", "common_work_consumes_budget"),
    ("exhaustion is not sticky", WEIGHT,
     "self.exhausted = true;", "self.exhausted = false;",
     "oregon-execution", "weight", "exhaustion_is_sticky"),
    ("exact budget is rejected", WEIGHT,
     "if delta > self.max_weight - self.consumed {",
     "if delta >= self.max_weight - self.consumed {",
     "oregon-execution", "weight", "exact_budget_succeeds_then_overrun_exhausts"),
    ("native counter overflow wraps", WEIGHT,
     "self.counters[index].checked_add(units)",
     "Some(self.counters[index].wrapping_add(units))",
     "oregon-execution", "weight", "native_counter_overflow_is_terminal"),
    ("under-target fee movement goes upward", FEES,
     "if parent_weight > parameters.target_weight {",
     "if parent_weight < parameters.target_weight {",
     "oregon-consensus", "execution_resources", "empty_parent_lowers_fee"),
    ("minimum upward movement is omitted", FEES,
     "change.max(1)", "change",
     "oregon-consensus", "execution_resources", "tiny_upward_change_costs_one_unit"),
    ("base-fee ceiling is omitted", FEES,
     ".min(parameters.max_base_fee)", ".min(u64::MAX)",
     "oregon-consensus", "execution_resources", "fee_never_exceeds_ceiling"),
    ("parent utilization bound is omitted", FEES,
     "if parent_weight > parameters.block_weight_limit() {",
     "if false && parent_weight > parameters.block_weight_limit() {",
     "oregon-consensus", "execution_resources", "invalid_parent_utilization_rejected"),
]


def main() -> None:
    require_clean()
    require_baseline()
    killed = 0
    with tempfile.TemporaryDirectory(prefix="oregon-resource-mutants-") as directory:
        disposable = Path(directory) / "repo"
        shutil.copytree(ROOT, disposable, ignore=shutil.ignore_patterns("target", ".git"))
        run(["git", "init", "-q"], disposable)
        # The source tree is already clean; the disposable copy is only a restoration sandbox.
        for name, source_path, old, new, crate, target, test_name in MUTATIONS:
            source = disposable / source_path.relative_to(ROOT)
            original = source.read_text()
            if original.count(old) != 1:
                raise SystemExit(f"mutation site is not unique: {name}")
            source.write_text(original.replace(old, new, 1))
            try:
                result = cargo_test(crate, target, test_name, disposable)
                expected_failure = (
                    result.returncode == 101
                    and f"test {test_name} ... FAILED" in result.stdout
                    and "could not compile" not in result.stdout
                )
                if not expected_failure:
                    print(result.stdout)
                    raise SystemExit(f"mutant survived or failed to compile: {name}")
                killed += 1
                print(f"KILLED: {name} [{test_name}]")
            finally:
                source.write_text(original)
            if source.read_text() != original:
                raise SystemExit(f"source restoration failed: {name}")
    print(f"Execution resource mutations: {killed}/{len(MUTATIONS)} killed")


if __name__ == "__main__":
    main()
