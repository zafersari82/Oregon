"""Prove execution-address tests detect fail-open and namespace mutations.

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
        [("_ => Err(ExecutionAddressError::UnknownKind(value)),", "_ => Ok(Self::Wasm),")],
        "unknown_kinds_are_rejected_instead_of_becoming_another_namespace",
    ),
    (
        "noncanonical EVM padding acceptance",
        [(
            "kind == ExecutionAddressKind::Evm && payload[..12].iter().any(|&byte| byte != 0)",
            "kind == ExecutionAddressKind::Evm && false",
        )],
        "nonzero_evm_padding_cannot_create_aliases",
    ),
    (
        "coordinated WASM/Oregon namespace reassignment",
        [
            ("Wasm = 0x02,\n    Oregon = 0x03,", "Wasm = 0x03,\n    Oregon = 0x02,"),
            (
                "0x02 => Ok(Self::Wasm),\n            0x03 => Ok(Self::Oregon),",
                "0x02 => Ok(Self::Oregon),\n            0x03 => Ok(Self::Wasm),",
            ),
        ],
        "semantic_namespaces_use_their_frozen_wire_tags",
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

    for name, edits, test in MUTATIONS:
        source = original.decode("utf-8")
        for old, new in edits:
            if source.count(old) != 1:
                raise SystemExit(f"Mutation site changed: {name}; review the mutation gate")
            source = source.replace(old, new, 1)
        try:
            SOURCE.write_text(source)
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
    print(f"{len(MUTATIONS)}/{len(MUTATIONS)} mutations killed; restored clean address suite passed", flush=True)


if __name__ == "__main__":
    main()
