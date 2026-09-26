# Keeper Cost Model

## Overview

This document describes the cost of running the Wraith Names TTL Keeper service, calculated for typical usage patterns.

## Per-Operation Costs

### Base Transaction Fee

Stellar's base transaction fee:
- **Rate**: 100 stroops (10^-7 XLM)
- **Per transaction**: 0.00001 XLM

### Soroban Resource Fees

Extending a name's TTL uses Soroban compute resources:

| Resource | Cost | Notes |
|----------|------|-------|
| Base cost | ~100 stroops | Per `extend_name_ttl()` call |
| Storage I/O | ~50 stroops | Read name entry + reverse lookup |
| Crypto operations | ~50 stroops | Hash lookups |
| **Total per name** | ~200 stroops | **0.00002 XLM** |

### Batch Efficiency

The Keeper batches multiple operations per transaction:

| Batch Size | Transaction Fee | Resource Fee | Cost Per Name |
|------------|-----------------|--------------|---------------|
| 1 name | 0.00001 XLM | 0.00002 XLM | **0.00003 XLM** |
| 10 names | 0.00001 XLM | 0.00020 XLM | **0.000021 XLM** |
| 100 names | 0.00001 XLM | 0.00200 XLM | **0.0000201 XLM** |

**Recommendation**: Batch 10-100 names per transaction for cost efficiency.

## Cost Per Name Per Year

### Scenario: 1000 Names

Assumptions:
- Names: 1000
- Batch size: 10 names per transaction
- Extensions per year: 10 (every ~36 days)

Calculation:

```
Batch size: 10
Batches per cycle: 1000 / 10 = 100 batches
Cost per batch: 0.00001 XLM (tx fee) + (10 × 0.00002 XLM) (resource) = 0.00021 XLM
Cost per cycle: 100 batches × 0.00021 XLM = 0.021 XLM
Cost per year: 0.021 XLM × 10 cycles = 0.21 XLM per 1000 names

Cost per name per year: 0.21 XLM / 1000 = 0.00021 XLM
```

### Scenario: 10,000 Names

```
Batch size: 10
Batches per cycle: 10,000 / 10 = 1000 batches
Cost per batch: 0.00021 XLM
Cost per cycle: 1000 × 0.00021 = 0.21 XLM
Cost per year: 0.21 × 10 = 2.1 XLM per 10,000 names

Cost per name per year: 2.1 XLM / 10,000 = 0.00021 XLM
```

### Scenario: 100,000 Names

```
Cost per name per year: 0.00021 XLM (scales linearly)
```

## Operating Cost Summary

| Names | Per Year | Per Month | Per Day | Per Hour |
|-------|----------|-----------|---------|----------|
| 1,000 | 0.21 XLM | 0.0175 XLM | 0.00058 XLM | 0.000024 XLM |
| 10,000 | 2.1 XLM | 0.175 XLM | 0.0058 XLM | 0.00024 XLM |
| 100,000 | 21 XLM | 1.75 XLM | 0.058 XLM | 0.0024 XLM |
| 1,000,000 | 210 XLM | 17.5 XLM | 0.58 XLM | 0.024 XLM |

## Extension Frequency Analysis

The extension frequency depends on:
- TTL threshold (default 1000 ledgers ≈ 83 minutes)
- Extension target (default 500,000 ledgers ≈ 41 days)
- Average name usage frequency

### Calculation

Using Soroban ledger parameters:
- Base fee: 100 stroops (0.00001 XLM)
- Ledger close rate: ~5 seconds
- Ledger time: 5 seconds

**TTL duration** = 500,000 ledgers × 5 seconds = 2,500,000 seconds = 28.9 days ≈ 29 days

**Threshold** = 1,000 ledgers × 5 seconds = 5,000 seconds ≈ 83 minutes

**Trigger point** = 29 days - 83 minutes ≈ 28 days

**Extension frequency** = Every ~29 days

**Annual frequency** = 365 days / 29 days ≈ 12.6 ≈ **~13 times per year**

## Cost Optimization Strategies

### 1. Larger Batches

Batch more names per transaction to amortize transaction fee:

```
Batch 100 names: 0.0000201 XLM per name
vs.
Batch 10 names: 0.000021 XLM per name
Savings: ~5%
```

### 2. Longer TTL Extension

Extend to a longer TTL to reduce frequency:

```
Current (500k ledgers): 13.5 times/year = 0.00283 XLM/year per name
Extended (1M ledgers): 6-7 times/year = 0.00142 XLM/year per name
Savings: ~50%
```

### 3. Selective Extension

Only extend names below a higher threshold to reduce frequency:

```
Current threshold (1000 ledgers): Full cost
Threshold (100,000 ledgers): 2-3 extensions/year = 60% savings
```

## Comparison with Alternatives

### No Keeper (Manual Recovery)

- Cost of restoring archived names: ~0.001 XLM per restoration
- Downtime: ~1-2 hours per archived name
- User experience: Poor (names unavailable during archival)

**vs. Keeper service: 0.00021 XLM/year** → ~5x cheaper over time

### Keeping Data On-Chain (Other Chains)

- Ethereum: ~$50/year for storage (state rent model)
- Bitcoin: Not applicable (no contract storage)

**vs. Keeper on Stellar: 0.00021 XLM/year ≈ $0.000021/year** (at $100/XLM) → 2M times cheaper

## Break-Even Analysis

Running your own Keeper requires:
- Server cost: ~$10/month
- Operational overhead: ~5-10 hours/year setup + monitoring

Break-even at:
```
Annual operating cost / cost per year per name = names
$120 / 0.00021 XLM = ~570,000 names (at $100/XLM)
```

For fewer than 570,000 names, use a shared Keeper service instead.

## Recommendations

### Individual Keepers

**For**: 100,000+ registered names

**Configuration**:
- Batch size: 100 names
- Threshold: 1,000 ledgers
- Extend to: 1,000,000 ledgers (longer TTL)
- Frequency: Every 48 hours (cron job)

**Cost**: ~0.00010 XLM per name per year

### Shared Keeper Service

**For**: < 100,000 names

**Model**: Centralized operator extends all names for a tiny fee

**Proposed fee**: 0.000001 XLM per name per year (covers infrastructure)

### Community Keeper

**For**: Decentralized, community-run service

**Model**: Multiple operators run keepers, compete on cost/uptime

**Benefits**:
- Decentralized
- Permissionless participation
- Market-driven pricing

## Monitoring & Metrics

Key metrics to track:

| Metric | Target | Alert |
|--------|--------|-------|
| Keeper uptime | 99.9% | < 99% |
| Avg extension time | < 30s | > 60s |
| Failure rate | < 0.1% | > 1% |
| Cost per extension | < 0.00025 XLM | > 0.0005 XLM |

## Conclusion

The Wraith Names Keeper service costs approximately:

- **$0.000021 per name per year** (at $100/XLM)
- **$210 per million names per year**
- **Negligible compared to name utility**

This makes proactive TTL extension highly cost-effective compared to manual recovery or archive management on other chains.

## References

- [Stellar Fees and Fee-Bump Transactions](https://developers.stellar.org/docs/learn/fees)
- [Soroban Resource Costs](https://developers.stellar.org/docs/learn/soroban-resource-model)
- [Ledger Close Times](https://developers.stellar.org/docs/learn/glossary)
