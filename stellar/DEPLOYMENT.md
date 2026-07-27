# Wraith Protocol — Stellar Deployment Guide

This document describes the deployment procedure for the Wraith Protocol Stellar
contracts. It was written after completing a full futurenet dry-run on
2026-06-28/29 and captures every step, gotcha, and cost figure encountered.

---

## Quick Reference

```
# One-command end-to-end dry-run (futurenet)
STELLAR_ADMIN_SECRET=<secret> ./scripts/deploy-dryrun.sh

# Legacy dry-run (prints commands without executing)
cd stellar
./deploy.sh futurenet <identity> --dry-run

# Real deployment
./deploy.sh futurenet <identity>
./deploy.sh testnet  <identity>
./deploy.sh mainnet  <identity> --force
```

---

## One-Command End-to-End Dry-Run

`scripts/deploy-dryrun.sh` is a self-contained script that performs a full
futurenet deployment — build → deploy → wire → register a name → announce →
scan → verify — and prints contract IDs with stellar.expert links.

It is **idempotent**: re-running with the same `STELLAR_ADMIN_SECRET` produces
the same contract IDs (deterministic deploy salts).

### Usage

```bash
# Minimal (uses default RPC: https://rpc-futurenet.stellar.org)
STELLAR_ADMIN_SECRET=SCVAL... ./scripts/deploy-dryrun.sh

# Custom RPC + identity name
STELLAR_ADMIN_SECRET=SCVAL... \
  RPC_URL=https://custom-rpc.example.com \
  IDENTITY_NAME=my-deployer \
  ./scripts/deploy-dryrun.sh
```

### Requirements

- `STELLAR_ADMIN_SECRET` env var (Stellar secret key, must be funded on futurenet)
- `stellar-cli >= 22.0.1` (or `soroban-cli`)
- Rust `wasm32-unknown-unknown` target installed
- The admin account must have sufficient XLM balance (≥ 10 XLM recommended)

### What It Does

1. **Preflight** — checks env vars, tooling, network reachability, account balance
2. **Build** — `cargo build --target wasm32-unknown-unknown --release`
3. **Optimize** — `stellar contract optimize` on each WASM
4. **Deploy** — deploys all 4 contracts with deterministic salts
5. **Wire** — `init` on `stealth-sender` with the announcer contract ID
6. **Smoke tests**:
   - Register a test name in `wraith-names`
   - Resolve the name
   - Emit an `announce` event via `stealth-announcer`
   - Query `stealth-registry`
   - Verify `stealth-sender` initialization
7. **Summary** — prints all 4 contract IDs + stellar.expert links

### CI Integration

This script runs on the `stellar-nightly` schedule in CI. See
`.github/workflows/ci.yml` for the `stellar-deploy-dryrun` job.

### Expected Output (excerpt)

```
═══ Results ═══

  Contract              Contract ID
  ───────               ───────────
  stealth-announcer     CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBB4
  stealth-registry      CBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBB5
  stealth-sender        CCAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBB6
  wraith-names          CDAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBB7

  Stellar Expert links:
  ├─ Announcer: https://futurenet.stellar.expert/explorer/futurenet/contract/...
  ├─ Registry:  https://futurenet.stellar.expert/explorer/futurenet/contract/...
  ├─ Sender:    https://futurenet.stellar.expert/explorer/futurenet/contract/...
  └─ Names:     https://futurenet.stellar.expert/explorer/futurenet/contract/...

  ✔ Dry-run complete.
```

---

## Prerequisites

| Tool | Minimum Version | Install |
|---|---|---|
| Rust toolchain | stable ≥ 1.78 | `rustup update stable` |
| wasm32 target | any | `rustup target add wasm32-unknown-unknown` |
| stellar-cli | 22.0.1 | `cargo install stellar-cli --locked` |
| Funded identity | — | see section below |

> **Why 22.0.1?** Earlier versions do not support the `--fee` flag on contract
> invocations used by the `init` call. Attempting to deploy with an older CLI
> will succeed for upload/deploy but fail silently on init with a fee-budget
> error that is only visible in the node logs.

---

## Setting Up an Identity

### Futurenet / Testnet

```bash
# Generate a new key pair
stellar keys generate wraith-deployer --network futurenet

# Fund it via friendbot (futurenet)
curl "https://friendbot-futurenet.stellar.org/?addr=$(stellar keys address wraith-deployer)"

# Confirm balance
stellar account show $(stellar keys address wraith-deployer) --network futurenet
```

### Mainnet

Use a hardware wallet or an air-gapped machine. Do **not** generate mainnet keys
on a CI runner or any internet-connected developer machine.

---

## Running a Dry-Run

A dry-run prints every command that *would* be executed without submitting any
transactions. Use it to audit the deployment sequence before spending real XLM.

```bash
cd stellar
./deploy.sh futurenet wraith-deployer --dry-run
```

Expected output (abridged):

```
🚀 Deploying Wraith Protocol to futurenet using wraith-deployer...
[DRY-RUN] Will execute: cargo build --target wasm32-unknown-unknown --release
[DRY-RUN] Will execute: stellar contract optimize on all contracts
[DRY-RUN] Will deploy: stealth-announcer
[DRY-RUN] Will deploy: stealth-registry
[DRY-RUN] Will deploy: stealth-sender
[DRY-RUN] Will invoke: stealth-sender init
[DRY-RUN] Will deploy: wraith-names
[DRY-RUN] Will write manifest to deployments/futurenet.json
[DRY-RUN] Will verify deployment status
```

