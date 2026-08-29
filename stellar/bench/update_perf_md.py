#!/usr/bin/env python3
"""Replace the auto-managed Current Numbers table in stellar/PERF.md."""

from __future__ import annotations

import argparse
import json
import re
import sys
from datetime import datetime, timezone
from pathlib import Path


MARKER_START = "<!-- BENCH:CURRENT:START -->"
MARKER_END = "<!-- BENCH:CURRENT:END -->"


def table_from_results(results: list[dict]) -> str:
    lines = [
        "| Contract | Function | Parameters | Instructions | Mem bytes | Read entries | Write entries | Read bytes | Write bytes | Event bytes |",
        "|---|---|---:|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for row in results:
        lines.append(
            "| {contract} | {function} | {params} | {instructions} | {mem_bytes} | "
            "{read_entries} | {write_entries} | {read_bytes} | {write_bytes} | "
            "{events_bytes} |".format(**row)
        )
    return "\n".join(lines) + "\n"


def render_block(data: dict) -> str:
    generated = data.get("generated_at", "unknown")
    commit = data.get("commit", "unknown")
    # Prefer ISO date for humans when generated_at is a unix timestamp.
    try:
        ts = int(generated)
        measured = datetime.fromtimestamp(ts, tz=timezone.utc).strftime("%Y-%m-%d")
    except (TypeError, ValueError):
        measured = str(generated)[:10]

    intro = (
        f"These are the harness results auto-updated from `develop` "
        f"(measured {measured}, commit `{commit[:12]}`).\n\n"
    )
    return intro + table_from_results(data["results"])


def update_perf(perf_path: Path, results_path: Path) -> None:
    data = json.loads(results_path.read_text())
    if "results" not in data:
        raise SystemExit(f"{results_path}: missing 'results'")

    text = perf_path.read_text()
    block = render_block(data)

    if MARKER_START in text and MARKER_END in text:
        pattern = re.compile(
            re.escape(MARKER_START) + r".*?" + re.escape(MARKER_END),
            re.DOTALL,
        )
        replacement = f"{MARKER_START}\n{block}{MARKER_END}"
        new_text, n = pattern.subn(replacement, text, count=1)
        if n != 1:
            raise SystemExit("failed to replace BENCH markers in PERF.md")
    else:
        # Fallback: replace the "## Current Numbers" section body until next ##.
        pattern = re.compile(
            r"(## Current Numbers\n\n).*?(?=\n## |\Z)",
            re.DOTALL,
        )
        replacement = (
            f"## Current Numbers\n\n"
            f"{MARKER_START}\n{block}{MARKER_END}\n\n"
        )
        new_text, n = pattern.subn(replacement, text, count=1)
        if n != 1:
            raise SystemExit("could not find '## Current Numbers' section in PERF.md")

    perf_path.write_text(new_text)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("perf_md", type=Path, help="path to PERF.md")
    parser.add_argument("results_json", type=Path, help="bench JSON results")
    args = parser.parse_args()
    update_perf(args.perf_md, args.results_json)
    print(f"updated {args.perf_md}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
