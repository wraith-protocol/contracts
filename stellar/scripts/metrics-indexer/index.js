import { rpc } from '@stellar/stellar-sdk';
import express from 'express';
import { Registry, Counter, Gauge } from 'prom-client';

// Configuration
const RPC_URL = process.env.RPC_URL || 'https://futurenet.sorobanrpc.com';
const NETWORK_PASSPHRASE =
  process.env.NETWORK_PASSPHRASE || 'Test SDF Future Network ; October 2022';
const PORT = process.env.PORT || 9090;
const POLL_INTERVAL_MS = process.env.POLL_INTERVAL_MS || 5000;

// Contract addresses to monitor (add your deployed contract addresses here)
const CONTRACTS = {
  stealthRegistry: process.env.STEALTH_REGISTRY_ADDRESS,
  stealthSender: process.env.STEALTH_SENDER_ADDRESS,
  stealthBatchSender: process.env.STEALTH_BATCH_SENDER_ADDRESS,
  wraithNames: process.env.WRAITH_NAMES_ADDRESS,
  stealthSplitter: process.env.STEALTH_SPLITTER_ADDRESS,
  stealthVault: process.env.STEALTH_VAULT_ADDRESS,
  governance: process.env.GOVERNANCE_ADDRESS,
};

// Soroban Symbols are capped at 9 characters, so contracts emit abbreviated
// topics. These tables map the on-chain wire symbols back to the canonical
// names documented in stellar/METRICS.md. Keep them in sync with
// stellar/wraith-metrics/src/lib.rs.
const CONTRACT_SYMBOLS = {
  st_reg: 'stealth-registry',
  st_send: 'stealth-sender',
  st_bat_sd: 'stealth-batch-sender',
  st_ann: 'stealth-announcer',
  wr_names: 'wraith-names',
  st_split: 'stealth-splitter',
  st_vault: 'stealth-vault',
  gov: 'governance',
};

const METRIC_SYMBOLS = {
  reg_cnt: 'register_count',
  rem_cnt: 'remove_count',
  lkp_cnt: 'lookup_count',
  send_cnt: 'send_count',
  send_vol: 'send_volume',
  bat_send: 'batch_send_count',
  bat_vol: 'batch_send_volume',
  bat_size: 'batch_size',
  err_cnt: 'error_count',
  renew_cnt: 'renew_count',
  rel_cnt: 'release_count',
  res_hit: 'resolve_hit_count',
  res_miss: 'resolve_miss_count',
  crt_cnt: 'create_count',
  fund_cnt: 'fund_count',
  fund_vol: 'fund_volume',
  benef_cnt: 'beneficiaries_per_split',
  dep_cnt: 'deposit_count',
  dep_vol: 'deposit_volume',
  clm_cnt: 'claim_count',
  rfnd_cnt: 'refund_count',
  prop_cnt: 'proposal_count',
  vote_cnt: 'vote_count',
  exec_cnt: 'execution_count',
};

const DIMENSION_SYMBOLS = {
  scheme_id: 'scheme_id',
  tok_addr: 'token_address',
  ast_addr: 'asset_address',
  err_code: 'error_code',
  prop_id: 'proposal_id',
  support: 'support',
};

// Initialize Stellar RPC server
const server = new rpc.Server(RPC_URL);

// Initialize Prometheus registry
export const register = new Registry();

