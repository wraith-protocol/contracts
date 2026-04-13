# Wraith Protocol Contracts

Stealth address smart contracts for the [Wraith Protocol](https://github.com/wraith-protocol) multichain privacy platform. EVM contracts in Solidity (Hardhat), Stellar contracts in Soroban/Rust.

Every payment generates a fresh one-time stealth address so on-chain observers cannot link sender, recipient, or transaction history.

## EVM Contracts (Solidity)

| Contract | Description |
|---|---|
| **ERC5564Announcer** | Minimal singleton that emits `Announcement` events per ERC-5564. No storage, no access control. |
| **ERC6538Registry** | Stealth meta-address registry per ERC-6538. Supports direct registration and delegated registration via EIP-712 signatures with replay-protected nonces. |
| **WraithSender** | Atomically transfers ETH or ERC-20 tokens to a stealth address and publishes an announcement in a single transaction. Supports batch sends and optional ETH gas tips. |
| **WraithNames** | Privacy-preserving `.wraith` name registry. Maps human-readable names to stealth meta-addresses with ownership proven via secp256k1 spending key signatures. |
| **WraithWithdrawer** | EIP-7702 delegation target for gas-sponsored stealth address withdrawals. A sponsor pays gas on behalf of the stealth address holder. |

## Stellar Contracts (Soroban/Rust)

| Contract | Description |
|---|---|
| **stealth-announcer** | Emits stealth address announcement events. No storage. |
| **stealth-registry** | Maps addresses to 64-byte stealth meta-addresses with auth-gated registration. |
| **stealth-sender** | Atomic token transfer + announcement via the announcer contract. Supports batch sends. |
| **wraith-names** | Name registry with SHA-256 hashed storage keys, reverse lookup, and lowercase alphanumeric validation (3-32 chars). |

## Getting Started

### Prerequisites

- Node.js 22+
- Rust toolchain with `cargo`

### EVM

```bash
cd evm
npm install
npx hardhat compile
npx hardhat test
```

### Stellar

```bash
cd stellar
cargo test --workspace
```

## Project Structure

```
evm/
  contracts/          # Solidity sources
  test/               # Hardhat + Chai tests
  scripts/deploy.ts   # Deployment script
stellar/
  stealth-announcer/  # Soroban contract
  stealth-registry/   # Soroban contract
  stealth-sender/     # Soroban contract
  wraith-names/       # Soroban contract
```

## Deployed Addresses

### Horizen Testnet

| Contract | Address |
|---|---|
| ERC5564Announcer | TBD |
| ERC6538Registry | TBD |
| WraithSender | TBD |
| WraithNames | TBD |
| WraithWithdrawer | TBD |

### Stellar Testnet

| Contract | Contract ID |
|---|---|
| stealth-announcer | TBD |
| stealth-registry | TBD |
| stealth-sender | TBD |
| wraith-names | TBD |

## License

MIT
