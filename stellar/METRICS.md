# Wraith Protocol Stellar Contract Metrics Standard

## Overview

This document defines the standard metric event format for Wraith Protocol Stellar contracts. All Wraith contracts emit standardized metric events to enable off-chain observability, dashboards, and monitoring.

## WraithMetricEvent Schema

Metric events are emitted as Soroban contract events with the following structure:

### Event Topic
```
(metric_name, contract_identifier)
```

### Event Data
```
(value, dimensions)
```

### Schema Definition

```rust
#[contracttype]
#[derive(Clone)]
pub struct WraithMetricEvent {
    /// Contract identifier (e.g., "stealth-registry", "stealth-sender")
    pub contract: Symbol,
    /// Metric name (e.g., "register_count", "send_volume")
    pub metric_name: Symbol,
    /// Numeric value of the metric
    pub value: i128,
    /// Optional dimensions for filtering/grouping (e.g., token_address, scheme_id)
    pub dimensions: Vec<(Symbol, Symbol)>,
}
```

## Standard Metric Names

### Counter Metrics (incrementing values)
- `register_count` - Number of registrations
- `remove_count` - Number of removals
- `send_count` - Number of sends
- `batch_send_count` - Number of batch sends
- `announce_count` - Number of announcements
- `error_count` - Number of errors
- `renew_count` - Number of name renewals (TTL extensions)
- `release_count` - Number of name releases
- `resolve_hit_count` - Number of name resolutions that found an entry
- `resolve_miss_count` - Number of name resolutions that found nothing
- `create_count` - Number of split definitions created
- `fund_count` - Number of split fundings
- `deposit_count` - Number of vault deposits
- `claim_count` - Number of vault claims
- `refund_count` - Number of vault refunds
- `proposal_count` - Number of governance proposals created
- `vote_count` - Number of governance votes cast
- `execution_count` - Number of governance proposals executed

### Gauge/Volume Metrics (absolute values)
- `send_volume` - Total volume sent (in token base units)
- `batch_size` - Size of a batch operation
- `total_transfers` - Total transfers in a batch
- `fund_volume` - Amount distributed by a split funding (in token base units)
- `deposit_volume` - Amount locked by a vault deposit (in token base units)
- `beneficiaries_per_split` - Number of beneficiaries in a split definition

### Timing Metrics (when applicable)
- `execution_time_ms` - Execution time in milliseconds

## Standard Dimensions

Common dimensions that can be attached to metrics:

- `scheme_id` - Stealth address scheme identifier
- `token_address` - Token contract address
- `asset_address` - Asset (token) contract address for splitter/vault metrics
- `asset_code` - Asset code (if applicable)
- `error_code` - Error code (for error metrics)
- `contract_version` - Contract version
- `proposal_id` - Governance proposal identifier
- `support` - Vote direction (`true` = for, `false` = against)

## Event Format

All metric events use the following event topic pattern:

```rust
env.events().publish(
    (symbol_short!("metric"), contract_name, metric_name),
    (value, dimensions),
);
```

## Contract-Specific Metrics

### Stealth Registry

| Metric Name | Trigger | Value | Dimensions |
|-------------|---------|-------|------------|
| `register_count` | After successful registration | 1 (increment) | `scheme_id` |
| `remove_count` | After successful removal | 1 (increment) | `scheme_id` |
| `lookup_count` | After successful lookup | 1 (increment) | `scheme_id` |

### Stealth Sender

| Metric Name | Trigger | Value | Dimensions |
|-------------|---------|-------|------------|
| `send_count` | After successful send | 1 (increment) | `scheme_id`, `token_address` |
| `send_volume` | After successful send | Amount sent | `scheme_id`, `token_address` |
| `batch_send_count` | After successful batch send | 1 (increment) | `scheme_id`, `token_address` |
| `batch_send_volume` | After successful batch send | Total amount | `scheme_id`, `token_address` |
| `batch_size` | After successful batch send | Number of transfers | `scheme_id`, `token_address` |

### Stealth Batch Sender

| Metric Name | Trigger | Value | Dimensions |
|-------------|---------|-------|------------|
| `batch_send_count` | After successful batch send | 1 (increment) | `asset_address` |
| `batch_send_volume` | After successful batch send | Total amount | `asset_address` |
| `batch_size` | After successful batch send | Number of transfers | `asset_address` |

### Wraith Names

| Metric Name | Trigger | Value | Dimensions |
|-------------|---------|-------|------------|
| `register_count` | After a name is written to storage (covers `register`, `register_on_behalf`, `bulk_register`, and auction `claim_name`) | 1 (increment) | — |
| `renew_count` | After `extend_name_ttl`, and once per `bulk_renew` batch | 1 (increment), or the batch size for `bulk_renew` | — |
| `release_count` | After a name is removed (covers `release` and `release_on_behalf`) | 1 (increment) | — |
| `resolve_hit_count` | After `resolve` finds the name entry | 1 (increment) | — |
| `resolve_miss_count` | When `resolve` does not find the name entry | 1 (increment) | — |

`bulk_register` emits one `register_count` per name (it calls the same internal
path as `register`), whereas `bulk_renew` emits a single `renew_count` carrying
the batch size as its value. Both aggregate to the same total.

`resolve` returns `NameNotFound` on a miss. The metric event is published before
the error is returned, so it is captured whenever the enclosing transaction is
applied — for example when `resolve` is invoked as a sub-call by a contract that
handles the error, or during simulation. A top-level `resolve` that fails takes
the whole transaction down with it and, like any other event in a failed
transaction, is not written to the ledger.

### Stealth Splitter

