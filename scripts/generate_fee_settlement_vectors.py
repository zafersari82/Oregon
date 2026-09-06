#!/usr/bin/env python3
"""Generate independent Stage 3B fee-settlement and reserve vectors."""

import argparse
import json
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parents[1]
OUTPUT = ROOT_DIR / "tests/vectors/fee-settlement-v1.json"

MASK32 = 0xFFFF_FFFF
IV = [
    0x6A09E667,
    0xBB67AE85,
    0x3C6EF372,
    0xA54FF53A,
    0x510E527F,
    0x9B05688C,
    0x1F83D9AB,
    0x5BE0CD19,
]
MSG_PERMUTATION = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8]
CHUNK_START = 1
CHUNK_END = 2
ROOT = 8

CAPABILITY_DOMAIN = b"OREGON/FEE/CAPABILITY/V1\0"
ESCROW_DOMAIN = b"OREGON/FEE/ESCROW/V1\0"
RECEIPT_DOMAIN = b"OREGON/FEE/RECEIPT/V1\0"
RESERVE_TRANSITION_DOMAIN = b"OREGON/RESERVE/TRANSITION/V1\0"
RESERVE_OUTPOINT_DOMAIN = b"OREGON/RESERVE/OUTPOINT/V1\0"
RESERVE_PROGRAM = b"OREGON/EXEC/RESERVE/V1\0"
HEIGHT_TWO_SUBSIDY = 237_500_000


def _rotr32(value, count):
    return ((value >> count) | ((value << (32 - count)) & MASK32)) & MASK32


def _g(state, a, b, c, d, x, y):
    state[a] = (state[a] + state[b] + x) & MASK32
    state[d] = _rotr32(state[d] ^ state[a], 16)
    state[c] = (state[c] + state[d]) & MASK32
    state[b] = _rotr32(state[b] ^ state[c], 12)
    state[a] = (state[a] + state[b] + y) & MASK32
    state[d] = _rotr32(state[d] ^ state[a], 8)
    state[c] = (state[c] + state[d]) & MASK32
    state[b] = _rotr32(state[b] ^ state[c], 7)


def _round(state, message):
    _g(state, 0, 4, 8, 12, message[0], message[1])
    _g(state, 1, 5, 9, 13, message[2], message[3])
    _g(state, 2, 6, 10, 14, message[4], message[5])
    _g(state, 3, 7, 11, 15, message[6], message[7])
    _g(state, 0, 5, 10, 15, message[8], message[9])
    _g(state, 1, 6, 11, 12, message[10], message[11])
    _g(state, 2, 7, 8, 13, message[12], message[13])
    _g(state, 3, 4, 9, 14, message[14], message[15])


def _block_words(block):
    padded = block + bytes(64 - len(block))
    return [
        int.from_bytes(padded[offset : offset + 4], "little")
        for offset in range(0, 64, 4)
    ]


def _compress(chaining_value, block_words, counter, block_len, flags):
    state = list(chaining_value) + IV[:4] + [
        counter & MASK32,
        (counter >> 32) & MASK32,
        block_len,
        flags,
    ]
    message = list(block_words)
    for round_index in range(7):
        _round(state, message)
        if round_index != 6:
            message = [message[index] for index in MSG_PERMUTATION]
    return [
        (state[index] ^ state[index + 8]) & MASK32 for index in range(8)
    ] + [
        (state[index + 8] ^ chaining_value[index]) & MASK32
        for index in range(8)
    ]


def blake3_256(data):
    """Independent BLAKE3-256 for one chunk (all Stage 3B preimages are <1 KiB)."""
    if len(data) > 1024:
        raise ValueError("Stage 3B oracle input exceeds one BLAKE3 chunk")

    chaining_value = IV[:]
    if not data:
        final_block = b""
        complete_blocks = 0
    else:
        block_count = (len(data) + 63) // 64
        complete_blocks = block_count - 1
        for block_index in range(complete_blocks):
            block = data[block_index * 64 : (block_index + 1) * 64]
            flags = CHUNK_START if block_index == 0 else 0
            chaining_value = _compress(
                chaining_value, _block_words(block), 0, 64, flags
            )[:8]
        final_block = data[complete_blocks * 64 :]

    flags = CHUNK_END | ROOT
    if complete_blocks == 0:
        flags |= CHUNK_START
    words = _compress(
        chaining_value, _block_words(final_block), 0, len(final_block), flags
    )
    return b"".join(word.to_bytes(4, "little") for word in words)[:32]


def _blake3_self_test():
    expected_empty = "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
    expected_abc = "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"
    if blake3_256(b"").hex() != expected_empty or blake3_256(b"abc").hex() != expected_abc:
        raise SystemExit("independent BLAKE3 self-test failed")


