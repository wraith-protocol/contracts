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

### Gauge/Volume Metrics (absolute values)
- `send_volume` - Total volume sent (in token base units)
- `batch_size` - Size of a batch operation
- `total_transfers` - Total transfers in a batch

### Timing Metrics (when applicable)
- `execution_time_ms` - Execution time in milliseconds

## Standard Dimensions

Common dimensions that can be attached to metrics:

- `scheme_id` - Stealth address scheme identifier
- `token_address` - Token contract address
- `asset_code` - Asset code (if applicable)
- `error_code` - Error code (for error metrics)
- `contract_version` - Contract version

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
