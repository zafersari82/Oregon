#!/usr/bin/env python3
"""Synthetic Stage 3A vectors; Python exact fractions, no Rust dependency."""
import argparse
from fractions import Fraction
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "tests/vectors/execution-resources-v1.json"
U64 = (1 << 64) - 1
SUPPLY = 100_000_000_000_000


def converted(units, numerator, denominator):
    quotient, remainder = divmod(units * numerator, denominator)
    result = quotient + bool(remainder)
    return result if result <= U64 else None


def next_fee(parameters, base, used):
    _, target, _, divisor, floor, ceiling = parameters
    if used == target:
        return base
    change = int(Fraction(base * abs(used - target), target * divisor))
    if used > target:
        return min(ceiling, base + max(1, change))
    return max(floor, base - change)


def build():
    ratios = [(2, 3), (1, U64), (U64, U64), (U64, 1), (U64, U64 - 1)]
    conversions = [
        {"numerator": n, "denominator": d, "units": u,
         "expected": converted(u, n, d)}
        for n, d in ratios for u in [0, 1, 2, 3, U64]
    ]
    normal = [1, 100, 120, 8, 1, 1000]
    profiles = [
        (normal, 100, [0, 1, 99, 100, 101, 199, 200]),
        (normal, 1, [0, 100, 101, 200]),
        (normal, 999, [0, 100, 101, 200]),
        (normal, 1000, [0, 100, 101, 200]),
        ([1, 100, 200, 8, 50, 1000], 50, [0, 99, 101, 200]),
        ([1, U64 // 2, U64 - 1, U64, 1, SUPPLY], SUPPLY,
         [0, U64 // 2, U64 - 1]),
        ([1, U64 // 2, U64 - 1, 2, 1, SUPPLY], SUPPLY // 2,
         [0, U64 // 2, U64 - 1]),
    ]
    fees = [{"parameters": p, "parent_base_fee": b, "parent_weight": u,
             "expected": next_fee(p, b, u)} for p, b, uses in profiles for u in uses]
    sequences = []
    for name, start, uses in [
        ("producer_full", 100, [200] * 5),
        ("producer_empty", 100, [0] * 5),
        ("producer_alternating", 100, [200, 0] * 8),
        ("producer_ceiling", 100, [200] * 64),
        ("producer_floor", 100, [0] * 64),
    ]:
        previous, results = start, []
        for used in uses:
            previous = next_fee(normal, previous, used)
            results.append(previous)
        sequences.append({"name": name, "parameters": normal, "initial_fee": start,
                          "parent_weights": uses, "expected_fees": results})
    assert sequences[0]["expected_fees"] == [112, 126, 141, 158, 177]
    assert sequences[1]["expected_fees"] == [88, 77, 68, 60, 53]
    assert sequences[3]["expected_fees"][-1] == 1000
    # Integer downward rounding intentionally stalls below D (not necessarily at min).
    assert sequences[4]["expected_fees"][-1] == 7
    meters = []
    cases = [
        ("split", [(1, 1), (2, 3), (3, 2)], 20, 2,
         [("evm", 1), ("evm", 1), ("evm", 1)]),
        ("combined", [(1, 1), (2, 3), (3, 2)], 20, 2, [("evm", 3)]),
        ("shared", [(1, 1), (2, 3), (3, 2)], 10, 2,
         [("native", 2), ("evm", 3), ("wasm", 2), ("common", 1),
          ("common", 0), ("native", 1), ("common", 0)]),
        ("counter_limit", [(1, U64)] * 3, 3, 0,
         [("native", U64), ("native", 1), ("evm", 0)]),
        ("conversion_limit", [(U64, 1)] * 3, U64, 0,
         [("wasm", 2), ("common", 0)]),
    ]
    for name, ratios, limit, intrinsic, charges in cases:
        counters = {"native": 0, "evm": 0, "wasm": 0}
        costs = dict(zip(counters, ratios))
        used, exhausted, events = intrinsic, False, []
        for domain, units in charges:
            next_counter = None
            if exhausted:
                failure = True
            elif domain == "common":
                failure = units > limit - used
                if not failure:
                    used += units
            else:
                next_counter = counters[domain] + units
                new = converted(next_counter, *costs[domain])
                old = converted(counters[domain], *costs[domain])
                failure = next_counter > U64 or new is None or new - old > limit - used
                if not failure:
                    used += new - old
                    counters[domain] = next_counter
            if failure:
                used, exhausted = limit, True
            events.append({"domain": domain, "units": units, "consumed": used,
                           "exhausted": exhausted, "failed": failure})
        meters.append({"name": name, "ratios": ratios, "max_weight": limit,
                       "intrinsic_weight": intrinsic, "events": events})
    block_budget_cases = [{
        "parameters": normal,
        "attempts": [0, 120, 121, 80, 1],
        "expected": ["invalid_transaction", "ok", "invalid_transaction", "ok", "block_exceeded"],
    }]
    return {"version": 1, "provenance": "Synthetic non-activation parameters; Python arbitrary-precision integers and fractions.Fraction; regenerate with scripts/generate_execution_resource_vectors.py",
            "conversions": conversions, "fees": fees, "sequences": sequences, "meters": meters,
            "invalid_fee_cases": [{"parameters": normal, "parent_base_fee": 100,
                                   "parent_weight": 201}],
            "block_budget_cases": block_budget_cases,
            "invalid_schedule_versions": [0, 2]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    expected = json.dumps(build(), indent=2) + "\n"
    if args.check:
        if not OUTPUT.exists() or OUTPUT.read_text() != expected:
            raise SystemExit("Execution resource vectors differ from the independent reference")
        print("Execution resource reference vectors match")
    else:
        OUTPUT.write_text(expected)
        print(OUTPUT.relative_to(ROOT))


if __name__ == "__main__":
    main()
