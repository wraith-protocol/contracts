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

## Deploy

```bash
cd evm/subgraph/<network>
goldsky subgraph deploy wraith-protocol-<network>/1.0.0 --from-abi instant-config.json
```

## Deployed Subgraphs

| Network         | Subgraph URL                                                                                                              |
| --------------- | ------------------------------------------------------------------------------------------------------------------------- |
| Horizen Testnet | `https://api.goldsky.com/api/public/project_cmhp1xyw0qu8901xcdayke69d/subgraphs/wraith-protocol-horizen-testnet/1.0.0/gn` |
