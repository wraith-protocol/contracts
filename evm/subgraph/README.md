# Subgraph Deployments

Each subdirectory contains the Goldsky instant subgraph config for one EVM network.
The `graph-cli` manifest is generated per network from `subgraph.template.yaml`; see
[graph-cli manifest](#graph-cli-manifest).

## Structure

```
subgraph/
  subgraph.template.yaml ← graph-cli manifest without network, addresses or start blocks
  subgraph.yaml          ← generated from the template for one network (git-ignored)
  horizen-testnet/       ← Horizen Testnet
    instant-config.json
    abis/
  ethereum/              ← Ethereum Mainnet (when deployed)
    instant-config.json
    abis/
  base/                  ← Base (when deployed)
    ...
```

## Redeployment Process

When deploying contracts to a new network or updating existing deployments, follow these steps:

### 1. Deploy Contracts

Deploy the contracts using Hardhat. The deployment script automatically updates the subgraph configuration with the deployed addresses and start block.

```bash
cd evm

# Set environment variables for the target network
export HORIZEN_TESTNET_RPC_URL="https://horizen-testnet.rpc.caldera.xyz/http"
export PRIVATE_KEY="your_private_key"

# Deploy to Horizen Testnet
npm run deploy:horizen-testnet
```

The deployment script will:

- Deploy all 5 contracts (ERC5564Announcer, ERC6538Registry, WraithSender, WraithNames, WraithWithdrawer)
- Capture the deployment block number
- Automatically update `subgraph/horizen-testnet/instant-config.json` with the real addresses and start block

### 2. Validate Configuration

Before deploying the subgraph, validate that all placeholder addresses have been replaced:

```bash
cd evm
npm run validate-subgraph
```

This script checks that:

- No contract address is set to `0x000000000000000000000000000000000000dead`
- No start block is set to `0`

If validation fails, the script will exit with an error and list the issues.

### 3. Deploy Subgraph

Once validation passes, deploy the subgraph to Goldsky:

```bash
cd evm/subgraph/<network>
goldsky subgraph deploy wraith-protocol-<network>/1.0.0 --from-abi instant-config.json
```

To deploy with `graph deploy` instead, generate the manifest first (see
[graph-cli manifest](#graph-cli-manifest)).

### 4. Adding a New Network

To add support for a new EVM network:

1. Create a new directory: `evm/subgraph/<network>/`
2. Copy the ABIs from `evm/artifacts/contracts/` to `evm/subgraph/<network>/abis/`
3. Create `instant-config.json` with placeholder addresses:
   ```json
   {
     "version": "1",
     "name": "wraith-protocol",
     "abis": {
       "ERC5564Announcer": { "path": "./abis/ERC5564Announcer.json" },
       "ERC6538Registry": { "path": "./abis/ERC6538Registry.json" },
       "WraithNames": { "path": "./abis/WraithNames.json" },
       "WraithSender": { "path": "./abis/WraithSender.json" },
       "WraithWithdrawer": { "path": "./abis/WraithWithdrawer.json" }
     },
     "instances": [
       {
         "abi": "ERC5564Announcer",
         "address": "0x000000000000000000000000000000000000dead",
         "chain": "<network>",
         "startBlock": 0
       }
       // ... repeat for all contracts
     ]
   }
   ```
4. Add the network configuration to `evm/hardhat.config.ts`
5. Add a deploy script to `evm/package.json`: `"deploy:<network>": "hardhat run scripts/deploy.ts --network <network>"`
6. Deploy contracts using the new script
7. Validate and deploy the subgraph

## graph-cli manifest

`subgraph.template.yaml` is the checked-in `graph-cli` manifest. It has no network,
contract addresses or start blocks. `npm run manifest` fills them in from
`deployments/evm/<network>.json` and writes `subgraph.yaml`, which is git-ignored:

```bash
(cd scripts/deployment-manifest && npm ci)   # once, from the repository root; the generator lives here
cd evm/subgraph
npm ci
npm run manifest -- --network <network>
npm run codegen
npm run deploy -- <subgraph-name> --node <graph-node-url>   # any graph deploy arguments
```

`npm run manifest` fails and writes nothing if a contract the template indexes is
not in the record, has no start block (or start block 0), or has the placeholder
address `0x…dead`. It lists every such contract. `npm run deploy` and
`npm run deploy-local` first run `npm run manifest -- --check`, which fails unless
`subgraph.yaml` exists and is exactly what the generator writes now for the network
it names, so these scripts do not deploy a stale or hand-edited manifest. A bare
`graph deploy` skips that check; use the npm scripts.

`codegen`, `build` and `test` read the template directly (`matchstick.yaml` points
`graph test` at it), so they work without a deployment.

## Deployed Subgraphs

| Network         | Subgraph URL                                                                                                              |
| --------------- | ------------------------------------------------------------------------------------------------------------------------- |
| Horizen Testnet | `https://api.goldsky.com/api/public/project_cmhp1xyw0qu8901xcdayke69d/subgraphs/wraith-protocol-horizen-testnet/1.0.0/gn` |

## Deployment Status

### Horizen Testnet

- **Deployed**: ERC5564Announcer, ERC6538Registry
- **Not yet deployed**: WraithNames, WraithSender, WraithWithdrawer

The subgraph currently only indexes the deployed contracts. To add support for WraithNames, WraithSender, and WraithWithdrawer, deploy these contracts to Horizen Testnet and update the subgraph configuration following the redeployment process above.

Until they are recorded in `deployments/evm/horizen-testnet.json`,
`npm run manifest -- --network horizen-testnet` fails and names the three of them.
