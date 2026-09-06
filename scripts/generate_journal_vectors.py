#!/usr/bin/env python3
"""Generate independent Stage 4A bounded journal vectors."""

import argparse
import json
from copy import deepcopy
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parents[1]
OUTPUT = ROOT_DIR / "tests/vectors/journal-v1.json"

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

KEY_DOMAIN = b"OREGON/STATE/SMT/KEY/V1\0"
VALUE_DOMAIN = b"OREGON/STATE/SMT/VALUE/V1\0"
EMPTY_DOMAIN = b"OREGON/STATE/SMT/EMPTY/V1\0"
LEAF_DOMAIN = b"OREGON/STATE/SMT/LEAF/V1\0"
NODE_DOMAIN = b"OREGON/STATE/SMT/NODE/V1\0"
SMT_DEPTH = 256

WASM = 0x0011
EXECUTION_ACCOUNTING = 0x0020
EXECUTION_RECEIPTS = 0x0030
ASYNC_OUTBOX = 0x0040
ASYNC_CONSUMED = 0x0041
FEE_STATE = 0x0050
SUPPORTED_DOMAINS = {
    WASM,
    EXECUTION_ACCOUNTING,
    EXECUTION_RECEIPTS,
    ASYNC_OUTBOX,
    ASYNC_CONSUMED,
    FEE_STATE,
}


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
    """Independent BLAKE3-256 for one chunk; all oracle hash preimages are small."""
    if len(data) > 1024:
        raise ValueError("journal oracle BLAKE3 input exceeds one chunk")

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
    if (
        blake3_256(b"").hex() != expected_empty
        or blake3_256(b"abc").hex() != expected_abc
    ):
        raise SystemExit("independent BLAKE3 self-test failed")


def domain_hash(domain, payload):
    return blake3_256(domain + payload)


def domain_prefix(domain):
    if domain not in SUPPORTED_DOMAINS:
        raise ValueError(f"unsupported journal vector domain: {domain}")
    return domain.to_bytes(2, "little")


def path_key(domain, key):
    return domain_hash(KEY_DOMAIN, domain_prefix(domain) + key)


def value_hash(domain, value):
    return domain_hash(VALUE_DOMAIN, domain_prefix(domain) + value)


def branch_hash(domain, depth, left, right):
    return domain_hash(
        NODE_DOMAIN,
        domain_prefix(domain) + depth.to_bytes(2, "little") + left + right,
    )


def leaf_hash(domain, path, hashed_value):
    return domain_hash(
        LEAF_DOMAIN, domain_prefix(domain) + path + hashed_value
    )


def empty_hashes(domain):
    hashes = [None] * (SMT_DEPTH + 1)
    hashes[SMT_DEPTH] = domain_hash(EMPTY_DOMAIN, domain_prefix(domain))
    for depth in range(SMT_DEPTH - 1, -1, -1):
        hashes[depth] = branch_hash(
            domain, depth, hashes[depth + 1], hashes[depth + 1]
        )
    return hashes


def state_root(domain, state):
    """Bottom-up logical sparse-tree oracle, independent of Rust nodes/transitions."""
    empty = empty_hashes(domain)
    leaves = []
    for key, value in sorted(state.items()):
        path = path_key(domain, key)
        leaves.append(
            (
                int.from_bytes(path, "big"),
                leaf_hash(domain, path, value_hash(domain, value)),
            )
        )
    if len({path for path, _ in leaves}) != len(leaves):
        raise ValueError("journal vector raw keys collide on a sparse-tree path")

    def subtree(items, depth):
        if not items:
            return empty[depth]
        if depth == SMT_DEPTH:
            if len(items) != 1:
                raise ValueError("sparse-tree path collision")
            return items[0][1]

        shift = SMT_DEPTH - 1 - depth
        left = []
        right = []
        for path, hashed_leaf in items:
            target = right if ((path >> shift) & 1) else left
            target.append((path, hashed_leaf))
        return branch_hash(
            domain,
            depth,
            subtree(left, depth + 1),
            subtree(right, depth + 1),
        )

    return subtree(leaves, 0)


def encoded_entry(key, value):
    return {"key_hex": key.hex(), "value_hex": value.hex()}


def domain_case(domain, entries):
    state = dict(entries)
    return {
        "domain_id": domain,
        "base_entries": [
            encoded_entry(key, value) for key, value in sorted(state.items())
        ],
    }


def put(domain, key, value):
    return {
        "op": "put",
        "domain_id": domain,
        "key_hex": key.hex(),
        "value_hex": value.hex(),
    }


def delete(domain, key):
    return {"op": "delete", "domain_id": domain, "key_hex": key.hex()}


def begin():
    return {"op": "begin"}


def commit():
    return {"op": "commit"}


def revert():
    return {"op": "revert"}


def decode_entries(entries):
    return {
        bytes.fromhex(item["key_hex"]): bytes.fromhex(item["value_hex"])
        for item in entries
    }


