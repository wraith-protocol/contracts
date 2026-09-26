#!/usr/bin/env python3
"""Compare current Stellar gas bench results against a baseline.

The baseline path supplied by CI is authoritative. CI selects the weekly
develop baseline artifact when available and falls back to the committed
baseline only when the artifact is unavailable.

Fails (exit 1) when a required operation is missing or any resource dimension
regresses beyond the configured threshold.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


DIMENSIONS = [
    "instructions",
    "mem_bytes",
    "read_entries",
    "write_entries",
    "read_bytes",
    "write_bytes",
    "events_bytes",
]

# Deterministic operations that must always be present in the current bench.
REQUIRED_OPS = {
    "stealth-sender::batch_send::batch_size=100",
    "stealth-batch-sender::batch_send::count=100",
    "wraith-names::bulk_register::count=20",
    "wraith-names::bulk_renew::count=20",
    "stealth-splitter::create_split::beneficiaries=25",
    "stealth-splitter::fund_split::beneficiaries=25",
    "stealth-vault::deposit::asset=xlm",
    "stealth-vault::deposit::asset=issued",
    "stealth-vault::claim::unlocked",
    "stealth-vault::refund::depositor",
    "stealth-vault::refund_permissionless::keeper",
    "governance::propose::happy_path",
    "governance::vote::happy_path",
    "governance::execute::happy_path",
}


def load_results(path: Path) -> dict[str, dict[str, Any]]:
    data = json.loads(path.read_text())
    results = data.get("results")

    if not isinstance(results, list):
        raise SystemExit(f"{path}: missing 'results' array")

    out: dict[str, dict[str, Any]] = {}

    for row in results:
        key = f"{row['contract']}::{row['function']}::{row['params']}"
        out[key] = row

    return out


def pct_delta(current: float, baseline: float) -> float:
    if baseline == 0:
        return 0.0 if current == 0 else float("inf")

    return ((current - baseline) / baseline) * 100.0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "baseline",
        type=Path,
        help="authoritative baseline selected by CI",
    )
    parser.add_argument(
        "current",
        type=Path,
        help="current PR/run JSON",
    )
    parser.add_argument(
        "--threshold-pct",
        type=float,
        default=5.0,
        help="max allowed per-op dimension increase (default: 5)",
    )
    args = parser.parse_args()

    # The supplied baseline is authoritative. Do not read or merge any
    # alternate baseline here.
    baseline = load_results(args.baseline)
    current = load_results(args.current)

    missing_current = sorted(REQUIRED_OPS - set(current))
    missing_from_baseline = sorted(set(baseline) - set(current))
    added = sorted(set(current) - set(baseline))

    regressions: list[tuple[str, str, int, int, float]] = []

    for key in sorted(set(baseline) & set(current)):
        for dim in DIMENSIONS:
            b_val = int(baseline[key][dim])
            c_val = int(current[key][dim])
            delta = pct_delta(c_val, b_val)

            if dim in {"read_entries", "write_entries"}:
                if c_val > b_val:
                    regressions.append(
                        (key, dim, b_val, c_val, delta)
                    )
            elif delta > args.threshold_pct:
                regressions.append(
                    (key, dim, b_val, c_val, delta)
                )

    print("=== Stellar gas bench comparison ===")
    print(f"baseline: {args.baseline}")
    print(f"current:  {args.current}")
    print(f"threshold: +{args.threshold_pct:.1f}%")
    print()

    if missing_current:
        print("FAIL: required operations are missing from current run:")
        for key in missing_current:
            print(f"  - {key}")
        print()
        return 1

    if missing_from_baseline:
        print("WARNING: ops missing from current run:")
        for key in missing_from_baseline:
            print(f"  - {key}")
        print()

    if added:
        print("INFO: new ops (no baseline gate):")
        for key in added:
            print(f"  + {key}")
        print()

    if regressions:
        print(
            f"REGRESSIONS (metrics exceeded baseline + "
            f"{args.threshold_pct:.1f}% limit):"
        )
        print()
        print(
            f"{'op':<55} {'dimension':<15} "
            f"{'baseline':>12} {'current':>12} {'delta':>10}"
        )
        print("-" * 110)

        for key, dim, b_val, c_val, delta in regressions:
            print(
                f"{key:<55} {dim:<15} "
                f"{b_val:12d} {c_val:12d} {delta:+9.2f}%"
            )

        print()
        print(f"FAIL: {len(regressions)} dimension(s) regressed.")
        return 1

    print("OK: no resource regressions above threshold.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
