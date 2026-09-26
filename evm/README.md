# Wraith EVM Contracts

Solidity contracts for the Wraith stealth address platform. Built with Hardhat; tested with Hardhat + Chai, Foundry invariants, Slither, and a Goldsky/The Graph subgraph.

## Contracts

| Contract             | Purpose                                                                                                                                                                                 |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **ERC5564Announcer** | Minimal singleton emitting `Announcement(schemeId, stealthAddress, caller, ephemeralPubKey, metadata)` events per ERC-5564. No storage, no access control.                              |
| **ERC6538Registry**  | Stealth meta-address registry per ERC-6538. Direct registration plus EIP-712 `registerKeysOnBehalf` with replay-protected nonces, and EIP-1271 smart-wallet signatures.                 |
| **WraithSender**     | Atomic asset transfer + announcement in one transaction. `sendETH` / `sendERC20` (optional ETH gas tip) and `batchSendETH` / `batchSendERC20`.                                          |
| **WraithNames**      | Privacy-preserving `.wraith` name → meta-address registry. Ownership proven by secp256k1 spending-key signature over `keccak256(name, metaAddress)`. No wallet address stored.          |
| **WraithWithdrawer** | EIP-7702 delegation target for gas-sponsored withdrawals. Sponsor fee variant (`withdrawETH` / `withdrawERC20`) and self-funded variants (`withdrawETHDirect` / `withdrawERC20Direct`). |

## Prerequisites

- Node.js 22+
- Foundry (forge) — <https://getfoundry.sh>
- Python 3 + `pip install slither-analyzer`
- `@graphprotocol/graph-cli` (installed via `npm ci` in `subgraph/`)

## Install

```bash
npm install
```

## Compile and test (Hardhat)

```bash
npx hardhat compile
npx hardhat test
```

## Invariant tests (Foundry)

Handler-based invariant suites under `foundry/test/invariant/`, covering
sender balance conservation, withdrawer atomicity, and name-registration
monotonicity. Each invariant runs 256 times.

`forge-std` is a pinned git submodule (v1.16.2), so initialize it before
building. The other two libraries under `foundry/lib/` are symlinks:
`openzeppelin` → `evm/node_modules/@openzeppelin` (so run `npm ci`
**first**, otherwise `forge` fails with a missing path) and
`wraith-contracts` → `evm/contracts`.

```bash
npm ci                       # needed for the openzeppelin symlink
cd foundry
git submodule update --init --recursive   # fetch pinned forge-std
forge build
forge test --match-path 'test/invariant/*'
```

## Gas snapshots

`foundry/.gas-snapshot` is committed. Regenerate after intentional gas changes:

```bash
cd foundry
forge snapshot --no-match-path 'test/invariant/*'
```

The CI diff gate fails any PR that increases gas by more than 5%:

```bash
forge snapshot --no-match-path 'test/invariant/*' --check --tolerance 5
```

## Static analysis (Slither)

`slither.config.json` pins a curated detector set. CI requires zero High/Medium findings:

```bash
slither . --config-file slither.config.json --fail-medium
```

## Subgraph

`subgraph/` indexes all five contracts: event handlers for Announcer, Registry,
and Names, plus call handlers for Sender and Withdrawer. The manifest targets
Horizen Testnet; fill in the three `0x…dead` placeholder addresses (Names,
Sender, Withdrawer) from `scripts/deploy.ts` output before deploying.

```bash
cd subgraph
npm ci
npm run codegen   # graph codegen
npm run build     # graph build
```

Deploy to a local graph-node:

```bash
npm run create-local
npm run deploy-local
```

## Deployment

```bash
npx hardhat run scripts/deploy.ts --network <network>
```

The script deploys Announcer, Registry, Sender (pointing at the Announcer),
Names, and Withdrawer, logging each address. Point the subgraph at the deployed
addresses and redeploy.

## CI

`.github/workflows/ci.yml` runs: prettier, `hardhat compile`, `hardhat test`,
Foundry invariants, the gas-snapshot diff gate, Slither (zero High/Medium), and
`graph codegen && graph build`.