| Metric Name | Trigger | Value | Dimensions |
|-------------|---------|-------|------------|
| `create_count` | After a split definition is stored | 1 (increment) | `asset_address` |
| `beneficiaries_per_split` | After a split definition is stored | Number of beneficiaries | `asset_address` |
| `fund_count` | After a successful `fund_split` | 1 (increment) | `asset_address` |
| `fund_volume` | After a successful `fund_split` | Amount distributed | `asset_address` |

### Stealth Vault

| Metric Name | Trigger | Value | Dimensions |
|-------------|---------|-------|------------|
| `deposit_count` | After a successful `deposit` | 1 (increment) | `asset_address` |
| `deposit_volume` | After a successful `deposit` | Amount locked | `asset_address` |
| `claim_count` | After a successful `claim` | 1 (increment) | `asset_address` |
| `refund_count` | After a successful `refund` | 1 (increment) | `asset_address` |

### Governance

| Metric Name | Trigger | Value | Dimensions |
|-------------|---------|-------|------------|
| `proposal_count` | After a proposal is stored by `propose` | 1 (increment) | `proposal_id` |
| `vote_count` | After a vote is recorded by `vote` | 1 (increment) | `proposal_id`, `support` |
| `execution_count` | After a proposal is executed by `execute` | 1 (increment) | `proposal_id` |

## Symbol Encoding

Soroban `Symbol`s used in event topics are limited to 9 characters, so the
contract identifiers and metric names above are abbreviated on-chain. Indexers
must map the wire symbol back to the canonical name from this document.

| Contract | Wire symbol |
|----------|-------------|
| `stealth-registry` | `st_reg` |
| `stealth-sender` | `st_send` |
| `stealth-batch-sender` | `st_bat_sd` |
| `stealth-announcer` | `st_ann` |
| `wraith-names` | `wr_names` |
| `stealth-splitter` | `st_split` |
| `stealth-vault` | `st_vault` |
| `governance` | `gov` |

| Metric name | Wire symbol |
|-------------|-------------|
| `register_count` | `reg_cnt` |
| `remove_count` | `rem_cnt` |
| `lookup_count` | `lkp_cnt` |
| `send_count` | `send_cnt` |
| `send_volume` | `send_vol` |
| `batch_send_count` | `bat_send` |
| `batch_send_volume` | `bat_vol` |
| `batch_size` | `bat_size` |
| `error_count` | `err_cnt` |
| `renew_count` | `renew_cnt` |
| `release_count` | `rel_cnt` |
| `resolve_hit_count` | `res_hit` |
| `resolve_miss_count` | `res_miss` |
| `create_count` | `crt_cnt` |
| `fund_count` | `fund_cnt` |
| `fund_volume` | `fund_vol` |
| `beneficiaries_per_split` | `benef_cnt` |
| `deposit_count` | `dep_cnt` |
| `deposit_volume` | `dep_vol` |
| `claim_count` | `clm_cnt` |
| `refund_count` | `rfnd_cnt` |
| `proposal_count` | `prop_cnt` |
| `vote_count` | `vote_cnt` |
| `execution_count` | `exec_cnt` |

| Dimension name | Wire symbol |
|----------------|-------------|
| `scheme_id` | `scheme_id` |
| `token_address` | `tok_addr` |
| `asset_address` | `ast_addr` |
| `error_code` | `err_code` |
| `proposal_id` | `prop_id` |
| `support` | `support` |

The canonical constants live in `stellar/wraith-metrics/src/lib.rs`
(`contract_ids`, `metric_names`, `dimension_names`); the indexer's lookup tables
in `stellar/scripts/metrics-indexer/index.js` mirror them.

## Indexer Implementation

A reference indexer implementation is provided in `stellar/scripts/metrics-indexer/` that:

1. Connects to a Stellar RPC node (futurenet/testnet/mainnet)
2. Subscribes to contract events
3. Parses WraithMetricEvent format
4. Aggregates metrics in memory
5. Exposes metrics in Prometheus format

## Prometheus Exporter Format

Metrics are exposed in Prometheus text format:

```
# HELP wraith_register_count Total number of registrations
# TYPE wraith_register_count counter
wraith_register_count{contract="stealth-registry",scheme_id="1"} 42

# HELP wraith_send_volume Total volume sent
# TYPE wraith_send_volume gauge
wraith_send_volume{contract="stealth-sender",token_address="CDL..."} 1000000
```

## Usage Example

### Emitting a Metric Event

```rust
// In stealth-registry/src/lib.rs
env.events().publish(
    (symbol_short!("metric"), symbol_short!("stealth-registry"), symbol_short!("register_count")),
    (1i128, soroban_sdk::vec![&env, (symbol_short!("scheme_id"), scheme_id.into_val(env))]),
);
```

### Consuming Metric Events

The reference indexer (see `stellar/scripts/metrics-indexer/`) demonstrates how to:

1. Parse event topics to extract contract and metric name
2. Parse event data to extract value and dimensions
3. Aggregate metrics over time
4. Export to Prometheus format

## Dashboard Integration

A demo Grafana dashboard configuration is provided to visualize:

- Registration trends over time
- Send volume by token
- Batch operation statistics
- Error rates

See `stellar/scripts/metrics-indexer/grafana-dashboard.json` for the complete dashboard definition.

## Versioning

This metrics standard is versioned as `v1`. Future versions will maintain backward compatibility or introduce new metric names with version suffixes (e.g., `register_count_v2`).

## Testing

To test metric event emission:

```bash
# Run contract tests
cd stellar/stealth-registry
cargo test

# Run indexer against futurenet
cd stellar/scripts/metrics-indexer
npm install
npm start -- --network futurenet
```

## References

- Soroban SDK Events: https://developers.stellar.org/docs/build/smart-contracts/reference/events
- Prometheus Format: https://prometheus.io/docs/instrumenting/exposition_formats/