// Metric specifications.
//
// `kind` selects the aggregation:
//   counter    - monotonic total; each event adds its value
//   sum_gauge  - running total exposed as a gauge (volumes)
//   gauge      - last observed value (point-in-time sizes)
const METRIC_SPECS = {
  register_count: { kind: 'counter', name: 'wraith_register_count', help: 'Total number of registrations', labels: ['scheme_id'] },
  remove_count: { kind: 'counter', name: 'wraith_remove_count', help: 'Total number of removals', labels: ['scheme_id'] },
  lookup_count: { kind: 'counter', name: 'wraith_lookup_count', help: 'Total number of meta-address lookups', labels: ['scheme_id'] },
  send_count: { kind: 'counter', name: 'wraith_send_count', help: 'Total number of sends', labels: ['scheme_id', 'token_address'] },
  send_volume: { kind: 'sum_gauge', name: 'wraith_send_volume', help: 'Total volume sent', labels: ['scheme_id', 'token_address'] },
  batch_send_count: { kind: 'counter', name: 'wraith_batch_send_count', help: 'Total number of batch sends', labels: ['scheme_id', 'token_address', 'asset_address'] },
  batch_send_volume: { kind: 'sum_gauge', name: 'wraith_batch_send_volume', help: 'Total volume sent in batches', labels: ['scheme_id', 'token_address', 'asset_address'] },
  batch_size: { kind: 'gauge', name: 'wraith_batch_size', help: 'Size of batch operations', labels: ['scheme_id', 'token_address', 'asset_address'] },
  error_count: { kind: 'counter', name: 'wraith_error_count', help: 'Total number of contract errors', labels: ['error_code'] },

  // wraith-names
  renew_count: { kind: 'counter', name: 'wraith_renew_count', help: 'Total number of name renewals', labels: [] },
  release_count: { kind: 'counter', name: 'wraith_release_count', help: 'Total number of name releases', labels: [] },
  resolve_hit_count: { kind: 'counter', name: 'wraith_resolve_hit_count', help: 'Total number of name resolutions that found an entry', labels: [] },
  resolve_miss_count: { kind: 'counter', name: 'wraith_resolve_miss_count', help: 'Total number of name resolutions that found nothing', labels: [] },

  // stealth-splitter
  create_count: { kind: 'counter', name: 'wraith_create_count', help: 'Total number of split definitions created', labels: ['asset_address'] },
  fund_count: { kind: 'counter', name: 'wraith_fund_count', help: 'Total number of split fundings', labels: ['asset_address'] },
  fund_volume: { kind: 'sum_gauge', name: 'wraith_fund_volume', help: 'Total volume distributed through splits', labels: ['asset_address'] },
  beneficiaries_per_split: { kind: 'gauge', name: 'wraith_beneficiaries_per_split', help: 'Beneficiaries in the most recently created split', labels: ['asset_address'] },

  // stealth-vault
  deposit_count: { kind: 'counter', name: 'wraith_deposit_count', help: 'Total number of vault deposits', labels: ['asset_address'] },
  deposit_volume: { kind: 'sum_gauge', name: 'wraith_deposit_volume', help: 'Total volume locked in vault deposits', labels: ['asset_address'] },
  claim_count: { kind: 'counter', name: 'wraith_claim_count', help: 'Total number of vault claims', labels: ['asset_address'] },
  refund_count: { kind: 'counter', name: 'wraith_refund_count', help: 'Total number of vault refunds', labels: ['asset_address'] },

  // governance
  proposal_count: { kind: 'counter', name: 'wraith_proposal_count', help: 'Total number of governance proposals created', labels: ['proposal_id'] },
  vote_count: { kind: 'counter', name: 'wraith_vote_count', help: 'Total number of governance votes cast', labels: ['proposal_id', 'support'] },
  execution_count: { kind: 'counter', name: 'wraith_execution_count', help: 'Total number of governance proposals executed', labels: ['proposal_id'] },
};

// Instantiate one Prometheus collector per spec. Every collector carries a
// `contract` label so a single panel can break down by emitting contract.
const metrics = {};
for (const [metricName, spec] of Object.entries(METRIC_SPECS)) {
  const options = {
    name: spec.name,
    help: spec.help,
    labelNames: ['contract', ...spec.labels],
    registers: [register],
  };
  metrics[metricName] = spec.kind === 'counter' ? new Counter(options) : new Gauge(options);
}

// In-memory metric storage: metricName -> Map(seriesKey -> running total)
const metricStore = {};
for (const metricName of Object.keys(METRIC_SPECS)) {
  metricStore[metricName] = new Map();
}

// Generate key for metric storage
function getMetricKey(contract, dimensions) {
  const dimString = Object.entries(dimensions)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([k, v]) => `${k}=${v}`)
    .join(',');
  return `${contract}:${dimString}`;
}

// Build the Prometheus label set for a metric, filling absent dimensions with
// 'unknown' so a series never silently disappears from a dashboard panel.
function buildLabels(spec, contract, dimensions) {
  const labels = { contract };
  for (const label of spec.labels) {
    labels[label] = dimensions[label] !== undefined ? String(dimensions[label]) : 'unknown';
  }
  return labels;
}

