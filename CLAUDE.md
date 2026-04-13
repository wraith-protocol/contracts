# Wraith Protocol Contracts

You are building the smart contracts for the Wraith multichain stealth address platform. EVM contracts in Solidity, Stellar contracts in Soroban/Rust.

## What These Contracts Do

Four core contracts per chain:

| Contract | Purpose |
|---|---|
| **Announcer** | Emits stealth address announcement events (no storage, no access control) |
| **Registry** | Maps addresses to stealth meta-addresses (ERC-6538 on EVM) |
| **Sender** | Atomic asset transfer + announcement in one transaction |
| **Names** | `.wraith` name → meta-address mapping with spending key ownership |

Plus EVM-only:
| Contract | Purpose |
|---|---|
| **Withdrawer** | EIP-7702 gas-sponsored stealth address withdrawals |

## Reference Code

- `reference/horizen/` — Working EVM contracts (Solidity, Hardhat)
- `reference/stellar/` — Working Stellar contracts (Soroban/Rust)
- `reference/docs/07-smart-contracts.md` — Full contract specs
- `reference/docs/08-testing.md` — Test cases and expected results

## Implementation Steps

Commit after each step.

### Step 1 — EVM Scaffold

```
evm/
  package.json
  hardhat.config.ts
  tsconfig.json
  contracts/
  test/
  scripts/
    deploy.ts
```

Deps: `hardhat`, `@nomicfoundation/hardhat-toolbox`, `@openzeppelin/contracts`

Verify: `npx hardhat compile` succeeds.

### Step 2 — EVM Contracts

Port from `reference/horizen/contracts/contracts/`:

**ERC5564Announcer.sol** — Minimal singleton. One function `announce()` that emits:
```solidity
event Announcement(
    uint256 indexed schemeId,
    address indexed stealthAddress,
    address indexed caller,
    bytes ephemeralPubKey,
    bytes metadata
);
```

**ERC6538Registry.sol** — Stealth meta-address registry per ERC-6538:
- `registerKeys(schemeId, stealthMetaAddress)`
- `registerKeysOnBehalf(registrant, schemeId, stealthMetaAddress, signature)` — EIP-712
- `stealthMetaAddressOf(registrant, schemeId)`
- Nonce management for replay protection

**WraithSender.sol** — Atomic send + announce. Uses ReentrancyGuard:
- `sendETH(schemeId, stealthAddress, ephemeralPubKey, metadata)` — payable
- `sendERC20(schemeId, stealthAddress, ephemeralPubKey, metadata, token, amount, gasTip)` — with optional ETH tip
- `batchSendETH(...)` and `batchSendERC20(...)` for multiple recipients
- Constructor takes announcer address

**WraithNames.sol** — Privacy-preserving name registry:
- `register(name, metaAddress, signature)` — spending key proves ownership
- `registerOnBehalf(name, metaAddress, signature)` — for sponsored registration
- `update(name, newMetaAddress, signature)`
- `release(name, signature)`
- `resolve(name) → bytes` and `nameOf(metaAddress) → string` (reverse lookup)
- Name validation: 3-32 chars, lowercase alphanumeric
- On-chain secp256k1 point decompression for signature recovery
- Signature: `ecrecover(keccak256("\x19Ethereum Signed Message:\n32" || keccak256(name || metaAddress)))`

**WraithWithdrawer.sol** — EIP-7702 delegation target:
- `withdrawETH(to, sponsorFee)`
- `withdrawERC20(token, to, sponsorFee)`
- `withdrawETHDirect(to)` and `withdrawERC20Direct(token, to)` — self-funded variants

Also create `interfaces/IERC5564Announcer.sol`, `interfaces/IERC6538Registry.sol`, and `test/ERC20Mock.sol`.

### Step 3 — EVM Tests

Port from `reference/horizen/contracts/test/`. Use Hardhat + chai:

| Test File | Coverage |
|---|---|
| `ERC5564Announcer.test.ts` | Event emission, multiple callers, metadata preservation |
| `ERC6538Registry.test.ts` | Register/lookup, EIP-712 delegation, replay prevention, nonce management, DOMAIN_SEPARATOR |
| `WraithNames.test.ts` | Register/resolve, reverse lookup, duplicate rejection, name length (3-32), invalid chars, signature verification, update by owner, update by non-owner (reject), release + re-register |
| `WraithSender.test.ts` | sendETH, sendERC20 with/without gas tip, batch ops, value mismatch, length mismatch |
| `WraithWithdrawer.test.ts` | Revert on empty balance, revert on fee >= balance |

Verify: `npx hardhat test` passes all tests.

### Step 4 — EVM Deployment Script

`scripts/deploy.ts`:
1. Deploy ERC5564Announcer
2. Deploy ERC6538Registry
3. Deploy WraithSender(announcer.address)
4. Deploy WraithNames
5. Deploy WraithWithdrawer
6. Log all addresses

### Step 5 — Stellar Scaffold

```
stellar/
  Cargo.toml                    # workspace
  stealth-announcer/
    Cargo.toml
    src/lib.rs
  stealth-registry/
    Cargo.toml
    src/lib.rs
  stealth-sender/
    Cargo.toml
    src/lib.rs
  wraith-names/
    Cargo.toml
    src/lib.rs
```

Deps: `soroban-sdk`

### Step 6 — Stellar Contracts

Port from `reference/stellar/contracts/`:

