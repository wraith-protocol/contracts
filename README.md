# Wraith Protocol Contracts

Stealth address smart contracts for the [Wraith Protocol](https://github.com/wraith-protocol) multichain privacy platform. EVM contracts in Solidity (Hardhat), Stellar contracts in Soroban/Rust, Solana programs in Anchor/Rust, CKB scripts in Rust (RISC-V).

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

## CKB Scripts (Rust/RISC-V)

| Script | Description |
|---|---|
| **wraith-stealth-lock** | Lock script that verifies secp256k1 signatures against `blake160(stealth_pubkey)` in `args[33:53]`. Embeds the ephemeral public key in `args[0:33]` — the Cell itself is the announcement. Delegates to on-chain ckb-auth via `exec_cell`. |
| **wraith-names-type** | Type script for `.wraith` name registration cells. Validates 66-byte cell data (spending + viewing public keys). Ownership proven by the cell's lock script. Supports create, update, and release (destroy). |

## Getting Started

### Prerequisites

- Node.js 22+
- Rust toolchain with `cargo`
- Anchor CLI and Solana CLI (for Solana programs)
- `riscv64-elf-gcc` cross-compiler (for CKB scripts)

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

### CKB

```bash
cd ckb
make build
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
ckb/
  contracts/
    wraith-stealth-lock/  # CKB lock script (RISC-V)
    wraith-names-type/    # CKB type script (RISC-V)
  testnet.toml            # Deployed code hash and cell deps
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

### CKB Testnet

| Script | Code Hash | Cell Dep |
|---|---|---|
| wraith-stealth-lock | `0x31f6ab9c7e7a26ecba980b838ac3b5bd6c3a2f1b945e75b7cf7e6a46cb19cb87` | `0xde1e8e4bed2d1d7102b9ad3d7a74925ace007800ae49498f9c374cb4968dd32b:0` |
| wraith-names-type | `0xc133817d433f72ea16a2404adaf961524e9572c8378829a21968710d6182e20d` | `0x9acd640d35eadd893b358dddd415f4061fe81cb249e8ace51a866fee314141b8:0` |
| ckb-auth (dependency) | `0x0915983bb31584df4566e0946fd00ef1e9a75ad37a39ce70fec9b5cbf3b87021` | `0xa0e99b29fd154385815142b76668d5f4ecf30ae85bc2942bd21e9e51b9066f97:0` |

## License

MIT
