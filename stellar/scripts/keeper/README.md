# Wraith Names TTL Keeper Bot

The TTL Keeper Bot automates the extension of storage TTL for registered `.wraith` names on Stellar's Soroban network. This prevents names from being archived due to TTL expiration, which would require paying restoration fees to access them again.

## Why TTL Extension is Important

Soroban storage entries have a **time-to-live (TTL)** measured in ledgers. When an entry's TTL expires:
1. The entry is moved to the **archived state**
2. Re-accessing archived data requires paying a **restoration fee**
3. Users may not notice their name is archived until they (or someone trying to resolve their name) hits the archival wall

By proactively extending TTLs, we provide a seamless UX where names remain accessible without surprise fees.

## How It Works

The keeper bot:

1. **Discovers registered names** — Iterates through the contract's ledger state or uses an off-chain index
2. **Checks TTL remaining** — For each name, determines how many ledgers until archival
3. **Extends if necessary** — If TTL remaining < threshold, calls `extend_name_ttl()` to push it further into the future
4. **Logs results** — Tracks which names were extended, successes, and failures

### Threshold-Based Extension

The bot uses a **TTL threshold** to decide when to extend:
- Default: **100,000 ledgers**
- If a name's current TTL is less than 100,000 ledgers away, the bot extends it
- Extends to **current_ledger + extend_to_future_ledgers** (default: 500,000 ledgers)

This means:
- A name registered today will not need extension for ~500,000 ledgers (~2.3 years on Stellar mainnet)
- The bot only acts once TTL drops below 100,000 ledgers (~463 days on mainnet)

### Idempotency

The `extend_name_ttl()` contract function is **idempotent**: calling it multiple times in the same ledger or with the same target ledger is safe and cheap.

## Setup & Running

### Prerequisites

- Node.js 18+
- Access to Stellar Soroban RPC endpoint
- Keeper account with XLM balance to pay for extension transactions
- `.wraith` contract ID

### Configuration

Set environment variables:

```bash
# Stellar network configuration
export STELLAR_NETWORK_PASSPHRASE="Test SDF Network ; June 2015"  # or mainnet
export SOROBAN_RPC_URL="https://soroban-testnet.stellar.org"

# Keeper account (must have XLM balance)
export KEEPER_SECRET_KEY="S..."

# Contract and behavior
export WRAITH_NAMES_CONTRACT="C..."
export TTL_THRESHOLD_LEDGERS="100000"        # Extend when TTL drops below this
export EXTEND_TO_FUTURE_LEDGERS="500000"     # Extend to this many ledgers in future
export DRY_RUN="false"                        # Set to "true" to preview without executing
```

### Run the Keeper

```bash
# Install dependencies (one-time)
npm ci

# Run keeper (once)
npx ts-node keeper.ts --network testnet --contract C... --threshold 100000 --extend-to 500000

# Run with dry-run to preview
npx ts-node keeper.ts --network testnet --contract C... --dry-run

# Dry-run to check what would happen
KEEPER_SECRET_KEY="S..." DRY_RUN=true npx ts-node keeper.ts \
  --network testnet \
  --contract C... \
  --threshold 100000
```

### Run as a Periodic Cron Job

To extend TTLs weekly:

```bash
# In your crontab
0 0 * * 0 cd /path/to/contracts/stellar/scripts/keeper && \
  KEEPER_SECRET_KEY="$SECRET" \
  WRAITH_NAMES_CONTRACT="$CONTRACT_ID" \
  npx ts-node keeper.ts --network testnet >> keeper.log 2>&1
```

## Cost Model

### Single Extension Cost

Each `extend_name_ttl()` invocation has:
- **Base cost**: ~1,000 stroops (0.0001 XLM) for the contract invocation
- **Varies by**: RPC fees, network congestion, contract complexity

On **Stellar Testnet**: Negligible (test stroops have no value)  
On **Stellar Mainnet**: ~0.0001–0.001 XLM per extension

### Annual Cost for 1,000 Names

Assuming:
- **1,000 registered names**
- **TTL threshold**: 100,000 ledgers (~463 days on mainnet)
- **Extension cost**: 0.001 XLM per name
- **Extension frequency**: Once every 300 days (to stay ahead of 463-day expiration)
- **Annual extensions per name**: ~1.2 (365 days ÷ 300 days)