**stealth-announcer** — Emits announcement events. No storage:
```rust
pub fn announce(env, caller, scheme_id, stealth_address, ephemeral_pub_key, metadata);
```

**stealth-registry** — Maps addresses to 64-byte meta-addresses:
```rust
pub fn register_keys(env, registrant, scheme_id, stealth_meta_address);
pub fn stealth_meta_address_of(env, registrant, scheme_id) -> Bytes;
```
Enforces 64-byte length, requires auth.

**stealth-sender** — Atomic send + announce:
```rust
pub fn init(env, admin, announcer);
pub fn send(env, caller, token, stealth_address, amount, scheme_id, ephemeral_pub_key, metadata);
pub fn batch_send(env, caller, token, stealth_addresses, amounts, scheme_id, ephemeral_pub_keys, metadatas);
```

**wraith-names** — Name registry:
```rust
pub fn register(env, caller, name, meta_address);
pub fn update(env, caller, name, new_meta_address);
pub fn release(env, caller, name);
pub fn resolve(env, name) -> Bytes;
pub fn name_of(env, meta_address) -> String;
```
Names hashed via SHA-256 for storage. Validation: 3-32 chars, lowercase alphanumeric.

### Step 7 — Stellar Tests

Each contract has a `#[cfg(test)]` module in `lib.rs`:

| Contract | Tests |
|---|---|
| stealth-announcer | Event emission, different scheme IDs |
| stealth-registry | Register/lookup, wrong-length rejection, not-registered, update existing |
| wraith-names | Register/resolve, name-taken, reverse lookup, release/re-register, invalid name validation |

Verify: `cargo test` passes in each contract directory.

## Final Structure

```
contracts/
  evm/
    package.json
    hardhat.config.ts
    tsconfig.json
    contracts/
      ERC5564Announcer.sol
      ERC6538Registry.sol
      WraithSender.sol
      WraithNames.sol
      WraithWithdrawer.sol
      interfaces/
        IERC5564Announcer.sol
        IERC6538Registry.sol
      test/
        ERC20Mock.sol
    test/
      ERC5564Announcer.test.ts
      ERC6538Registry.test.ts
      WraithNames.test.ts
      WraithSender.test.ts
      WraithWithdrawer.test.ts
    scripts/
      deploy.ts
  stellar/
    Cargo.toml
    stealth-announcer/
      Cargo.toml
      src/lib.rs
    stealth-registry/
      Cargo.toml
      src/lib.rs
    stealth-sender/
      Cargo.toml
      src/lib.rs
    wraith-names/
      Cargo.toml
      src/lib.rs
  reference/                    # DO NOT MODIFY
    horizen/                    # existing EVM contracts
    stellar/                    # existing Stellar contracts
    docs/                       # implementation specs
```

## Code Quality Tooling

### Prettier (EVM only — Solidity + TypeScript)

Add `.prettierrc` in repo root:
```json
{
  "semi": true,
  "singleQuote": true,
  "trailingComma": "all",
  "printWidth": 100,
  "tabWidth": 2,
  "overrides": [
    {
      "files": "*.sol",
      "options": { "printWidth": 120 }
    }
  ]
}
```

Add `.prettierignore`:
```
node_modules
artifacts
cache
typechain-types
reference
stellar
```

Add format scripts to `evm/package.json`:
```json
{
  "format": "prettier --write .",
  "format:check": "prettier --check ."
}
```

### Husky + Commitlint (in evm/ directory)

Install: `husky`, `@commitlint/cli`, `@commitlint/config-conventional`, `prettier`, `prettier-plugin-solidity`

Add `commitlint.config.js` in repo root:
```js
module.exports = { extends: ['@commitlint/config-conventional'] };
```

Husky hooks:
- `.husky/pre-commit`: `cd evm && npx prettier --check . && npx hardhat compile && npx hardhat test`
- `.husky/commit-msg`: `npx --no -- commitlint --edit $1`

### CI

Add `.github/workflows/ci.yml`:

```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
jobs:
  evm:
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: evm
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22
      - run: npm ci
      - run: npx prettier --check .
      - run: npx hardhat compile
      - run: npx hardhat test
  stellar:
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: stellar
    steps:
      - uses: actions/checkout@v4
      - uses: stellar/setup-soroban@v1
      - run: cargo fmt --check --manifest-path Cargo.toml
      - run: cargo test --workspace
```

## README

Create a README.md covering: what these contracts do (stealth address infrastructure for Wraith Protocol), the EVM contracts (Announcer, Registry, Sender, Names, Withdrawer) with brief descriptions, the Stellar contracts (announcer, registry, sender, names), how to compile and test each (npx hardhat test for EVM, cargo test for Stellar), deployment instructions, and a deployed addresses table (leave empty for now). Keep it concise and technical.

## Rules

- NEVER add Co-Authored-By lines to commits
- NEVER commit, modify, or delete anything in the reference/ folder — it is gitignored and read-only
- NEVER add numbered step comments in code
- NEVER strip existing NatSpec/docs from reference code when porting
- All commit messages MUST follow conventional commits format (feat:, fix:, chore:, docs:, test:, refactor:)
- Commit after each completed step
- Push to origin after each completed step
- EVM and Stellar can be done in parallel (separate agents)
- EVM tests use Hardhat + chai
- Stellar tests use native `#[cfg(test)]` with soroban-sdk test utilities
- All contracts must be fully tested before considering the step done
