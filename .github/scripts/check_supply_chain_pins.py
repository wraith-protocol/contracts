#!/usr/bin/env python3
"""Enforce the pinning rules in SUPPLY_CHAIN.md for CI and release inputs.

Checks every workflow under .github/workflows and every tracked Dockerfile:

  * `uses:` references a full 40-character commit SHA with a `# vX.Y.Z` comment
    (local `./` actions are exempt; `docker://` images need an @sha256 digest).
  * `runs-on:` names a fixed runner image, not a `-latest` alias.
  * Toolchain-installing actions receive an explicit, exact version input.
  * `cargo install` uses `--locked` and an exact `--version`.
  * Nothing is piped from the network straight into a shell.
  * Downloaded release archives are checked with `sha256sum -c`.
  * Dockerfile `FROM` lines carry an @sha256 digest.
  * Lockfiles resolve only from crates.io and registry.npmjs.org.

Stdlib only, so it runs on a bare runner. Findings are printed as GitHub
annotations and the script exits non-zero if there are any.
"""

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

SHA_REF = re.compile(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_./-]+@[0-9a-f]{40}$")
USES = re.compile(r"^\s*(?:-\s+)?uses:\s*([^\s#]+)\s*(?:#\s*(\S.*))?$")
RUNS_ON = re.compile(r"^\s*runs-on:\s*(\S+)")
STEP_START = re.compile(r"^(\s*)-\s+\S")
WITH_KEY = re.compile(r"^\s+([A-Za-z0-9_-]+):\s*['\"]?([^'\"#]*?)['\"]?\s*(?:#.*)?$")
ENV_REF = re.compile(r"^\$\{\{\s*env\.([A-Z0-9_]+)\s*\}\}$")
EXACT_VERSION = re.compile(r"^v?\d+\.\d+\.\d+$|^nightly-\d{4}-\d{2}-\d{2}$")
PIPE_TO_SHELL = re.compile(r"\|\s*(?:sudo\s+)?(?:ba|z)?sh\b")
ARCHIVE_DOWNLOAD = re.compile(r"\b(?:curl|wget)\b.*\.(?:tar\.gz|tgz|tar\.xz|zip)\b")
CARGO_INSTALL = re.compile(r"\bcargo install\b")
CARGO_EXACT = re.compile(r"--version[= ]['\"]?=?\S*\d+\.\d+\.\d+")

# Inputs that select a tool version. Each must be present and exact.
REQUIRED_INPUTS = {
    "dtolnay/rust-toolchain": ["toolchain"],
    "actions/setup-node": ["node-version"],
    "foundry-rs/foundry-toolchain": ["version"],
    "crytic/slither-action": ["slither-version", "solc-version"],
    "model-checking/kani-github-action": ["kani-version"],
    "heyAyushh/setup-anchor": ["anchor-version", "solana-cli-version", "node-version"],
    "sigstore/cosign-installer": ["cosign-release"],
}

findings = []


def report(path, line, message):
    rel = path.relative_to(ROOT)
    findings.append(f"::error file={rel},line={line}::{message}")


def workflow_env(lines):
    env = {}
    for line in lines:
        m = re.match(r"^\s+([A-Z][A-Z0-9_]*):\s*['\"]?([^'\"#\s]+)", line)
        if m:
            env.setdefault(m.group(1), m.group(2))
    return env


def step_blocks(lines):
    """Yield (start_line_no, [lines]) for each `- ...` list item."""
    i = 0
    while i < len(lines):
        m = STEP_START.match(lines[i])
        if not m:
            i += 1
            continue
        indent = len(m.group(1))
        j = i + 1
        while j < len(lines):
            stripped = lines[j].strip()
            if stripped and len(lines[j]) - len(lines[j].lstrip()) <= indent:
                break
            j += 1
        yield i + 1, lines[i:j]
        i = j


def expand(text, env):
    text = re.sub(r"\$\{\{\s*env\.([A-Z0-9_]+)\s*\}\}", lambda m: env.get(m.group(1), m.group(0)), text)
    return re.sub(r"\$\{?([A-Z][A-Z0-9_]*)\}?", lambda m: env.get(m.group(1), m.group(0)), text)


def logical_lines(text_lines):
    """Join shell `\\` continuations, keeping the first physical line number."""
    buf, first = [], 0
    for offset, line in enumerate(text_lines):
        if not buf:
            first = offset
        if line.lstrip().startswith("#") and not buf:
            continue
        stripped = line.rstrip()
        if stripped.endswith("\\"):
            buf.append(stripped[:-1])
            continue
        buf.append(line)
        yield first, " ".join(part.strip() for part in buf)
        buf = []
    if buf:
        yield first, " ".join(part.strip() for part in buf)


