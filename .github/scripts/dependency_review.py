#!/usr/bin/env python3
"""Review dependency changes between two commits against the OSV database.

Usage: dependency_review.py <base-sha> <head-sha>

Diffs every tracked Cargo.lock, package-lock.json and pnpm-lock.yaml between
the two commits, prints the added and changed packages, and queries
https://api.osv.dev for known advisories affecting the newly introduced
versions. Exits non-zero on any MODERATE, HIGH, CRITICAL or unrated advisory,
unless its id is listed in .github/supply-chain/osv-allowlist.txt with a
justification. LOW and informational (e.g. RustSec "unmaintained") advisories
are reported as warnings.

This works without the repository dependency graph, so it covers crates.io and
npm on any fork. Stdlib only.
"""

import json
import os
import re
import subprocess
import sys
import time
import tomllib
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ALLOWLIST = ROOT / ".github" / "supply-chain" / "osv-allowlist.txt"
OSV_BATCH = "https://api.osv.dev/v1/querybatch"
OSV_VULN = "https://api.osv.dev/v1/vulns/"
NON_BLOCKING = {"LOW", "INFORMATIONAL"}
LOCKFILES = ("Cargo.lock", "package-lock.json", "pnpm-lock.yaml")


def git(*args):
    return subprocess.run(["git", *args], cwd=ROOT, check=True, capture_output=True, text=True).stdout


def show(sha, path):
    try:
        return git("show", f"{sha}:{path}")
    except subprocess.CalledProcessError:
        return None


def parse_cargo(text):
    pkgs = set()
    for pkg in tomllib.loads(text).get("package", []):
        if pkg.get("source"):
            pkgs.add(("crates.io", pkg["name"], pkg["version"]))
    return pkgs


def parse_package_lock(text):
    pkgs = set()
    for key, meta in json.loads(text).get("packages", {}).items():
        if not key or meta.get("link") or "version" not in meta:
            continue
        name = meta.get("name") or key.rsplit("node_modules/", 1)[-1]
        pkgs.add(("npm", name, meta["version"]))
    return pkgs


def parse_pnpm_lock(text):
    pkgs = set()
    in_packages = False
    for line in text.splitlines():
        if not line.startswith(" ") and line.strip():
            in_packages = line.rstrip() == "packages:"
            continue
        m = re.match(r"^  '?(@?[^@'\s]+)@([^'():\s]+)'?:\s*$", line) if in_packages else None
        if m:
            pkgs.add(("npm", m.group(1), m.group(2)))
    return pkgs


PARSERS = {
    "Cargo.lock": parse_cargo,
    "package-lock.json": parse_package_lock,
    "pnpm-lock.yaml": parse_pnpm_lock,
}


def changed_lockfiles(base, head):
    out = git("diff", "--name-only", f"{base}...{head}")
    return [p for p in out.splitlines() if Path(p).name in LOCKFILES]


def fetch(url, body=None):
    for attempt in range(3):
        try:
            req = urllib.request.Request(url, data=body, headers={"Content-Type": "application/json"})
            with urllib.request.urlopen(req, timeout=60) as resp:
                return json.load(resp)
        except OSError as err:
            if attempt == 2:
                raise SystemExit(f"OSV query failed: {err}")
            time.sleep(2**attempt * 5)


def severity(vuln_id, cache={}):
    """GHSA severity label, INFORMATIONAL for RustSec notices, or UNRATED."""
    if vuln_id not in cache:
        data = fetch(OSV_VULN + vuln_id)
        extra = data.get("database_specific") or {}
        if data.get("withdrawn"):
            cache[vuln_id] = "WITHDRAWN"
        elif extra.get("informational"):
            cache[vuln_id] = "INFORMATIONAL"
        else:
            cache[vuln_id] = str(extra.get("severity") or "UNRATED").upper()
    return cache[vuln_id]


def osv_query(pkgs):
    results = []
    pkgs = sorted(pkgs)
    for i in range(0, len(pkgs), 500):
        chunk = pkgs[i : i + 500]
        body = json.dumps(
            {"queries": [{"package": {"ecosystem": e, "name": n}, "version": v} for e, n, v in chunk]}
        ).encode()
        data = fetch(OSV_BATCH, body)
        for pkg, res in zip(chunk, data.get("results", [])):
            ids = [v["id"] for v in res.get("vulns", [])]
            if ids:
                results.append((pkg, ids))
    return results


def allowlist():
    if not ALLOWLIST.exists():
        return set()
    ids = set()
    for line in ALLOWLIST.read_text(encoding="utf-8").splitlines():
        entry = line.split("#", 1)[0].strip()
        if entry:
            ids.add(entry)
    return ids


def summary(text):
    path = os.environ.get("GITHUB_STEP_SUMMARY")
    if path:
        with open(path, "a", encoding="utf-8") as fh:
            fh.write(text + "\n")
    print(text)


def main():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    base, head = sys.argv[1], sys.argv[2]

    introduced = set()
    lines = ["## Dependency review", ""]
    for path in changed_lockfiles(base, head):
        parse = PARSERS[Path(path).name]
        before_text, after_text = show(base, path), show(head, path)
        before = parse(before_text) if before_text else set()
        after = parse(after_text) if after_text else set()
        added = after - before
        removed = before - after
        if not added and not removed:
            continue
        introduced |= added
        lines.append(f"### `{path}`: {len(added)} added, {len(removed)} removed")
        lines += [f"- `+ {n}@{v}` ({e})" for e, n, v in sorted(added, key=lambda p: p[1])]
        lines += [f"- `- {n}@{v}` ({e})" for e, n, v in sorted(removed, key=lambda p: p[1])]
        lines.append("")

    if not introduced:
        summary("## Dependency review\n\nNo npm or Cargo dependency changes.")
        return 0

    allowed = allowlist()
    hits = [(pkg, [(i, severity(i)) for i in ids]) for pkg, ids in osv_query(introduced)]
    hits = [(pkg, [(i, sev) for i, sev in ids if sev != "WITHDRAWN"]) for pkg, ids in hits]
    hits = [(pkg, ids) for pkg, ids in hits if ids]

    blocking = []
    if hits:
        lines.append("### Known advisories in introduced versions")
        for (e, n, v), ids in hits:
            marks = []
            for i, sev in ids:
                note = "allowlisted" if i in allowed else sev.lower()
                marks.append(f"{i} ({note})")
                if i not in allowed and sev not in NON_BLOCKING:
                    blocking.append(f"{n}@{v} ({e}): {i} {sev}")
                elif i not in allowed:
                    print(f"::warning::{n}@{v} ({e}): {i} {sev}")
            lines.append(f"- `{n}@{v}` ({e}): {', '.join(marks)}")
    else:
        lines.append(f"No known advisories affect the {len(introduced)} introduced package versions.")
    summary("\n".join(lines))

    if blocking:
        for entry in blocking:
            print(f"::error::{entry}")
        print("See https://osv.dev for details, or record a justified exception per SUPPLY_CHAIN.md.")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
