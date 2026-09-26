# Supply Chain Policy

Everything that CI and the release build download is pinned to a reviewed
version, and the pin is checked on every push and pull request. Anything not
listed here is not allowed into CI or a release. This document lists the
pinned inputs, how to bump them, and how the release toolchain is recorded.

## Rules

Enforced by `.github/scripts/check_supply_chain_pins.py` (the `Supply Chain / pins` job):

| Input                                 | Rule                                                                                      |
| ------------------------------------- | ----------------------------------------------------------------------------------------- |
| GitHub Actions (`uses:`)              | Full 40-character commit SHA plus a `# vX.Y.Z` comment. Local `./` actions are exempt.    |
| Runner images (`runs-on:`)            | Fixed image (`ubuntu-24.04`), never `*-latest`.                                           |
| Toolchain actions                     | Explicit exact version input (see the inventory below). No `stable`, `nightly`, `latest`. |
| `cargo install`                       | `--locked` and an exact `--version`.                                                      |
| Downloaded release archives           | Verified with `sha256sum -c` before use.                                                  |
| `curl ... \| sh`                      | Not allowed. Download, verify, then execute.                                              |
| Dockerfile `FROM`                     | Tag plus `@sha256:` index digest.                                                         |
| `Cargo.lock`                          | Every crate resolves from crates.io.                                                      |
| `package-lock.json`, `pnpm-lock.yaml` | Every package resolves from registry.npmjs.org. No git or tarball URLs.                   |