def domain_hash(domain, payload):
    return blake3_256(domain + payload)


def le16(value):
    return value.to_bytes(2, "little")


def le32(value):
    return value.to_bytes(4, "little")


def le64(value):
    return value.to_bytes(8, "little")


def fill(byte):
    return bytes([byte]) * 32


def execution_address(kind, fill_byte):
    return bytes([kind]) + fill(fill_byte)


def checked_fee(base_fee, max_fee, priority_fee, max_weight, actual_weight):
    if (
        max_weight == 0
        or actual_weight == 0
        or actual_weight > max_weight
        or max_fee < base_fee
        or priority_fee > max_fee
    ):
        raise ValueError("invalid fee terms")
    max_escrow = max_weight * max_fee
    effective_price = min(max_fee, base_fee + priority_fee)
    charged = actual_weight * effective_price
    base_component = actual_weight * base_fee
    priority_component = charged - base_component
    refund = max_escrow - charged
    return {
        "max_escrow": max_escrow,
        "effective_price": effective_price,
        "charged": charged,
        "base_component": base_component,
        "priority_component": priority_component,
        "refund": refund,
    }


def reserve_preimage(
    chain_id,
    height,
    parent_block_hash,
    previous,
    native_deposit_total,
    execution_withdrawal_total,
    execution_fee_total,
    new_execution_balance_total,
    producer_coinbase_txid,
):
    encoded = bytearray()
    encoded += le16(1)
    encoded += le64(chain_id)
    encoded += le64(height)
    encoded += parent_block_hash
    if previous is None:
        encoded += b"\x00"
        previous_amount = 0
    else:
        encoded += b"\x01"
        encoded += previous["txid"]
        encoded += le32(previous["index"])
        previous_amount = previous["amount"]
    encoded += le64(previous_amount)
    encoded += le64(native_deposit_total)
    encoded += le64(execution_withdrawal_total)
    encoded += le64(execution_fee_total)
    encoded += le64(new_execution_balance_total)
    encoded += producer_coinbase_txid
    return bytes(encoded)


def producer_expected(native_fees, execution_fees, outputs):
    total_fees = native_fees + execution_fees
    miner_claim = sum(value for value, _ in outputs)
    if miner_claim > HEIGHT_TWO_SUBSIDY + total_fees:
        return "coinbase_overclaim"
    if miner_claim < total_fees:
        return "invalid_coinbase"
    if execution_fees:
        if (
            not outputs
            or outputs[-1][0] != execution_fees
            or outputs[-1][1] == RESERVE_PROGRAM
        ):
            return "invalid_coinbase"
    return "ok"


