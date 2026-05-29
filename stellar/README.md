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
