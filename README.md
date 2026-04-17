# Wraith Protocol Contracts

Stealth address smart contracts for the [Wraith Protocol](https://github.com/wraith-protocol) multichain privacy platform. EVM contracts in Solidity (Hardhat), Stellar contracts in Soroban/Rust, Solana programs in Anchor/Rust.

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

## Solana Programs (Anchor/Rust)

| Program | Description |
|---|---|
| **wraith-announcer** | Stateless event emitter for stealth address announcements via Anchor `emit!()`. |
| **wraith-sender** | Atomic SOL transfer + announcement in one instruction. Also supports SPL token sends. |
| **wraith-names** | PDA-based name registry. Names are 3-32 chars (lowercase alphanumeric/hyphens), stored as PDA seeds. |

## Getting Started

### Prerequisites

- Node.js 22+
- Rust toolchain with `cargo`
- Anchor CLI and Solana CLI (for Solana programs)

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

### Solana

```bash
cd solana
anchor build
anchor test
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
solana/
  programs/
    wraith-announcer/ # Anchor program
    wraith-sender/    # Anchor program
    wraith-names/     # Anchor program
  tests/              # TypeScript tests
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

### Solana Devnet

| Program | Program ID |
|---|---|
| wraith-announcer | `9Ko7TuXHpLUH1ZsZWQEpeA9Tv7hX325ooWk5SD7Y9nuq` |
| wraith-sender | `E6J7GBSTjKbYANWjfTo5HfnXZ4Tg3LAasN7NrvCwn5Dq` |
| wraith-names | `4JrrQh5aK7iLvx6MgtEQk7K7X3SsWfTLxVJu1jXEwNjD` |

## License

MIT
