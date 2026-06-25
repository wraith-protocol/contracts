# Wraith Metrics Indexer

Reference indexer for consuming Wraith Protocol Stellar contract metrics events and exporting them in Prometheus format.

## Overview

This indexer connects to a Stellar RPC node, subscribes to contract events, parses the standardized WraithMetricEvent format, aggregates metrics in memory, and exposes them via a Prometheus-compatible HTTP endpoint.

## Installation

```bash
cd stellar/scripts/metrics-indexer
npm install
```

## Configuration

Set environment variables to configure the indexer:

```bash
# Stellar RPC endpoint
export RPC_URL="https://futurenet.sorobanrpc.com"

# Network passphrase
export NETWORK_PASSPHRASE="Test SDF Future Network ; October 2022"

# HTTP server port for Prometheus metrics
export PORT=9090

# Polling interval for new events (milliseconds)
export POLL_INTERVAL_MS=5000

# Contract addresses to monitor
export STEALTH_REGISTRY_ADDRESS="CC..."
export STEALTH_SENDER_ADDRESS="CC..."
export STEALTH_BATCH_SENDER_ADDRESS="CC..."
```

## Usage

### Start the indexer

```bash
npm start
```

### Development mode with auto-reload

```bash
npm run dev
```

### Access metrics

The Prometheus metrics endpoint is available at:

```
http://localhost:9090/metrics
```

Health check endpoint:

```
http://localhost:9090/health
```

## Metrics Exported

The indexer exports the following Prometheus metrics:

- `wraith_register_count` - Total number of registrations (counter)
- `wraith_remove_count` - Total number of removals (counter)
- `wraith_send_count` - Total number of sends (counter)
- `wraith_send_volume` - Total volume sent (gauge)
- `wraith_batch_send_count` - Total number of batch sends (counter)
- `wraith_batch_send_volume` - Total volume sent in batches (gauge)
- `wraith_batch_size` - Size of batch operations (gauge)

All metrics include labels for:
- `contract` - Contract identifier
- `scheme_id` - Stealth address scheme (where applicable)
- `token_address` - Token contract address (where applicable)
- `asset_address` - Asset contract address (where applicable)

## Integration with Prometheus

Configure Prometheus to scrape the indexer:

```yaml
scrape_configs:
  - job_name: 'wraith-metrics'
    static_configs:
      - targets: ['localhost:9090']
```

## Grafana Dashboard

A demo Grafana dashboard is provided in `grafana-dashboard.json`. Import it into your Grafana instance to visualize:

- Registration/removal rates
- Send volume trends
- Batch operation statistics
- Contract activity breakdown

To import:

1. Open Grafana
2. Go to Dashboards → Import
3. Upload `grafana-dashboard.json`
4. Configure the Prometheus datasource

## Architecture

The indexer:

1. Polls the Stellar RPC for contract events at configured intervals
2. Parses event topics to identify WraithMetricEvent format
3. Extracts metric name, value, and dimensions from event data
4. Aggregates metrics in memory (counters increment, gauges set absolute values)
5. Exposes aggregated metrics via Prometheus text format

## Event Format

The indexer expects events in the WraithMetricEvent format:

**Topic:** `(metric, contract_identifier, metric_name)`

**Data:** `(value, dimensions)`

See `stellar/METRICS.md` for the full specification.

## Testing

To test against futurenet:

```bash
export RPC_URL="https://futurenet.sorobanrpc.com"
export NETWORK_PASSPHRASE="Test SDF Future Network ; October 2022"
export STEALTH_REGISTRY_ADDRESS="<your-contract-address>"
npm start
```

Then invoke contract functions and observe metrics being updated at `/metrics`.

## Production Considerations

For production use:

1. Use a persistent storage backend (e.g., Redis, PostgreSQL) instead of in-memory storage
2. Implement event cursor persistence to avoid reprocessing events
3. Add authentication/authorization to the metrics endpoint
4. Configure appropriate retention policies in Prometheus
5. Set up alerts based on metric thresholds
6. Use a production Stellar RPC endpoint (testnet or mainnet)

## License

MIT
