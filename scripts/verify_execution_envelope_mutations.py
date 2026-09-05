"""Prove universal-envelope contract tests kill critical V1 wire mutations.

Run only on a disposable CI checkout. The production source is restored after
 every mutant; mutation source must never be committed or published.
"""

from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "crates/oregon-primitives/src/execution_envelope.rs"
COMMAND = [
    "cargo", "+1.85.0", "test", "--locked", "-p", "oregon-primitives",
    "--test", "execution_envelopes", "--test", "execution_envelope_security",
]

MUTATIONS = [
    (
        "unknown domain accepted as EVM",
        [("_ => Err(ExecutionEnvelopeError::UnknownDomain(value)),", "_ => Ok(Self::Evm),")],
        "discriminants_are_frozen_and_unknown_values_fail_closed",
    ),
    (
        "identical explicit fee payer accepted",
        [(
            "if parts.fee_payer == Some(parts.principal) {",
            "if false && parts.fee_payer == Some(parts.principal) {",
        )],
        "fee_payer_presence_and_scope_rules_are_canonical",
    ),
    (
        "oversized envelope bypasses top-level bound",
        [(
            "if bytes.len() > MAX_ENVELOPE_BYTES {",
            "if false && bytes.len() > MAX_ENVELOPE_BYTES {",
        )],
        "decoder_rejects_noncanonical_option_flags_and_total_size_first",
    ),
    (
        "distinct fee payer allowed without fee-payer authorization",
        [
            (
                "if has_distinct_fee_payer && !has_fee_payer_authorization {",
                "if false && has_distinct_fee_payer && !has_fee_payer_authorization {",
            ),
            (
                "if has_distinct_fee_payer && authorizations.len() != 2 {",
                "if false && has_distinct_fee_payer && authorizations.len() != 2 {",
            ),
        ],
        "fee_payer_presence_and_scope_rules_are_canonical",
    ),
    (
        "execution domain omitted from native signing commitment",
        [(
            "        bytes.push(self.execution_domain as u8);\n",
            "",
        )],
        "chain_and_domain_are_bound_into_native_signing_commitment",
    ),
    (
        "proof bytes incorrectly included in native signing commitment",
        [(
            "        for authorization in &self.authorizations {\n"
            "            bytes.push(authorization.scope as u8);\n"
            "            bytes.extend_from_slice(&(authorization.scheme as u16).to_le_bytes());\n"
            "        }\n"
            "        self.encode_payload_and_hints(&mut bytes);\n"
            "        bytes\n"
            "    }\n\n"
            "    pub fn signing_hash",
            "        for authorization in &self.authorizations {\n"
            "            bytes.push(authorization.scope as u8);\n"
            "            bytes.extend_from_slice(&(authorization.scheme as u16).to_le_bytes());\n"
            "            write_varint(authorization.proof.len() as u64, &mut bytes);\n"
            "            bytes.extend_from_slice(&authorization.proof);\n"
            "        }\n"
            "        self.encode_payload_and_hints(&mut bytes);\n"
            "        bytes\n"
            "    }\n\n"
            "    pub fn signing_hash",
        )],
        "proof_bytes_are_excluded_from_native_signing_hash_but_included_in_txid",
    ),
    (
        "proof bytes omitted from full Oregon txid",
        [(
            "domain_hash(TXID_DOMAIN, &self.encode())",
            "domain_hash(TXID_DOMAIN, &self.signing_bytes())",
        )],
        "proof_bytes_are_excluded_from_native_signing_hash_but_included_in_txid",
    ),
    (
        "Ethereum source authorization accepts mutable Oregon height window",
        [(
            "if valid_after_height != 0 || valid_until_height != u64::MAX {",
            "if false && (valid_after_height != 0 || valid_until_height != u64::MAX) {",
        )],
        "ethereum_source_authorization_requires_neutral_height_window",
    ),
]


def run(command):
    return subprocess.run(
        command,
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=180,
        check=False,
    )


def main():
    original = SOURCE.read_bytes()
    baseline = run(COMMAND)
    if baseline.returncode != 0 or "test result: ok." not in baseline.stdout:
        raise SystemExit("Clean envelope baseline failed:\n" + baseline.stdout)

    for name, edits, test in MUTATIONS:
        source = original.decode("utf-8")
        for old, new in edits:
            if source.count(old) != 1:
                raise SystemExit(f"Mutation site changed: {name}; review the mutation gate")
            source = source.replace(old, new, 1)
        try:
            SOURCE.write_text(source)
            result = run(COMMAND + [test, "--", "--exact"])
            if (
                result.returncode != 101
                or f"test {test} ... FAILED" not in result.stdout
                or "error[E" in result.stdout
            ):
                raise SystemExit(f"Mutation not killed by intended test: {name}\n{result.stdout}")
            print(f"KILLED: {name} — {test}", flush=True)
        finally:
            SOURCE.write_bytes(original)

    restored = run(COMMAND)
    if restored.returncode != 0 or "test result: ok." not in restored.stdout:
        raise SystemExit("Restored clean envelope suite failed:\n" + restored.stdout)
    print(f"{len(MUTATIONS)}/{len(MUTATIONS)} mutations killed; restored clean envelope suite passed", flush=True)


if __name__ == "__main__":
    main()
