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
export WRAITH_NAMES_ADDRESS="CC..."
export STEALTH_SPLITTER_ADDRESS="CC..."
export STEALTH_VAULT_ADDRESS="CC..."
export GOVERNANCE_ADDRESS="CC..."
```

Any address left unset is skipped, so the indexer can be pointed at a subset of
the deployment.

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
- `wraith_lookup_count` - Total number of meta-address lookups (counter)
- `wraith_send_count` - Total number of sends (counter)
- `wraith_send_volume` - Total volume sent (gauge)
- `wraith_batch_send_count` - Total number of batch sends (counter)
- `wraith_batch_send_volume` - Total volume sent in batches (gauge)
- `wraith_batch_size` - Size of batch operations (gauge)
- `wraith_error_count` - Total number of contract errors (counter)
- `wraith_renew_count` - Total number of name renewals (counter)
- `wraith_release_count` - Total number of name releases (counter)
- `wraith_resolve_hit_count` - Name resolutions that found an entry (counter)
- `wraith_resolve_miss_count` - Name resolutions that found nothing (counter)
- `wraith_create_count` - Total number of split definitions created (counter)
- `wraith_fund_count` - Total number of split fundings (counter)
- `wraith_fund_volume` - Total volume distributed through splits (gauge)
- `wraith_beneficiaries_per_split` - Beneficiaries in the newest split (gauge)
- `wraith_deposit_count` - Total number of vault deposits (counter)
- `wraith_deposit_volume` - Total volume locked in vault deposits (gauge)
- `wraith_claim_count` - Total number of vault claims (counter)
- `wraith_refund_count` - Total number of vault refunds (counter)
- `wraith_proposal_count` - Total number of governance proposals (counter)
- `wraith_vote_count` - Total number of governance votes cast (counter)
- `wraith_execution_count` - Total number of proposals executed (counter)

All metrics include labels for:
- `contract` - Contract identifier
- `scheme_id` - Stealth address scheme (where applicable)
- `token_address` - Token contract address (where applicable)
- `asset_address` - Asset contract address (where applicable)
- `proposal_id` - Governance proposal identifier (where applicable)
- `support` - Vote direction, `true`/`false` (governance votes only)

Dimensions a metric does not carry are reported as `unknown` rather than being
dropped, so a series never silently disappears from a dashboard panel.

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
- Name register/renew/release rates and resolution hit ratio
- Splitter create/fund rates, fund volume, and beneficiaries per split
- Vault deposit/claim/refund rates and deposit volume by asset
- Governance proposal, vote, and execution rates

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

Soroban `Symbol`s are capped at 9 characters, so contracts emit abbreviated
topics (`wr_names`, `res_miss`, ...). `index.js` maps them back to the canonical
names via `CONTRACT_SYMBOLS`, `METRIC_SYMBOLS`, and `DIMENSION_SYMBOLS`; keep
those tables in sync with `stellar/wraith-metrics/src/lib.rs`.

See `stellar/METRICS.md` for the full specification, including the complete
symbol table.

## Testing

### Verify the dashboard against a synthetic event stream

`synthetic-stream.js` replays a hand-written WraithMetricEvent stream — one
event for every metric in `stellar/METRICS.md` — through the same parser and
aggregator the live indexer uses, then checks that every `wraith_*` metric
referenced by a `grafana-dashboard.json` panel expression has a sample in the
scrape output. A panel that would render blank fails the check.

```bash
npm run verify:dashboard        # exits non-zero if any panel has no data
node synthetic-stream.js --print  # also dumps the Prometheus scrape output
```

### Against futurenet

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
