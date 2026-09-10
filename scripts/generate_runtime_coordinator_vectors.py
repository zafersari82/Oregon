#!/usr/bin/env python3
"""Generate independent Stage 4B runtime/coordinator canonical vectors."""

import argparse
import json
import struct
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parents[1]
OUTPUT = ROOT_DIR / "tests/vectors/runtime-coordinator-v1.json"

MASK32 = 0xFFFF_FFFF
IV = [
    0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A,
    0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
]
MSG_PERMUTATION = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8]
CHUNK_START = 1
CHUNK_END = 2
ROOT = 8

EVENT_DOMAIN = b"OREGON/EXEC/EVENT/V1\0"
EVENTS_DOMAIN = b"OREGON/EXEC/EVENTS/V1\0"
STATE_EFFECT_DOMAIN = b"OREGON/EXEC/STATE-EFFECT/V1\0"
RETURN_DOMAIN = b"OREGON/EXEC/RETURN/V1\0"
OUTBOX_EFFECT_DOMAIN = b"OREGON/EXEC/OUTBOX-EFFECT/V1\0"
EXEC_RECEIPT_DOMAIN = b"OREGON/EXEC/RECEIPT/V1\0"
FEE_RECEIPT_DOMAIN = b"OREGON/FEE/RECEIPT/V1\0"

RECEIPT_BYTES = 259


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
    return [int.from_bytes(padded[offset:offset + 4], "little") for offset in range(0, 64, 4)]


def _compress(chaining_value, block_words, counter, block_len, flags):
    state = list(chaining_value) + IV[:4] + [
        counter & MASK32, (counter >> 32) & MASK32, block_len, flags
    ]
    message = list(block_words)
    for round_index in range(7):
        _round(state, message)
        if round_index != 6:
            message = [message[index] for index in MSG_PERMUTATION]
    return [
        (state[index] ^ state[index + 8]) & MASK32 for index in range(8)
    ] + [
        (state[index + 8] ^ chaining_value[index]) & MASK32 for index in range(8)
    ]


def blake3_256(data):
    """Independent BLAKE3-256 for one chunk; every vector preimage is < 1024 bytes."""
    if len(data) > 1024:
        raise ValueError("runtime coordinator oracle input exceeds one BLAKE3 chunk")

    chaining_value = IV[:]
    if not data:
        final_block = b""
        complete_blocks = 0
    else:
        block_count = (len(data) + 63) // 64
        complete_blocks = block_count - 1
        for block_index in range(complete_blocks):
            block = data[block_index * 64:(block_index + 1) * 64]
            flags = CHUNK_START if block_index == 0 else 0
            chaining_value = _compress(
                chaining_value, _block_words(block), 0, 64, flags
            )[:8]
        final_block = data[complete_blocks * 64:]

    flags = CHUNK_END | ROOT
    if complete_blocks == 0:
        flags |= CHUNK_START
    words = _compress(
        chaining_value, _block_words(final_block), 0, len(final_block), flags
    )
    return b"".join(word.to_bytes(4, "little") for word in words)[:32]


def _self_test():
    if blake3_256(b"").hex() != "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262":
        raise SystemExit("independent BLAKE3 empty-vector self-test failed")
    if blake3_256(b"abc").hex() != "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85":
        raise SystemExit("independent BLAKE3 abc-vector self-test failed")


def domain_hash(domain, payload):
    return blake3_256(domain + payload)


def evm_address(fill):
    return bytes([0x01]) + bytes(12) + bytes([fill]) * 20


def wasm_address(fill):
    return bytes([0x02]) + bytes([fill]) * 32


def encode_event(emitter, topics, data):
    return (
        struct.pack("<H", 1)
        + emitter
        + bytes([len(topics)])
        + b"".join(topics)
        + struct.pack("<I", len(data))
        + data
    )


def event_id(event):
    return domain_hash(
        EVENT_DOMAIN,
        encode_event(event["emitter"], event["topics"], event["data"]),
    )


def event_root(events):
    ids = [event_id(event) for event in events]
    return domain_hash(EVENTS_DOMAIN, struct.pack("<I", len(ids)) + b"".join(ids))


def encode_state_effects(descriptors):
    payload = struct.pack("<HH", 1, len(descriptors))
    for descriptor in descriptors:
        payload += struct.pack(
            "<HH",
            descriptor["domain_id"],
            descriptor["scheme_id"],
        )
        payload += descriptor["old_root"]
        payload += descriptor["new_root"]
    return payload


def state_effect_root(descriptors):
    return domain_hash(STATE_EFFECT_DOMAIN, encode_state_effects(descriptors))


def fee_receipt_bytes(txid, payer, fee_outcome):
    values = [2, 5, 1, 100, 10, 3, 20, 10, 30, 470]
    result = (
        struct.pack("<H", 1)
        + txid
        + bytes([0x22]) * 32
        + payer
        + bytes([0x02, fee_outcome])
        + b"".join(struct.pack("<Q", value) for value in values)
    )
    if len(result) != 181:
        raise AssertionError("fee receipt oracle width changed")
    return result


