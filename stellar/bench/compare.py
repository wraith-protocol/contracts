#!/usr/bin/env python3
"""Compare current Stellar gas bench results against a baseline.

Fails (exit 1) when any per-op `instructions` exceeds baseline + threshold
(default 5%). Prints a clear regression diff for CI logs.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


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
    parser.add_argument("baseline", type=Path, help="baseline JSON from develop")
    parser.add_argument("current", type=Path, help="current PR/run JSON")
    parser.add_argument(
        "--threshold-pct",
        type=float,
        default=5.0,
        help="max allowed per-op instructions increase (default: 5)",
    )
    args = parser.parse_args()

    baseline = load_results(args.baseline)
    current = load_results(args.current)

    missing = sorted(set(baseline) - set(current))
    added = sorted(set(current) - set(baseline))
    regressions: list[tuple[str, int, int, float]] = []
    improvements: list[tuple[str, int, int, float]] = []

    for key in sorted(set(baseline) & set(current)):
        b_ins = int(baseline[key]["instructions"])
        c_ins = int(current[key]["instructions"])
        delta = pct_delta(c_ins, b_ins)
        if delta > args.threshold_pct:
            regressions.append((key, b_ins, c_ins, delta))
        elif delta < 0:
            improvements.append((key, b_ins, c_ins, delta))

    print("=== Stellar gas bench comparison ===")
    print(f"baseline: {args.baseline}")
    print(f"current:  {args.current}")
    print(f"threshold: +{args.threshold_pct:.1f}% instructions (per-op)")
    print()

    if missing:
        print("WARNING: ops missing from current run:")
        for key in missing:
            print(f"  - {key}")
        print()

    if added:
        print("INFO: new ops (no baseline gate):")
        for key in added:
            print(f"  + {key}")
        print()

    if improvements:
        print("Improvements (instructions ↓):")
        for key, b_ins, c_ins, delta in improvements:
            print(f"  {key}: {b_ins} -> {c_ins} ({delta:+.2f}%)")
        print()

    if regressions:
        print("REGRESSIONS (per-op gas / instructions > baseline + "
              f"{args.threshold_pct:.1f}%):")
        print()
        print(f"{'op':<55} {'baseline':>12} {'current':>12} {'delta':>10}")
        print("-" * 92)
        for key, b_ins, c_ins, delta in regressions:
            print(f"{key:<55} {b_ins:12d} {c_ins:12d} {delta:+9.2f}%")
        print()
        print(
            f"FAIL: {len(regressions)} op(s) exceeded +{args.threshold_pct:.1f}% "
            "instructions vs baseline."
        )
        return 1

    print("OK: no per-op instructions regression above threshold.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
