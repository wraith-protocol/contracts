# Stellar Contracts — Wraith Protocol

This directory contains the Soroban smart contracts for the Wraith multichain stealth address platform.

## Contracts

- `stealth-announcer`: Emits announcement events for stealth payments. [README](./stealth-announcer/README.md)
- `stealth-batch-sender`: Atomically sends tokens to multiple stealth addresses in a single transaction. [README](./stealth-batch-sender/README.md)
- `stealth-registry`: Maps addresses to 64-byte stealth meta-addresses. [README](./stealth-registry/README.md)
- `stealth-sender`: Handles atomic transfers and announcements. [README](./stealth-sender/README.md)
- `stealth-vault`: Time-locked vault for stealth payments with refund safety net. [README](./stealth-vault/README.md)
- `stealth-splitter`: 1-to-N stealth payment splitter. [README](./stealth-splitter/README.md)
- `wraith-asset-policy`: Admin-controlled asset allowlist for stealth payments. [README](./wraith-asset-policy/README.md)
- `wraith-metrics`: Shared metrics library for standardized event emission. [README](./wraith-metrics/README.md)
- `wraith-names`: Privacy-preserving name registry for `.wraith` names. [README](./wraith-names/README.md)
- `contracts/governance`: Token-weighted governance (PoC, not production ready). [README](./contracts/governance/README.md)

See [`ERRORS.md`](./ERRORS.md) for the Stellar contract error-code catalog and allocation policy.

## Prerequisites

