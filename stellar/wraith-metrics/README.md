# Wraith Metrics Library (`wraith-metrics`)

**⚠️ THIS IS A LIBRARY, NOT A DEPLOYABLE CONTRACT** — This crate provides shared types and helper functions for metric event emission across Wraith Protocol contracts.

The `wraith-metrics` library defines the standard metric event schema and helper functions used by Wraith Protocol Stellar contracts to enable standardized off-chain observability and monitoring.

## Purpose

Shared metrics infrastructure for Wraith Protocol contracts. All Wraith contracts emit standardized metric events using this library to enable off-chain dashboards, monitoring, and analytics.

## Usage

This is a library crate, not a deployable contract. It is included as a dependency by other Wraith contracts that need to emit metric events.

### Adding to a Contract

Add to `Cargo.toml`:
```toml
[dependencies]
wraith-metrics = { path = "../wraith-metrics" }
```

### Emitting a Metric Event

```rust
use wraith_metrics::{contract_ids, dimension_names, emit_metric, metric_names};

// Emit a counter metric
emit_metric(
    &env,
    contract_ids::STEALTH_SENDER,
    metric_names::SEND_COUNT,
    1,
    soroban_sdk::vec![&env, (dimension_names::TOKEN_ADDRESS, token_address.into_val(&env))],
);
```

## Data Structures

### WraithMetricEvent

The standard metric event structure:

```rust
#[contracttype]
#[derive(Clone)]
pub struct WraithMetricEvent {
    pub contract: Symbol,           // Contract identifier (e.g., "stealth-registry")
    pub metric_name: Symbol,        // Metric name (e.g., "register_count")
    pub value: i128,                // Numeric value of the metric
    pub dimensions: Vec<(Symbol, Val)>, // Optional dimensions for filtering/grouping
}
```

## Helper Functions

### emit_metric

Emit a metric event using the standard schema.

**Parameters:**
- `env: &Env` — The Soroban environment
- `contract: Symbol` — Contract identifier
- `metric_name: Symbol` — Metric name
- `value: i128` — Metric value
- `dimensions: Vec<(Symbol, Val)>` — Optional dimensions

**Event emitted:**
- Topics: `("metric", contract, metric_name)`
- Data: `(value, dimensions)`

## Standard Metric Names

Defined in `metric_names` module:

| Constant | Symbol | Description |
|----------|--------|-------------|
| `REGISTER_COUNT` | `reg_cnt` | Number of registrations |
| `REMOVE_COUNT` | `rem_cnt` | Number of removals |
| `LOOKUP_COUNT` | `lkp_cnt` | Number of lookups |
| `SEND_COUNT` | `send_cnt` | Number of sends |
| `SEND_VOLUME` | `send_vol` | Total volume sent |
| `BATCH_SEND_COUNT` | `bat_send` | Number of batch sends |
| `BATCH_SEND_VOLUME` | `bat_vol` | Total batch send volume |
| `BATCH_SIZE` | `bat_size` | Size of a batch operation |
| `ERROR_COUNT` | `err_cnt` | Number of errors |

## Standard Contract Identifiers

Defined in `contract_ids` module:

| Constant | Symbol | Contract |
|----------|--------|----------|
| `STEALTH_REGISTRY` | `st_reg` | stealth-registry |
| `STEALTH_SENDER` | `st_send` | stealth-sender |
| `STEALTH_BATCH_SENDER` | `st_bat_sd` | stealth-batch-sender |
| `STEALTH_ANNOUNCER` | `st_ann` | stealth-announcer |

## Standard Dimension Names

Defined in `dimension_names` module:

| Constant | Symbol | Description |
|----------|--------|-------------|
| `SCHEME_ID` | `scheme_id` | Stealth address scheme identifier |
| `TOKEN_ADDRESS` | `tok_addr` | Token contract address |
| `ASSET_ADDRESS` | `ast_addr` | Asset contract address |
| `ERROR_CODE` | `err_code` | Error code |

## Event Schema

All metric events use the following event topic pattern:

```rust
env.events().publish(
    (symbol_short!("metric"), contract_name, metric_name),
    (value, dimensions),
);
```

## Pause / Admin / Metrics Posture

| Feature | Status |
|---------|--------|
| Pausable | N/A — library crate |
| Admin | N/A — library crate |
| Metrics | N/A — this library provides metrics infrastructure |

## Related Docs

- [METRICS.md](../METRICS.md) — Full metrics standard documentation with indexer implementation and Prometheus exporter format

## Integration with Indexer

A reference indexer implementation is provided in `stellar/scripts/metrics-indexer/` that:
1. Connects to a Stellar RPC node
2. Subscribes to contract events
3. Parses WraithMetricEvent format
4. Aggregates metrics in memory
5. Exposes metrics in Prometheus format

See [METRICS.md](../METRICS.md) for details on the indexer and dashboard integration.

## Testing

This library has no standalone tests (it is a pure utility library). Testing is done by the contracts that use it.
