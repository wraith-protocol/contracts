# Supported Stellar TypeScript bindings

`contracts.json` is the single inventory for package generation, ABI snapshots,
the aggregate exports and package validation. Every Cargo workspace member must
be included or explicitly excluded; an unclassified new member fails the gate.

| Package              | Barrel export      | Purpose                                                               |
| -------------------- | ------------------ | --------------------------------------------------------------------- |
| stealth-announcer    | StealthAnnouncer   | Announcement events                                                   |
| stealth-registry     | StealthRegistry    | Stealth meta-address registration                                     |
| stealth-sender       | StealthSender      | Atomic send and announcement                                          |
| stealth-batch-sender | StealthBatchSender | Batch sends                                                           |
| wraith-names         | WraithNames        | Name registration and resolution                                      |
| stealth-splitter     | StealthSplitter    | Split payments                                                        |
| stealth-vault        | StealthVault       | Time-locked deposits, claims and refunds                              |
| wraith-asset-policy  | WraithAssetPolicy  | Asset policy management                                               |
| governance           | Governance         | Token-weighted governance; proof of concept, **not production ready** |

## Deliberate exclusions

- `wraith-metrics`: shared event-emission/types library with no contract entry points
  and no `cdylib` WASM artifact; its types are included in the specs of consuming contracts.
- `bench`: host-only resource benchmark harness enabling SDK test utilities.
- `bench-crossover`: host-only crossover benchmark harness.
- `integration-tests`: host-only test harness.

The exclusions and their reasons are also machine-readable in `contracts.json`.
Governance lives in `stellar/contracts/governance` and produces `governance.wasm`.

## Regeneration and drift checks

Use the reviewed Rust 1.98.1 and Stellar CLI 26.1.0 pins from `SUPPLY_CHAIN.md`,
with `wasm32-unknown-unknown` installed. From the repository root:

```sh
pnpm install --frozen-lockfile
pnpm bindings:stellar
pnpm check:stellar-bindings
pnpm test:stellar-bindings
pnpm validate:stellar-bindings
```

Generation always rebuilds current local WASM with the committed Cargo lockfile;
ordinary generation and `--check` do not read deployed IDs or contact an RPC.
Each package contains its full generated client/spec and explicit `Client` export.
The generator normalizes Buffer portability, package metadata and line endings.
Reserved ABI argument names (such as governance `function`) retain their public
object keys but use a plain typed parameter instead of invalid destructuring.
`--check` generates in a private temporary directory and compares every source,
metadata file and ABI snapshot, detecting missing, stale and extra artifacts.
`stellar/abi/update.sh` updates the same complete ABI set without rewriting packages.

For an intentional deployment-specific export, use `pnpm bindings:stellar --deployed`
and provide IDs for every supported contract via `stellar/contract-ids.json` or
the inventory's environment variables. This opt-in output is not the local CI baseline.
`STELLAR_CLI` may name an absolute path to the pinned CLI executable.

## Package consumers

Each package has `npm run build` and `npm run check`, publishes only `dist/`
plus its package metadata/README, and exposes JS and declaration entry points.
After installation, import `{ Client }` from the package name and pass the
network's `contractId`, `networkPassphrase` and `rpcUrl` explicitly.

`validate:stellar-bindings` copies every package into a clean temporary build
fixture, installs declared dependencies with the reviewed fixture lockfile, builds and type-checks it, and packs
real npm tarballs. A separate clean consumer installs those tarballs, type-checks
all public imports and imports every `Client` under Node. Neither fixture resolves
dependencies from this repository's `node_modules`. Both use `npm ci`, retaining
the pinned registry dependency graph while adding only the locally built tarballs;
lifecycle scripts are disabled.
