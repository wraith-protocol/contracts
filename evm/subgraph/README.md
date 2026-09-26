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

## Tests

Mapping logic is covered by [Matchstick](https://github.com/LimeChain/matchstick) fixtures in
`tests/`, one file per data source:

| File                 | Handlers covered                                                                                   |
| -------------------- | -------------------------------------------------------------------------------------------------- |
| `announcer.test.ts`  | `handleAnnouncement`                                                                               |
| `registry.test.ts`   | `handleStealthMetaAddressSet`, `handleNonceIncremented`                                            |
| `names.test.ts`      | `handleNameRegistered`, `handleNameReleased`                                                       |
| `sender.test.ts`     | `handleSendETH`, `handleSendERC20`, `handleBatchSendETH`, `handleBatchSendERC20`                   |
| `withdrawer.test.ts` | `handleWithdrawETH`, `handleWithdrawERC20`, `handleWithdrawETHDirect`, `handleWithdrawERC20Direct` |

Each handler asserts entity ids, relationships, amounts, block/tx provenance and timestamps, and
every handler has duplicate-delivery and replay (same transaction hash, different block) cases
proving the mappings stay idempotent.

```bash
npm ci
npm run codegen
npm test
```

The Matchstick rust binary version is pinned in the `test` script; `npm run test:coverage` produces a
coverage report. Tests run in CI alongside `graph build`.

## Deployed Subgraphs

| Network         | Subgraph URL                                                                                                              |
| --------------- | ------------------------------------------------------------------------------------------------------------------------- |
| Horizen Testnet | `https://api.goldsky.com/api/public/project_cmhp1xyw0qu8901xcdayke69d/subgraphs/wraith-protocol-horizen-testnet/1.0.0/gn` |