**Total annual cost**: ~1.2 XLM for 1,000 names

### Scalability

The cost per name is **independent of total names** because:
- Each `extend_name_ttl()` is a separate contract call
- No aggregation overhead
- No indexing or lookups across the entire namespace

**Linear scaling**: Doubling the number of names doubles the cost.

### Trade-off: Aggressive vs. Lazy Extension

| Strategy | Threshold | Frequency | Cost | UX |
|---|---|---|---|---|
| **Conservative** | 50,000 ledgers (~232 days) | ~2× yearly | Higher | Never archived |
| **Balanced** (default) | 100,000 ledgers (~463 days) | ~1× yearly | Moderate | Rarely archived |
| **Lazy** | 200,000 ledgers (~926 days) | ~0.5× yearly | Low | Occasional archival |

## Keeper Bot Implementation Status

### Current (Stub)

The `keeper.ts` script provides:
- ✅ CLI argument parsing (`--network`, `--contract`, `--threshold`, etc.)
- ✅ Environment variable loading
- ✅ Dry-run mode for previewing operations
- ✅ Placeholder for contract TTL extension
- ✅ Basic logging and summary reporting

### TODO: Production Implementation

1. **Name Discovery** — Implement `getAllRegisteredNames()` to fetch all registered names from the contract
   - Option A: Contract exposes `get_all_names_paginated()` method
   - Option B: Use Soroban RPC to iterate `wraith-names:Name(...)` ledger entries
   - Option C: Maintain off-chain index (e.g., The Graph / subgraph indexer)

2. **TTL Checking** — Fetch current TTL for each name (requires contract instrumentation or RPC query)

3. **Batch Operations** — Group extensions into multi-sig transactions to reduce cost

4. **Error Handling** — Retry logic, circuit breaker for failed extensions

5. **Monitoring** — Emit metrics (extended count, failures, average cost) to observability stack

## Design Decisions

### No Authorization Required for `extend_name_ttl()`

The contract function is permissionless—anyone can extend any name's TTL. This enables:
- **Decentralized care-taking** — Community members can run keepers
- **Resilience** — If one keeper bot goes offline, others can take over
- **No rent-seeking** — Extension cost is transparent and fair

**Trade-off**: Squatting becomes effectively permanent (an attacker who registers names will never lose them to archival). **This is intentional** — `.wraith` names are meant to be permanent identity, not a scarce resource to be recycled.

### Single-Call Extension

`extend_name_ttl(name, extend_to_ledger)` extends **both**:
- The name entry itself
- The reverse-lookup entry (metaaddress → name)

This keeps both structures in sync and simplifies keeper logic.

### Events for Transparency

Each `extend_name_ttl()` emits:
```
event: ("extend", name_hash)
data: (name, extend_to_ledger)
```

This allows off-chain systems to track when names are extended and by whom (all calls are visible on-ledger).

## Monitoring & Alerts

### Suggested Metrics to Track

1. **Names at risk** — Count with TTL < 200,000 ledgers
2. **Extension success rate** — (successful ÷ total) × 100
3. **Average cost per extension** — Total XLM spent ÷ count
4. **Lag behind threshold** — How much earlier than threshold do we extend?

### Example Alert Conditions

- **Critical**: If >50% of names have TTL < threshold
- **Warning**: If extension success rate < 95%
- **Info**: Daily summary of extensions performed

## Future Improvements

1. **On-Chain TTL Management** — Smart contract that auto-extends TTLs on every access (pays network gas but eliminates keeper bot dependency)

2. **Name Renewal Registry** — Owners can pre-pay for TTL renewals; keeper bot auto-extends on schedule

3. **Crowdfunded Extensions** — Community pool that shares keeper costs across all names

4. **MEV-Resistant Extension** — Mechanism to prevent keepers from front-running extensions for fee extraction

## References

- [Soroban Storage TTL Semantics](https://developers.stellar.org/docs/build/guides/contract-development/storage/state-archival)
- [ERC-6538 Stealth Address Registry](https://eips.ethereum.org/EIPS/eip-6538)
- [Wraith Protocol Specification](../docs/07-smart-contracts.md)