Dependency changes are reviewed by `.github/scripts/dependency_review.py` (the
`Supply Chain / dependency-review` job, pull requests only). It diffs every
tracked `Cargo.lock`, `package-lock.json` and `pnpm-lock.yaml` between the base
and head commits, lists added and removed packages in the job summary, and
queries [OSV](https://osv.dev) for advisories against the newly introduced
versions. MODERATE, HIGH, CRITICAL and unrated advisories fail the job; LOW and
informational (RustSec "unmaintained") advisories are warnings. It does not
depend on the repository dependency graph, so it works on forks.

Lockfile integrity is checked by the `Supply Chain / lockfiles` job:
`pnpm install --frozen-lockfile`, `npm ci` followed by `npm audit signatures`
(registry signatures and provenance attestations), and `cargo metadata --locked`
for each Cargo workspace.

## Inventory

### CI toolchains

Declared once per workflow in a top-level `env:` block headed
`# Reviewed toolchain pins`.

| Tool                    | Pin                                                     | Where                                                                        | Verified by                                       |
| ----------------------- | ------------------------------------------------------- | ---------------------------------------------------------------------------- | ------------------------------------------------- |
| Rust (stable)           | `1.98.1`                                                | `ci.yml`, `coverage.yml`, `sac-smoke.yml`, `integration-futurenet.yml`       | rustup channel manifest signatures                |
| Rust (nightly, fuzz)    | `nightly-2026-09-01`                                    | `ci.yml`                                                                     | rustup channel manifest signatures                |
| Rust (Solana)           | `1.91.0` action / `1.89.0` `solana/rust-toolchain.toml` | `ci.yml`                                                                     | rustup channel manifest signatures                |
| Node.js                 | `22.23.2`                                               | `ci.yml`, `audit-freeze.yml`, `stellar-verification.yml`, `supply-chain.yml` | `actions/setup-node` release checksums            |
| pnpm                    | `10.28.2`                                               | same as Node.js, and `packageManager` in `package.json`                      | corepack npm registry signature                   |
| stellar-cli (CI)        | `22.0.1`                                                | `ci.yml` (`STELLAR_CLI_SHA256`)                                              | `sha256sum -c` against the pinned hash            |
| stellar-cli (Futurenet) | `28.0.0`                                                | `integration-futurenet.yml`                                                  | `cargo install --locked` (crates.io checksums)    |
| Foundry                 | `v1.8.3`                                                | `ci.yml`                                                                     | `foundry-rs/foundry-toolchain`                    |
| slither-analyzer        | `0.11.4`, solc `0.8.28`                                 | `ci.yml`                                                                     | PyPI via `crytic/slither-action`                  |
| Kani                    | `0.68.0`                                                | `ci.yml`                                                                     | crates.io via `model-checking/kani-github-action` |
| Anchor / Solana CLI     | `0.30.1` / `1.18.17`, Node `20.15.0`                    | `ci.yml`                                                                     | `heyAyushh/setup-anchor`                          |
| cargo-fuzz              | `0.12.0`                                                | `ci.yml`                                                                     | `cargo install --locked`                          |
| cargo-tarpaulin         | `0.37.4`                                                | `coverage.yml`                                                               | `cargo install --locked`                          |
| cosign                  | `v2.2.4`                                                | `stellar-attestation.yml`                                                    | `sigstore/cosign-installer` checksum              |

### Release build (Stellar reproducible build)

| Input       | Pin                                                                                                     | Where                               |
| ----------- | ------------------------------------------------------------------------------------------------------- | ----------------------------------- |
| Base image  | `debian:bookworm-20260918-slim@sha256:3783cc01769c7b2b1b83a5c5ad96c815348e28ed7da68e2e3687004faa906251` | `stellar/build/Dockerfile`          |
| rustup-init | `1.29.1`, `sha256:dda7234360b7f578ca8b0ddcb80145646fa61a67c1720a5abc7051b35c9fcb71`                     | `stellar/build/Dockerfile`          |
| Rust        | `1.88.0`, target `wasm32-unknown-unknown`                                                               | `stellar/build/rust-toolchain.toml` |
| stellar-cli | `22.0.0`, `cargo install --locked`, compiled with the Rust pin above                                    | `stellar/build/Dockerfile`          |
| soroban-sdk | as resolved in `stellar/Cargo.lock`                                                                     | `stellar/Cargo.lock`                |

The release build pins an older Rust than CI on purpose: changing the compiler
changes the WASM bytes, which changes the hashes that `stellar/build/verify.js`
compares against deployed contracts. Bump it only as part of a planned
redeploy.

### Actions

Pinned by SHA in every workflow. Dependabot proposes bumps weekly
(`.github/dependabot.yml`).

| Action                                 | Version | SHA                                        |
| -------------------------------------- | ------- | ------------------------------------------ |
| `actions/cache`                        | v4.3.0  | `0057852bfaa89a56745cba8c7296529d2fc39830` |
| `actions/checkout`                     | v4.4.0  | `11d5960a326750d5838078e36cf38b85af677262` |
| `actions/configure-pages`              | v5.0.0  | `983d7736d9b0ae728b81ab479565c72886d7745b` |
| `actions/deploy-pages`                 | v4.0.5  | `d6db90164ac5ed86f2b6aed7e0febac5b3c0c03e` |
| `actions/setup-node`                   | v4.4.0  | `49933ea5288caeca8642d1e84afbd3f7d6820020` |
| `actions/upload-artifact`              | v4.6.2  | `ea165f8d65b6e75b540449e92b4886f43607fa02` |
| `actions/upload-pages-artifact`        | v3.0.1  | `56afc609e74202658d3ffba0e8f6dda462b719fa` |
| `crytic/slither-action`                | v0.4.0  | `f197989dea5b53e986d0f88c60a034ddd77ec9a8` |
| `dawidd6/action-download-artifact`     | v6      | `bf251b5aa9c2f7eeb574a96ee720e24f801b7c11` |
| `docker/setup-buildx-action`           | v3.12.0 | `8d2750c68a42422c14e847fe6c8ac0403b4cbd6f` |
| `dorny/paths-filter`                   | v3.0.4  | `0e4a8c6effa4802afeda77dc8d303f8176d7dfad` |
| `dtolnay/rust-toolchain`               | master  | `02cb101ec7c40f2c49e1d9714d64511d8e1b74de` |
| `foundry-rs/foundry-toolchain`         | v1.9.1  | `908c540300062bd5a7e473851cdb4282204cee09` |
| `github/codeql-action/upload-sarif`    | v3.38.2 | `1190a975f95ce23525efb6a3fc21ea29567c1b52` |
| `heyAyushh/setup-anchor`               | v4.2    | `039028505349e846df1e57a5691d6d744474ae9b` |
| `model-checking/kani-github-action`    | v1      | `69d357bade1eb7b32bc3fc3d5c3d173c5bfa5237` |
| `sigstore/cosign-installer`            | v3.5.0  | `59acb6260d9c0ba8f4a2f9d9b48431a222b68e20` |
| `softprops/action-gh-release`          | v2.6.2  | `3bb12739c298aeb8a4eeaf626c5b8d85266b0e65` |
| `stefanzweifel/git-auto-commit-action` | v5.2.0  | `b863ae1933cb653a53c021fe36dbb774e1fb9403` |

## Release toolchain record

Every Stellar release carries `attestation.json`, produced inside the pinned
container by `stellar/build/build.sh` and signed with cosign by
`stellar-attestation.yml`. Its `toolchain` object records what built the
artifacts:

```json
"toolchain": {
  "rust": "1.88.0",
  "cargo": "1.88.0",
  "rustup": "1.29.1",
  "stellar-cli": "22.0.0",
  "soroban-sdk": "22.0.11",
  "target": "wasm32-unknown-unknown",
  "base_image": "debian:bookworm-20260918-slim@sha256:3783cc01...",
  "cargo_lock_sha256": "<sha256 of stellar/Cargo.lock>"
}
```

To confirm a release, rebuild from its commit (see
[`audit-prep/05-reproducible-builds.md`](./audit-prep/05-reproducible-builds.md)) and
compare both the WASM hashes and the `toolchain` object.

## Updating a pin

1. **Open a dedicated PR.** One tool or action family per PR, titled
   `chore(deps): bump <tool> <old> -> <new>`. Dependabot PRs already follow this.
2. **Read what changed.** Release notes and the diff between the old and new
   SHA or tag. For an action, check that the new SHA is reachable from the
   upstream tag (`git ls-remote https://github.com/<owner>/<repo> refs/tags/<tag>`)
   and not from a fork. Look for new network calls, new permissions, or new
   transitive actions.
3. **Update the pin and its companions together:**
   - Action: the SHA and the `# vX.Y.Z` comment.
   - Toolchain: the `env:` value in _every_ workflow that declares it
     (`grep -rn '<NAME>:' .github/workflows`).
   - Downloaded binary: the version and its `*_SHA256`. Compute the hash from
     the official release asset. If upstream publishes checksums or build
     provenance (`gh attestation verify`), check against those as well.
   - Base image: the dated tag and the digest together
     (`docker buildx imagetools inspect <image>:<tag>`).
4. **Update the inventory in this file** in the same PR.
5. **CI must pass**, including `Supply Chain / pins`.
6. **Maintainer review.** `.github/` is owned by `@truthixify` via CODEOWNERS.
   Pin bumps need that approval before merge.

## Re-audit triggers

A pin bump is routine. The following need a full re-audit, recorded in the PR
description:

- **Release build inputs** (`stellar/build/**`, the release Rust toolchain,
  stellar-cli in the Dockerfile, the base image). Rebuild twice from a clean
  checkout, confirm identical WASM hashes, and state in the PR whether hashes
  moved against the last release `attestation.json`. Any movement requires a
  planned redeploy.
- **A new third-party action or tool.** Justify it, confirm the maintainer and
  the repository, prefer first-party (`actions/*`) alternatives, and add it to
  the inventory.
- **A new dependency source** (git dependency, non-default registry). The pins
  check rejects these; allowing one is a policy change to this document.
- **A security advisory** against any pinned input. Bump to a fixed version, or
  record why it is not reachable.
- **Quarterly**, even with no advisories. Walk the inventory, bump anything
  more than one minor version behind, and re-verify the release build.

## Exceptions

If an introduced package has an advisory that is not reachable (for example a
dev-only CLI in the subgraph toolchain), add its id to
`.github/supply-chain/osv-allowlist.txt` with a comment giving the reason, the
reviewer, and a revisit date. Exceptions need the same CODEOWNERS approval as
any other `.github/` change.

## Known gaps

- `apt-get` packages in `stellar/build/Dockerfile` and the CKB job come from
  the Debian and Ubuntu archives at build time. They are signed by the
  distribution but not version-pinned. The contract WASM does not link against
  them.
- `model-checking/kani-github-action` and `heyAyushh/setup-anchor` call further
  actions by moving tag internally. We pin the outer action. Upgrading those
  inner actions requires bumping the outer SHA.
- stellar-cli `22.0.1` publishes neither checksums nor build provenance. The
  pinned hash was taken from the GitHub release asset on first use.
- The repository dependency graph is not enabled. Enabling it (Settings → Code
  security) would additionally allow `actions/dependency-review-action` for
  license policy. The OSV review above does not require it.