def materialize_expected(case):
    base = {
        item["domain_id"]: decode_entries(item["base_entries"])
        for item in case["domains"]
    }
    frames = [{}]

    for operation in case["operations"]:
        kind = operation["op"]
        if kind == "begin":
            frames.append({})
            continue
        if kind == "commit":
            if len(frames) == 1:
                raise ValueError("vector attempts to commit root frame")
            child = frames.pop()
            parent = frames[-1]
            for domain, writes in child.items():
                parent.setdefault(domain, {}).update(writes)
            continue
        if kind == "revert":
            if len(frames) == 1:
                raise ValueError("vector attempts to revert root frame")
            frames.pop()
            continue

        domain = operation["domain_id"]
        key = bytes.fromhex(operation["key_hex"])
        if domain not in base:
            raise ValueError("vector operation uses unconfigured domain")
        if kind == "put":
            value = bytes.fromhex(operation["value_hex"])
        elif kind == "delete":
            value = None
        else:
            raise ValueError(f"unknown journal vector operation: {kind}")
        frames[-1].setdefault(domain, {})[key] = value

    if len(frames) != 1:
        raise ValueError("vector leaves child frame open at finalization")

    final = deepcopy(base)
    if case["intent"] == "committed":
        for domain, writes in frames[0].items():
            for key, value in writes.items():
                if value is None:
                    final[domain].pop(key, None)
                else:
                    final[domain][key] = value
    elif case["intent"] != "reverted":
        raise ValueError("unknown journal vector intent")

    expected_domains = []
    changed_domains = []
    for domain in sorted(base):
        old_root = state_root(domain, base[domain])
        new_root = state_root(domain, final[domain])
        expected_domains.append(
            {
                "domain_id": domain,
                "old_root_hex": old_root.hex(),
                "new_root_hex": new_root.hex(),
                "final_entries": [
                    encoded_entry(key, value)
                    for key, value in sorted(final[domain].items())
                ],
            }
        )
        if case["intent"] == "committed" and new_root != old_root:
            changed_domains.append(domain)

    result = deepcopy(case)
    result["expected_domains"] = expected_domains
    result["expected_transition_domains"] = changed_domains
    return result


def build():
    _blake3_self_test()

    cases = [
        {
            "name": "nonempty_base_override",
            "domains": [
                domain_case(WASM, [(b"alpha", b"old"), (b"keep", b"base")])
            ],
            "operations": [
                put(WASM, b"alpha", b"new"),
                put(WASM, b"beta", b"x"),
            ],
            "intent": "committed",
        },
        {
            "name": "nested_child_commit",
            "domains": [domain_case(WASM, [])],
            "operations": [
                put(WASM, b"root", b"r"),
                begin(),
                put(WASM, b"k", b"child"),
                begin(),
                put(WASM, b"nested", b"x"),
                commit(),
                commit(),
            ],
            "intent": "committed",
        },
        {
            "name": "ancestor_revert_discards_committed_descendant",
            "domains": [domain_case(WASM, [(b"base", b"persisted")])],
            "operations": [
                put(WASM, b"survivor", b"root"),
                begin(),
                put(WASM, b"child", b"c"),
                begin(),
                put(WASM, b"grandchild", b"g"),
                commit(),
                revert(),
            ],
            "intent": "committed",
        },
        {
            "name": "delete_and_present_empty",
            "domains": [
                domain_case(
                    WASM,
                    [
                        (b"delete-me", b"old"),
                        (b"empty", b"old"),
                        (b"stable", b"s"),
                    ],
                )
            ],
            "operations": [
                delete(WASM, b"delete-me"),
                put(WASM, b"empty", b""),
            ],
            "intent": "committed",
        },
        {
            "name": "multiple_domains_sorted_roots",
            "domains": [
                domain_case(FEE_STATE, [(b"fee", b"10")]),
                domain_case(WASM, [(b"code", b"v1")]),
                domain_case(ASYNC_OUTBOX, []),
            ],
            "operations": [
                put(FEE_STATE, b"fee", b"11"),
                begin(),
                put(ASYNC_OUTBOX, b"msg", b"payload"),
                put(WASM, b"code", b"v2"),
                commit(),
                begin(),
                put(WASM, b"discard", b"no"),
                revert(),
            ],
            "intent": "committed",
        },
        {
            "name": "whole_transaction_revert",
            "domains": [
                domain_case(WASM, [(b"code", b"v1")]),
                domain_case(FEE_STATE, [(b"fee", b"7")]),
            ],
            "operations": [
                put(WASM, b"code", b"v2"),
                begin(),
                put(FEE_STATE, b"fee", b"9"),
                commit(),
                put(WASM, b"extra", b"x"),
            ],
            "intent": "reverted",
        },
    ]

    return {
        "version": 1,
        "context": {
            "chain_id": 7,
            "height": 42,
            "parent_block_hash_hex": (bytes([0x11]) * 32).hex(),
            "txid_hex": (bytes([0x22]) * 32).hex(),
        },
        "cases": [materialize_expected(case) for case in cases],
    }


def rendered():
    return json.dumps(build(), indent=2, sort_keys=True) + "\n"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail unless the committed literal corpus matches this independent oracle",
    )
    args = parser.parse_args()

    content = rendered()
    if args.check:
        if not OUTPUT.exists():
            raise SystemExit(f"missing committed journal vector corpus: {OUTPUT}")
        if OUTPUT.read_text() != content:
            raise SystemExit(
                "journal vector corpus differs from independent oracle; "
                "regenerate intentionally and review the literal diff"
            )
        print("journal-v1.json matches independent Stage 4A oracle")
        return

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(content)
    print(f"wrote {OUTPUT}")


if __name__ == "__main__":
    main()