- [Rust](https://rustup.rs/) and `wasm32-unknown-unknown` target.
- [Soroban CLI](https://soroban.stellar.org/docs/getting-started/setup#install-the-soroban-cli).

## Build

To build all contracts:

```bash
cargo build --target wasm32-unknown-unknown --release
```

## Test

To run tests for all contracts:

```bash
cargo test
```

## Operations

### Coverage

The workflow in [.github/workflows/coverage.yml](../.github/workflows/coverage.yml) runs `cargo tarpaulin` for all nine Stellar crates and uploads the combined HTML report as a workflow artifact.

- Latest coverage run: [GitHub Actions workflow](https://github.com/wraith-protocol/contracts/actions/workflows/coverage.yml)
- Coverage artifact: downloaded from the Actions run page under the `stellar-coverage-html` artifact

## Deployment

A deployment script is provided to deploy all contracts in one go.

### 1. Configure Identities and Networks

```bash
# Add an identity (if not already done)
soroban config identity add --secret-key my-deployer

# Add a network (if not already done)
soroban config network add --rpc-url https://soroban-testnet.stellar.org:443 --network-passphrase "Test SDF Network ; September 2015" testnet
```

### 2. Run Deployment Script

```bash
./deploy.sh testnet my-deployer
```

The script will deploy all contracts and initialize the `stealth-sender` with the `stealth-announcer` ID.

## Asset Allowlist Policy

To protect the unlinkability and user experience of stealth transfers (preventing clawback-enabled or freeze-enabled assets from being sent, as identified in audit #43), `stealth-sender` supports an optional, configurable on-chain `asset_policy` check.

If an `asset_policy` contract address is supplied during `init`, the `stealth-sender` contract calls it before every transfer to ensure the asset is allowed.

### Custom Policy Interface

Any contract can act as an asset policy as long as it implements the following method:

```rust
pub fn check_asset(env: Env, asset: Address) -> bool;
```

- **asset**: The contract address of the Stellar Asset Contract (SAC) being checked.
- **Returns**: `true` if the asset is allowed for stealth payments, or `false` otherwise. If `false` is returned, `stealth-sender` rejects the transaction with `SenderError::TokenNotAllowed`.

### Reference Implementation

The `wraith-asset-policy` contract provides a default implementation that is controlled by an admin. The admin can add or remove assets from a persistent allowlist. If a caller wants custom rules (such as check-free transfers, or automated query-based enforcement), they can deploy their own contract matching the interface above.

## Protocol Fee Mechanism

To sustain hosted infrastructure costs, `stealth-sender` includes an optional protocol fee mechanism.

### Configuration

During contract initialization (`init`), the deployer can configure:
- `fee_recipient`: `Option<Address>` (The designated address that receives the protocol fee).
- `fee_basis_points`: `u32` (Fee percentage in basis points. Capped at a maximum of `50` basis points (0.5%) by contract invariant).

If both fields are zero (`None` and `0`), the fee mechanism is disabled (default permissionless behavior).

### Behavior

When active, for each `send` or `batch_send`:
1. `fee = amount * fee_basis_points / 10000` is computed.
2. `fee` is transferred atomically to the `fee_recipient` (if `fee > 0`).
3. `amount - fee` is transferred to the recipient's `stealth_address`.
4. In `batch_send`, the individual fees are calculated per recipient, and a single aggregated fee transfer is executed to the `fee_recipient` at the end to minimize gas costs.

## Storage Entry Recovery Tooling

In the Stellar Soroban smart contract network, ledger entries (including contract instances, WASM bytecode, and contract data storage) have a Time-To-Live (TTL). When an entry's TTL expires, it is evicted from the active ledger state and moved into the **Archived State Tree**. 

To use an archived contract or access its archived data, the state must first be restored by submitting a transaction containing a `RestoreFootprint` operation.

This folder contains a CLI tool designed to simplify this process by:
1. Scanning historical events to discover all potential storage keys.
2. Checking the on-chain live/archived status of the contract instance, its WASM code, and all discovered keys.
3. Pre-computing restoration fees in XLM using transaction simulation.
4. Executing restoration transactions idempotently.

---

### Installation

Before using the tool, ensure you have installed the dependencies using `pnpm`:

```bash
cd stellar
pnpm install
```

---

### CLI Command Reference

Execute the script directly using `pnpm recover` or `npx ts-node scripts/recover-storage.ts`.

#### 1. `list-archived`
Surface live and archived contract data entries.

```bash
npx ts-node scripts/recover-storage.ts list-archived --contract-id <contract-id> [--network <network>] [--start-ledger <ledger>]
```

**Options:**
*   `-c, --contract-id <id>` (Required): The Soroban contract ID.
*   `-n, --network <network>` (Optional, default `futurenet`): The Stellar network (`futurenet`, `testnet`, `mainnet`).
*   `-r, --rpc-url <url>` (Optional): Custom RPC URL override.
*   `-w, --wasm-hash <hash>` (Optional): Manually specify Wasm hash if the instance is archived and StellarExpert is unreachable.
*   `-s, --start-ledger <ledger>` (Optional, default `1`): The ledger sequence to start scanning events from.

---

#### 2. `estimate-restore`
Pre-computes transaction fees (both network base fees and Soroban resource fees) in XLM before submitting the restore operation.

```bash
npx ts-node scripts/recover-storage.ts estimate-restore --contract-id <contract-id> [--network <network>] [--start-ledger <ledger>]
```

---

#### 3. `restore`
Restores all archived storage entries for the contract. This operation is idempotent; it acts as a no-op if all entries are already live.

```bash
npx ts-node scripts/recover-storage.ts restore --contract-id <contract-id> --secret-key <key> [--network <network>] [--start-ledger <ledger>]
```

**Options:**
*   `-k, --secret-key <key>` (Required): Secret key of the account paying restoration fees.

---

### Per-Contract Recovery Procedure

Our smart contracts (`stealth-registry`, `wraith-names`, `stealth-sender`) utilize `instance()` storage for state preservation. 

#### 1. `stealth-announcer`
*   **Storage Type:** Stateless (no contract storage).
*   **Recovery Scope:** If the contract instance or its Wasm code expires, the announcer cannot be called. Only the **Contract Instance** and **Contract Wasm Code** need to be restored.
*   **Procedure:** Run the `restore` subcommand. The tool will identify that the instance/code are archived and restore them.

#### 2. `stealth-sender`
*   **Storage Type:** `instance()` storage (storing the announcer's Address).
*   **Recovery Scope:** The announcer address is stored in the contract instance. Restoring the **Contract Instance** restores this setting automatically.
*   **Procedure:** Run the `restore` subcommand.

#### 3. `stealth-registry`
*   **Storage Type:** `instance()` storage (mapping registrant addresses and scheme IDs to meta-addresses).
*   **Recovery Scope:** In Soroban, all `instance()` storage keys share the lifecycle of the contract instance. Restoring the **Contract Instance** restores all registry entries.
*   **Procedure:** 
    1. Run `list-archived` to check if the instance is archived.
    2. Run `estimate-restore` to get the cost of recovery in XLM.
    3. Run `restore` with your secret key to bring the contract and all registered meta-addresses back to life.

#### 4. `wraith-names`
*   **Storage Type:** `instance()` storage (mapping name hashes to name entries, and meta-address hashes to name hashes).
*   **Recovery Scope:** All name entries and reverse entries are stored within the contract instance.
*   **Procedure:** Follow the same steps as `stealth-registry`. The CLI will verify the status of the contract instance and code, scan event logs to verify individual name entries, and restore them.