def check_run_text(path, start, text_lines, env):
    block = "\n".join(text_lines)
    for offset, code in logical_lines(text_lines):
        code = expand(code, env)
        if PIPE_TO_SHELL.search(code) and re.search(r"\b(curl|wget)\b", code):
            report(path, start + offset, "network download piped into a shell; download, verify the checksum, then run")
        if CARGO_INSTALL.search(code):
            if "--locked" not in code:
                report(path, start + offset, "cargo install without --locked")
            if not CARGO_EXACT.search(code):
                report(path, start + offset, "cargo install without an exact --version")
        if ARCHIVE_DOWNLOAD.search(code) and "sha256sum -c" not in block:
            report(path, start + offset, "release archive downloaded without a sha256sum -c check")


def check_workflow(path):
    lines = path.read_text(encoding="utf-8").splitlines()
    env = workflow_env(lines)

    for no, line in enumerate(lines, 1):
        m = RUNS_ON.match(line)
        if m and m.group(1).endswith("-latest"):
            report(path, no, f"runs-on '{m.group(1)}' floats; use a fixed runner image")

    for start, block in step_blocks(lines):
        uses = None
        for offset, line in enumerate(block):
            m = USES.match(line)
            if m:
                uses, comment, uses_line = m.group(1), m.group(2), start + offset
                break
        if uses is None:
            check_run_text(path, start, block, env)
            continue

        if uses.startswith("./"):
            continue
        if uses.startswith("docker://"):
            if "@sha256:" not in uses:
                report(path, uses_line, f"{uses} is not pinned to an @sha256 digest")
            continue
        if not SHA_REF.match(uses):
            report(path, uses_line, f"{uses} is not pinned to a full commit SHA")
            continue
        if not comment:
            report(path, uses_line, f"{uses} needs a trailing '# <version>' comment")

        action = "/".join(uses.split("@", 1)[0].split("/")[:2])
        if action not in REQUIRED_INPUTS:
            continue
        inputs = {}
        for line in block:
            wm = WITH_KEY.match(line)
            if wm:
                inputs[wm.group(1)] = wm.group(2).strip()
        for key in REQUIRED_INPUTS[action]:
            value = inputs.get(key)
            if value is None:
                report(path, uses_line, f"{action} needs an explicit '{key}' input")
                continue
            em = ENV_REF.match(value)
            if em:
                value = env.get(em.group(1), "")
            if not EXACT_VERSION.match(value):
                report(path, uses_line, f"{action} '{key}: {value}' is not an exact version")


def check_dockerfile(path):
    lines = path.read_text(encoding="utf-8").splitlines()
    env = {}
    for line in lines:
        m = re.match(r"^\s*(?:ARG|ENV)\s+([A-Z][A-Z0-9_]*)=['\"]?([^'\"\s]+)", line)
        if m:
            env.setdefault(m.group(1), m.group(2))
    for no, line in enumerate(lines, 1):
        parts = line.split()
        if parts and parts[0].upper() == "FROM":
            image = next((p for p in parts[1:] if not p.startswith("--")), "")
            if image != "scratch" and "@sha256:" not in image:
                report(path, no, f"base image '{image}' is not pinned to an @sha256 digest")
    check_run_text(path, 1, lines, env)


CRATES_IO = {
    "registry+https://github.com/rust-lang/crates.io-index",
    "sparse+https://index.crates.io/",
}


def check_lockfile(path):
    lines = path.read_text(encoding="utf-8").splitlines()
    for no, line in enumerate(lines, 1):
        if path.name == "Cargo.lock":
            m = re.match(r'^source = "([^"#]+)', line)
            if m and m.group(1) not in CRATES_IO:
                report(path, no, f"crate resolved from '{m.group(1)}', not crates.io")
        elif path.name == "package-lock.json":
            m = re.search(r'"resolved":\s*"([^"]+)"', line)
            if m and not m.group(1).startswith("https://registry.npmjs.org/"):
                report(path, no, f"package resolved from '{m.group(1)}', not registry.npmjs.org")
        elif path.name == "pnpm-lock.yaml":
            if re.search(r"\btarball:|git\+|codeload\.github\.com", line):
                report(path, no, "package resolved from a tarball or git URL, not registry.npmjs.org")


def tracked(pattern):
    out = subprocess.run(
        ["git", "ls-files", "--", pattern], cwd=ROOT, check=True, capture_output=True, text=True
    ).stdout
    return [ROOT / p for p in out.splitlines() if p]


def main():
    workflows = sorted((ROOT / ".github" / "workflows").glob("*.y*ml"))
    dockerfiles = sorted(set(tracked("*Dockerfile*")))
    for path in workflows:
        check_workflow(path)
    for path in dockerfiles:
        check_dockerfile(path)
    lockfiles = sorted(
        set(tracked("*Cargo.lock") + tracked("*package-lock.json") + tracked("*pnpm-lock.yaml"))
    )
    for path in lockfiles:
        check_lockfile(path)

    if findings:
        print("\n".join(findings))
        print(f"\n{len(findings)} supply-chain pinning violation(s). See SUPPLY_CHAIN.md.")
        return 1
    print(
        f"Checked {len(workflows)} workflows, {len(dockerfiles)} Dockerfiles and "
        f"{len(lockfiles)} lockfiles: all inputs pinned."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