def execution_receipt_bytes(
    txid,
    payer,
    execution_outcome,
    trap_code,
    fee_outcome,
    state_root,
    events_root,
    event_count,
    return_data,
    outbox_root,
):
    fee_id = domain_hash(FEE_RECEIPT_DOMAIN, fee_receipt_bytes(txid, payer, fee_outcome))
    result = (
        struct.pack("<H", 1)
        + txid
        + bytes([0x12, execution_outcome])
        + struct.pack("<H", trap_code)
        + payer
        + struct.pack("<Q", 10)
        + struct.pack("<Q", 30)
        + fee_id
        + state_root
        + events_root
        + struct.pack("<I", event_count)
        + domain_hash(RETURN_DOMAIN, return_data)
        + struct.pack("<I", len(return_data))
        + outbox_root
        + struct.pack("<I", 0)
    )
    if len(result) != RECEIPT_BYTES:
        raise AssertionError(f"execution receipt oracle width is {len(result)}")
    return result


def hex_event(event):
    return {
        "emitter_hex": event["emitter"].hex(),
        "topics_hex": [topic.hex() for topic in event["topics"]],
        "data_hex": event["data"].hex(),
    }


def hex_descriptor(descriptor):
    return {
        "domain_id": descriptor["domain_id"],
        "scheme_id": descriptor["scheme_id"],
        "old_root_hex": descriptor["old_root"].hex(),
        "new_root_hex": descriptor["new_root"].hex(),
    }


def build():
    _self_test()

    one = bytes([0x01]) * 32
    two = bytes([0x02]) * 32
    three = bytes([0x03]) * 32

    first_event = {
        "emitter": wasm_address(0x22),
        "topics": [bytes([0xAA]) * 32],
        "data": b"hello",
    }
    second_event = {
        "emitter": evm_address(0x33),
        "topics": [bytes([0xBB]) * 32, bytes([0xCC]) * 32],
        "data": b"world",
    }

    oregon_effects = [
        {"domain_id": 0x0011, "scheme_id": 0x0001, "old_root": one, "new_root": two},
        {"domain_id": 0x0020, "scheme_id": 0x0001, "old_root": two, "new_root": three},
    ]
    evm_effects = [
        {"domain_id": 0x0010, "scheme_id": 0x0100, "old_root": one, "new_root": two}
    ]

    first_events_root = event_root([first_event])
    phase_a_root = state_effect_root(oregon_effects)
    empty_outbox_root = domain_hash(OUTBOX_EFFECT_DOMAIN, struct.pack("<I", 0))
    txid = bytes([0x11]) * 32
    payer = wasm_address(0x44)

    receipt_specs = [
        ("committed", 0x00, 0x0000, 0x00, b"ok"),
        ("reverted", 0x01, 0x0000, 0x01, b"no"),
        ("trapped", 0x02, 0x0001, 0x01, b""),
        ("resource_exhausted", 0x03, 0x0000, 0x02, b""),
    ]
    receipt_cases = []
    for name, outcome, trap_code, fee_outcome, return_data in receipt_specs:
        receipt = execution_receipt_bytes(
            txid,
            payer,
            outcome,
            trap_code,
            fee_outcome,
            phase_a_root,
            first_events_root,
            1,
            return_data,
            empty_outbox_root,
        )
        receipt_cases.append(
            {
                "name": name,
                "outcome": outcome,
                "trap_code": trap_code,
                "fee_outcome": fee_outcome,
                "return_data_hex": return_data.hex(),
                "receipt_hex": receipt.hex(),
                "receipt_id_hex": domain_hash(EXEC_RECEIPT_DOMAIN, receipt).hex(),
            }
        )

    return {
        "version": 1,
        "event_cases": [
            {
                "name": "single",
                "events": [hex_event(first_event)],
                "root_hex": first_events_root.hex(),
            },
            {
                "name": "ordered_two",
                "events": [hex_event(first_event), hex_event(second_event)],
                "root_hex": event_root([first_event, second_event]).hex(),
            },
        ],
        "state_effect_cases": [
            {
                "name": "oregon_smt",
                "descriptors": [hex_descriptor(item) for item in oregon_effects],
                "root_hex": phase_a_root.hex(),
            },
            {
                "name": "evm_commitment",
                "descriptors": [hex_descriptor(item) for item in evm_effects],
                "root_hex": state_effect_root(evm_effects).hex(),
            },
        ],
        "empty_outbox_root_hex": empty_outbox_root.hex(),
        "receipt_context": {
            "txid_hex": txid.hex(),
            "fee_payer_hex": payer.hex(),
            "state_effect_root_hex": phase_a_root.hex(),
            "events_root_hex": first_events_root.hex(),
            "event_count": 1,
            "actual_weight": 10,
            "fee_charged": 30,
            "base_fee_per_weight": 2,
            "max_fee_per_weight": 5,
            "max_priority_fee_per_weight": 1,
            "max_weight": 100,
            "refund": 470,
        },
        "receipt_cases": receipt_cases,
        "negative_metadata": [
            "duplicate_effect_domain",
            "noncanonical_effect_order",
            "execution_receipts_in_phase_a",
            "evm_labeled_oregon_smt",
            "trap_code_on_non_trapped",
            "zero_trap_code_on_trapped",
        ],
    }


def render():
    return json.dumps(build(), indent=2, sort_keys=True) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    expected = render()
    if args.check:
        if not OUTPUT.exists() or OUTPUT.read_text() != expected:
            raise SystemExit("runtime coordinator vectors are stale; regenerate them")
        return

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(expected)
    print(f"wrote {OUTPUT}")


if __name__ == "__main__":
    main()
