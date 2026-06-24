# Stellar Contracts — Wraith Protocol

This directory contains the Soroban smart contracts for the Wraith multichain stealth address platform.

## Contracts

- `stealth-announcer`: Emits announcement events for stealth payments.
- `stealth-registry`: Maps addresses to 64-byte stealth meta-addresses.
- `stealth-sender`: Handles atomic transfers and announcements.
- `wraith-names`: Privacy-preserving name registry for `.wraith` names.

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