def build():
    _blake3_self_test()

    fee_cases = []
    for name, base_fee, max_fee, priority_fee, max_weight, actual_weight in [
        ("basic", 10, 20, 5, 100, 40),
        ("priority_capped", 10, 12, 5, 100, 40),
        ("exact_max_weight", 3, 7, 2, 11, 11),
        ("resource_exhausted", 2, 9, 3, 64, 64),
    ]:
        fee_cases.append(
            {
                "name": name,
                "base_fee_per_weight": base_fee,
                "max_fee_per_weight": max_fee,
                "max_priority_fee_per_weight": priority_fee,
                "max_weight": max_weight,
                "actual_weight": actual_weight,
                **checked_fee(
                    base_fee, max_fee, priority_fee, max_weight, actual_weight
                ),
            }
        )

    payer = execution_address(3, 0x55)
    capability_preimage = (
        le16(1)
        + bytes([2])
        + payer
        + fill(0x66)
        + le64(2000)
        + le64(7)
        + fill(0x77)
    )
    capability_id = domain_hash(CAPABILITY_DOMAIN, capability_preimage)
    txid = fill(0x44)
    escrow_preimage = (
        le16(1)
        + txid
        + payer
        + bytes([2])
        + capability_id
        + le64(10)
        + le64(20)
        + le64(5)
        + le64(100)
        + le64(2000)
    )
    escrow_id = domain_hash(ESCROW_DOMAIN, escrow_preimage)
    receipt = (
        le16(1)
        + txid
        + escrow_id
        + payer
        + bytes([2, 1])
        + b"".join(
            le64(value)
            for value in [10, 20, 5, 100, 40, 15, 400, 200, 600, 1400]
        )
    )
    if len(capability_preimage) != 116 or len(escrow_preimage) != 140 or len(receipt) != 181:
        raise AssertionError("Stage 3B fixed-width preimage changed")

    settlement_case = {
        "txid": txid.hex(),
        "payer_kind": 3,
        "payer_payload": fill(0x55).hex(),
        "source_kind": 2,
        "source_commitment": fill(0x66).hex(),
        "available_amount": 2000,
        "source_sequence": 7,
        "authorization_commitment": fill(0x77).hex(),
        "capability_preimage_hex": capability_preimage.hex(),
        "capability_id": capability_id.hex(),
        "terms": {
            "base_fee_per_weight": 10,
            "max_fee_per_weight": 20,
            "max_priority_fee_per_weight": 5,
            "max_weight": 100,
            "max_escrow": 2000,
        },
        "escrow_preimage_hex": escrow_preimage.hex(),
        "escrow_id": escrow_id.hex(),
        "outcome": 1,
        "actual_weight": 40,
        "receipt_hex": receipt.hex(),
        "receipt_id": domain_hash(RECEIPT_DOMAIN, receipt).hex(),
    }

    reserve_cases = []
    for name, previous, deposits, withdrawals, fees, new_total in [
        ("zero_deposit", None, 40, 0, 0, 40),
        (
            "rebalance",
            {"txid": fill(0x90), "index": 0, "amount": 100},
            50,
            20,
            30,
            100,
        ),
        (
            "zero_result",
            {"txid": fill(0x90), "index": 0, "amount": 100},
            0,
            80,
            20,
            0,
        ),
    ]:
        preimage = reserve_preimage(
            7,
            100,
            fill(0x10),
            previous,
            deposits,
            withdrawals,
            fees,
            new_total,
            fill(0x20),
        )
        transition_id = domain_hash(RESERVE_TRANSITION_DOMAIN, preimage)
        outpoint_txid = domain_hash(RESERVE_OUTPOINT_DOMAIN, transition_id)
        reserve_cases.append(
            {
                "name": name,
                "chain_id": 7,
                "height": 100,
                "parent_block_hash": fill(0x10).hex(),
                "previous": None
                if previous is None
                else {
                    "txid": previous["txid"].hex(),
                    "index": previous["index"],
                    "amount": previous["amount"],
                },
                "native_deposit_total": deposits,
                "execution_withdrawal_total": withdrawals,
                "execution_fee_total": fees,
                "new_execution_balance_total": new_total,
                "producer_coinbase_txid": fill(0x20).hex(),
                "canonical_transition_hex": preimage.hex(),
                "transition_id": transition_id.hex(),
                "reserve_outpoint_txid": None if new_total == 0 else outpoint_txid.hex(),
                "reserve_outpoint_index": None if new_total == 0 else 0,
            }
        )

    producer_definitions = [
        (
            "valid",
            100,
            50,
            [(HEIGHT_TWO_SUBSIDY + 100, b"\x51"), (50, b"\x52")],
        ),
        (
            "missing_execution_payout",
            100,
            50,
            [(HEIGHT_TWO_SUBSIDY + 150, b"\x51")],
        ),
        (
            "reserve_program_payout",
            100,
            50,
            [(HEIGHT_TWO_SUBSIDY + 100, b"\x51"), (50, RESERVE_PROGRAM)],
        ),
        (
            "zero_execution_fee",
            100,
            0,
            [(HEIGHT_TWO_SUBSIDY + 100, b"\x51")],
        ),
        ("fee_underclaim", 100, 50, [(99, b"\x51"), (50, b"\x52")]),
        (
            "double_count_guard",
            100,
            50,
            [(HEIGHT_TWO_SUBSIDY + 150, b"\x51"), (50, b"\x52")],
        ),
    ]
    producer_cases = []
    for name, native_fees, execution_fees, outputs in producer_definitions:
        producer_cases.append(
            {
                "name": name,
                "height": 2,
                "native_fees": native_fees,
                "execution_fees": execution_fees,
                "outputs": [
                    {"value": value, "locking_program_hex": program.hex()}
                    for value, program in outputs
                ],
                "expected": producer_expected(native_fees, execution_fees, outputs),
            }
        )

    return {
        "version": 1,
        "provenance": (
            "Independent Stage 3B oracle. Pure-Python one-chunk BLAKE3 is "
            "self-tested against the official empty and abc digests; no Rust code "
            "is invoked. All integer arithmetic uses Python arbitrary-precision integers."
        ),
        "fee_cases": fee_cases,
        "settlement_case": settlement_case,
        "reserve_cases": reserve_cases,
        "producer_cases": producer_cases,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    expected = json.dumps(build(), indent=2) + "\n"
    if args.check:
        if not OUTPUT.exists() or OUTPUT.read_text() != expected:
            raise SystemExit("Stage 3B fee-settlement vectors differ from independent oracle")
        print("Stage 3B fee-settlement reference vectors match")
    else:
        OUTPUT.parent.mkdir(parents=True, exist_ok=True)
        OUTPUT.write_text(expected)
        print(OUTPUT.relative_to(ROOT_DIR))


if __name__ == "__main__":
    main()