// Parse metric event from Soroban event.
//
// Topics and values are expected to have already been converted to native JS
// (see `scValToNative` in @stellar/stellar-sdk) by the transport layer.
export function parseMetricEvent(event) {
  try {
    const topic = event.topic;
    const value = event.value;

    // Check if this is a metric event (topic[0] should be "metric")
    if (!Array.isArray(topic) || topic.length < 3 || topic[0] !== 'metric') {
      return null;
    }

    const contract = CONTRACT_SYMBOLS[topic[1]] || topic[1];
    const metricName = METRIC_SYMBOLS[topic[2]] || topic[2];

    // Parse dimensions from value
    const dimensions = {};
    if (value && value.length >= 2) {
      const dimArray = value[1];
      if (Array.isArray(dimArray)) {
        for (const [key, val] of dimArray) {
          dimensions[DIMENSION_SYMBOLS[key] || key] = val;
        }
      }
    }

    const metricValue = Number(value[0]);

    return {
      contract,
      metricName,
      value: metricValue,
      dimensions,
    };
  } catch (error) {
    console.error('Error parsing metric event:', error);
    return null;
  }
}

// Update metric storage
export function updateMetric(metricEvent) {
  const { contract, metricName, value, dimensions } = metricEvent;
  const spec = METRIC_SPECS[metricName];

  if (!spec) {
    console.log(`Unknown metric name: ${metricName}`);
    return;
  }

  const key = getMetricKey(contract, dimensions);
  const labels = buildLabels(spec, contract, dimensions);

  switch (spec.kind) {
    case 'counter': {
      const total = (metricStore[metricName].get(key) || 0) + value;
      metricStore[metricName].set(key, total);
      metrics[metricName].inc(labels, value);
      break;
    }

    case 'sum_gauge': {
      const total = (metricStore[metricName].get(key) || 0) + value;
      metricStore[metricName].set(key, total);
      metrics[metricName].set(labels, total);
      break;
    }

    case 'gauge': {
      metricStore[metricName].set(key, value);
      metrics[metricName].set(labels, value);
      break;
    }
  }
}

// Fetch events from a contract
async function fetchContractEvents(contractAddress, cursor = '0') {
  try {
    const response = await server.getEvents({
      filters: [
        {
          type: 'contract',
          contractIds: [contractAddress],
        },
      ],
      cursor,
      limit: 100,
    });

    return response;
  } catch (error) {
    console.error(`Error fetching events for contract ${contractAddress}:`, error);
    return null;
  }
}

// Process events from all contracts
async function processEvents() {
  console.log('Processing events...');

  for (const [name, address] of Object.entries(CONTRACTS)) {
    if (!address) {
      console.log(`Skipping ${name}: no address configured`);
      continue;
    }

    console.log(`Fetching events for ${name} (${address})...`);
    const events = await fetchContractEvents(address);

    if (events && events.events) {
      for (const event of events.events) {
        const metricEvent = parseMetricEvent(event);
        if (metricEvent) {
          console.log(`Processing metric event:`, metricEvent);
          updateMetric(metricEvent);
        }
      }
    }
  }

  console.log('Event processing complete');
}

export function start() {
  // Start Express server for Prometheus metrics
  const app = express();

  app.get('/metrics', async (req, res) => {
    try {
      res.set('Content-Type', register.contentType);
      res.end(await register.metrics());
    } catch (error) {
      res.status(500).end(error.toString());
    }
  });

  app.get('/health', (req, res) => {
    res.json({ status: 'healthy' });
  });

  app.listen(PORT, () => {
    console.log(`Metrics indexer listening on port ${PORT}`);
    console.log(`Prometheus metrics available at http://localhost:${PORT}/metrics`);
    console.log(`RPC URL: ${RPC_URL}`);
    console.log(`Network: ${NETWORK_PASSPHRASE}`);

    // Initial event processing
    processEvents();

    // Poll for new events
    setInterval(processEvents, POLL_INTERVAL_MS);
  });

  // Graceful shutdown
  process.on('SIGTERM', () => {
    console.log('SIGTERM received, shutting down gracefully');
    process.exit(0);
  });

  process.on('SIGINT', () => {
    console.log('SIGINT received, shutting down gracefully');
    process.exit(0);
  });
}

// Only start the HTTP server when run directly, so the parser and aggregator
// can be imported by verification scripts (see synthetic-stream.js).
if (process.argv[1] && import.meta.url === new URL(`file://${process.argv[1]}`).href) {
  start();
}