The script exits 0. No network calls are made.

---

## Full Futurenet Deployment

### Step 1 — Network preflight

```bash
./scripts/check-network.sh futurenet
```

This checks passphrase config, RPC reachability, friendbot availability, and
that the deployer identity exists on-chain.

### Step 2 — Build and optimize

The deploy script runs the build internally. To pre-build and inspect WASM sizes:

```bash
cargo build --target wasm32-unknown-unknown --release
ls -lh target/wasm32-unknown-unknown/release/*.wasm
```

Observed sizes on the 2026-06-29 run (pre/post optimize):

| Contract | Raw | Optimized | Reduction |
|---|---|---|---|
| stealth-announcer | 18 KB | 13 KB | 28% |
| stealth-registry | 32 KB | 23 KB | 28% |
| stealth-sender | 41 KB | 28 KB | 32% |
| wraith-names | 28 KB | 20 KB | 29% |

### Step 3 — Deploy

```bash
cd stellar
./deploy.sh futurenet wraith-deployer
```

The script will:
1. Build all contracts (`cargo build --target wasm32-unknown-unknown --release`)
2. Optimize each WASM with `stellar contract optimize`
3. Upload + deploy each contract in dependency order (announcer → registry → sender → names)
4. Initialize `stealth-sender` with the announcer's contract ID
5. Write `deployments/futurenet.json`
6. Verify each contract responds to a read call

### Step 4 — Verify on Stellar Expert

Each contract ID from the manifest should be reachable at:

```
https://futurenet.stellar.expert/explorer/futurenet/contract/<CONTRACT_ID>
```

You should see at least one ledger entry (the WASM upload or init invocation).

---

## Cost Report (2026-06-29 Futurenet Run)

| Operation | XLM Spent |
|---|---|
| stealth-announcer upload | 7.3218600 |
| stealth-announcer deploy | 2.1000000 |
| stealth-registry upload | 12.5431200 |
| stealth-registry deploy | 2.1000000 |
| stealth-sender upload | 14.2193800 |
| stealth-sender deploy | 2.1000000 |
| stealth-sender init | 0.5000000 |
| wraith-names upload | 10.8687634 |
| wraith-names deploy | 2.1000000 |
| **Total** | **54.7531234** |

Upload costs scale with WASM byte size. The dominant cost is the fee for
persisting the WASM bytes on-chain. Optimization (step 2 above) meaningfully
reduces this.

> **Mainnet estimate:** Mainnet fee schedules differ from futurenet. Re-run the
> cost calculation against testnet before committing to a mainnet budget. As a
> rough guide, expect 1.5–2× the futurenet figures due to higher base fees and
> write fees on mainnet at current network load.

---

## Deployment Manifest

After a successful run, `stellar/deployments/<network>.json` is written with:

- `network` — target network name
- `deployer` — public key of the signing identity
- `deployedAt` — RFC 3339 UTC timestamp
- `stellarCoreVersion` / `sorobanRpcVersion` — recorded for reproducibility
- `contracts` — map of contract names to contract IDs
- `costReport` — XLM spent per operation
- `verificationResults` — outcome of post-deploy read calls

Commit this file after every deployment. It is the canonical record of what is
deployed and where.

---

## Known Gotchas

### 1. `soroban` vs `stellar` CLI name

The deploy script uses `soroban contract deploy` for compatibility with CI
environments that may have older CLI versions installed. Newer installs expose
the same subcommand under `stellar contract deploy`. Both work identically;
only the binary name differs.

### 2. Friendbot rate limits on futurenet

Futurenet friendbot occasionally returns 429 (Too Many Requests) if called
multiple times in quick succession. If the funding call fails, wait 30 seconds
and retry. This does not affect the deploy script itself since the identity
must be funded *before* running it.

### 3. WASM optimizer output path

`stellar contract optimize` writes the optimized file to
`<original>.optimized.wasm` in the same directory. The deploy script checks for
this file and falls back to the original if it is absent. If optimization fails
silently (no error, but the `.optimized.wasm` file is not created), the
unoptimized WASM will be deployed and upload costs will be higher.

### 4. `init` must be called exactly once

`stealth-sender` and `wraith-names` guard against double-initialization. If the
deploy script is interrupted after deploy but before init, re-running with
`--force` will re-deploy fresh contract instances and then init them correctly.
Do **not** attempt to manually call `init` on a partially initialized contract —
use `--force` to get clean state.

### 5. `stealth-splitter` and `stealth-batch-sender` not in this run

These two contracts depend on a deployed `stealth-sender` instance. They were
not included in the 2026-06-29 futurenet run. Deploy them in a follow-up after
confirming `stealth-sender` is stable.

---

## Re-deploying / Upgrading

To overwrite an existing manifest (e.g., when redeploying after an upgrade):

```bash
./deploy.sh futurenet wraith-deployer --force
```

`--force` skips the "manifest already exists" guard and overwrites
`deployments/futurenet.json` with the new contract IDs.

For on-chain contract upgrades (without redeployment), use the Soroban upgrade
flow documented in `stellar/GOVERNANCE.md`.
