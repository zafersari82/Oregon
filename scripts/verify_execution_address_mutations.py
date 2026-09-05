"""Prove execution-address rejection tests detect two fail-open mutations.

Run only on a disposable verification checkout. The source file is restored in
finally even on a failed gate; mutations must never be committed or published.
"""

from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "crates/oregon-primitives/src/execution_address.rs"
COMMAND = [
    "cargo", "+1.85.0", "test", "--locked", "-p", "oregon-primitives",
    "--test", "execution_addresses",
]
MUTATIONS = [
    (
        "unknown-kind acceptance",
        "_ => Err(ExecutionAddressError::UnknownKind(value)),",
        "_ => Ok(Self::Wasm),",
        "unknown_kinds_are_rejected_instead_of_becoming_another_namespace",
    ),
    (
        "noncanonical EVM padding acceptance",
        "kind == ExecutionAddressKind::Evm && payload[..12].iter().any(|&byte| byte != 0)",
        "kind == ExecutionAddressKind::Evm && false",
        "nonzero_evm_padding_cannot_create_aliases",
    ),
]


def run(command):
    return subprocess.run(
        command, cwd=ROOT, text=True, stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT, timeout=120, check=False,
    )


def main():
    original = SOURCE.read_bytes()
    baseline = run(COMMAND)
    if baseline.returncode != 0 or "test result: ok." not in baseline.stdout:
        raise SystemExit("Clean address baseline failed:\n" + baseline.stdout)

    for name, old, new, test in MUTATIONS:
        source = original.decode("utf-8")
        if source.count(old) != 1:
            raise SystemExit(f"Mutation site changed: {name}; review the mutation gate")
        try:
            SOURCE.write_text(source.replace(old, new, 1))
            result = run(COMMAND + [test, "--", "--exact"])
            # A build error, timeout, missing test or unrelated suite failure is
            # not evidence that the intended security assertion killed a mutant.
            if (
                result.returncode != 101
                or f"test {test} ... FAILED" not in result.stdout
                or "test result: FAILED. 0 passed; 1 failed;" not in result.stdout
            ):
                raise SystemExit(f"Mutation not killed as required: {name}\n{result.stdout}")
            print(f"KILLED: {name} — {test}", flush=True)
        finally:
            SOURCE.write_bytes(original)

    restored = run(COMMAND)
    if restored.returncode != 0 or "test result: ok." not in restored.stdout:
        raise SystemExit("Restored clean address suite failed:\n" + restored.stdout)
    print("2/2 mutations killed; restored clean address suite passed", flush=True)


if __name__ == "__main__":
    main()
