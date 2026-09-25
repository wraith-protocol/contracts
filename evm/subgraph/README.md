# Subgraph Deployments

Each subdirectory contains the Goldsky instant subgraph config for one EVM network.

## Structure

```
subgraph/
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
export HORIZEN_TESTNET_RPC_URL="https://testnet.horizen.io/api"
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

## Deployed Subgraphs

| Network         | Subgraph URL                                                                                                              |
| --------------- | ------------------------------------------------------------------------------------------------------------------------- |
| Horizen Testnet | `https://api.goldsky.com/api/public/project_cmhp1xyw0qu8901xcdayke69d/subgraphs/wraith-protocol-horizen-testnet/1.0.0/gn` |
