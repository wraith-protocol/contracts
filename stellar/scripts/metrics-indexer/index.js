import { Server, Api } from '@stellar/stellar-sdk';
import express from 'express';
import { Registry, Counter, Gauge } from 'prom-client';

// Configuration
const RPC_URL = process.env.RPC_URL || 'https://futurenet.sorobanrpc.com';
const NETWORK_PASSPHRASE = process.env.NETWORK_PASSPHRASE || 'Test SDF Future Network ; October 2022';
const PORT = process.env.PORT || 9090;
const POLL_INTERVAL_MS = process.env.POLL_INTERVAL_MS || 5000;

// Contract addresses to monitor (add your deployed contract addresses here)
const CONTRACTS = {
  stealthRegistry: process.env.STEALTH_REGISTRY_ADDRESS,
  stealthSender: process.env.STEALTH_SENDER_ADDRESS,
  stealthBatchSender: process.env.STEALTH_BATCH_SENDER_ADDRESS,
};

// Initialize Stellar RPC server
const server = new Server(RPC_URL);

// Initialize Prometheus registry
const register = new Registry();

// Define Prometheus metrics
const metrics = {
  registerCount: new Counter({
    name: 'wraith_register_count',
    help: 'Total number of registrations',
    labelNames: ['contract', 'scheme_id'],
    registers: [register],
  }),
  removeCount: new Counter({
    name: 'wraith_remove_count',
    help: 'Total number of removals',
    labelNames: ['contract', 'scheme_id'],
    registers: [register],
  }),
  sendCount: new Counter({
    name: 'wraith_send_count',
    help: 'Total number of sends',
    labelNames: ['contract', 'scheme_id', 'token_address'],
    registers: [register],
  }),
  sendVolume: new Gauge({
    name: 'wraith_send_volume',
    help: 'Total volume sent',
    labelNames: ['contract', 'scheme_id', 'token_address'],
    registers: [register],
  }),
  batchSendCount: new Counter({
    name: 'wraith_batch_send_count',
    help: 'Total number of batch sends',
    labelNames: ['contract', 'scheme_id', 'token_address', 'asset_address'],
    registers: [register],
  }),
  batchSendVolume: new Gauge({
    name: 'wraith_batch_send_volume',
    help: 'Total volume sent in batches',
    labelNames: ['contract', 'scheme_id', 'token_address', 'asset_address'],
    registers: [register],
  }),
  batchSize: new Gauge({
    name: 'wraith_batch_size',
    help: 'Size of batch operations',
    labelNames: ['contract', 'scheme_id', 'token_address', 'asset_address'],
    registers: [register],
  }),
};

// In-memory metric storage
const metricStore = {
  registerCount: new Map(),
  removeCount: new Map(),
  sendCount: new Map(),
  sendVolume: new Map(),
  batchSendCount: new Map(),
  batchSendVolume: new Map(),
  batchSize: new Map(),
};

// Generate key for metric storage
function getMetricKey(contract, dimensions) {
  const dimString = Object.entries(dimensions)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([k, v]) => `${k}=${v}`)
    .join(',');
  return `${contract}:${dimString}`;
}

// Parse metric event from Soroban event
function parseMetricEvent(event) {
  try {
    const topic = event.topic;
    const value = event.value;

    // Check if this is a metric event (topic[0] should be "metric")
    if (topic.length < 3 || topic[0] !== 'metric') {
      return null;
    }

    const contract = topic[1];
    const metricName = topic[2];

    // Parse dimensions from value
    const dimensions = {};
    if (value && value.length >= 2) {
      const dimArray = value[1];
      if (Array.isArray(dimArray)) {
        for (const [key, val] of dimArray) {
          dimensions[key] = val;
        }
      }
    }

    const metricValue = value[0];

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
function updateMetric(metricEvent) {
  const { contract, metricName, value, dimensions } = metricEvent;
  const key = getMetricKey(contract, dimensions);

  switch (metricName) {
    case 'register_count':
      const currentRegCount = metricStore.registerCount.get(key) || 0;
      metricStore.registerCount.set(key, currentRegCount + value);
      metrics.registerCount.inc({
        contract,
        scheme_id: dimensions.scheme_id || 'unknown',
      });
      break;

    case 'remove_count':
      const currentRemoveCount = metricStore.removeCount.get(key) || 0;
      metricStore.removeCount.set(key, currentRemoveCount + value);
      metrics.removeCount.inc({
        contract,
        scheme_id: dimensions.scheme_id || 'unknown',
      });
      break;

    case 'send_count':
      const currentSendCount = metricStore.sendCount.get(key) || 0;
      metricStore.sendCount.set(key, currentSendCount + value);
      metrics.sendCount.inc({
        contract,
        scheme_id: dimensions.scheme_id || 'unknown',
        token_address: dimensions.token_address || 'unknown',
      });
      break;

    case 'send_volume':
      const currentSendVolume = metricStore.sendVolume.get(key) || 0;
      metricStore.sendVolume.set(key, currentSendVolume + value);
      metrics.sendVolume.set({
        contract,
        scheme_id: dimensions.scheme_id || 'unknown',
        token_address: dimensions.token_address || 'unknown',
      }, currentSendVolume + value);
      break;

    case 'batch_send_count':
      const currentBatchCount = metricStore.batchSendCount.get(key) || 0;
      metricStore.batchSendCount.set(key, currentBatchCount + value);
      metrics.batchSendCount.inc({
        contract,
        scheme_id: dimensions.scheme_id || 'unknown',
        token_address: dimensions.token_address || 'unknown',
        asset_address: dimensions.asset_address || 'unknown',
      });
      break;

    case 'batch_send_volume':
      const currentBatchVolume = metricStore.batchSendVolume.get(key) || 0;
      metricStore.batchSendVolume.set(key, currentBatchVolume + value);
      metrics.batchSendVolume.set({
        contract,
        scheme_id: dimensions.scheme_id || 'unknown',
        token_address: dimensions.token_address || 'unknown',
        asset_address: dimensions.asset_address || 'unknown',
      }, currentBatchVolume + value);
      break;

    case 'batch_size':
      metricStore.batchSize.set(key, value);
      metrics.batchSize.set({
        contract,
        scheme_id: dimensions.scheme_id || 'unknown',
        token_address: dimensions.token_address || 'unknown',
        asset_address: dimensions.asset_address || 'unknown',
      }, value);
      break;

    default:
      console.log(`Unknown metric name: ${metricName}`);
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

// Start the server
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
