import { resolve } from 'node:path';

import express from 'express';
import { Registry, Counter, Gauge } from 'prom-client';

import {
  createFileCheckpointStore,
  CursorIntegrityError,
  emptyCheckpoint,
  emptyContractState,
  planPage,
} from './cursor-store.js';
import { createRpcClient } from './rpc-client.js';
import { redactUrl } from './redact.js';
import { createPoller, deriveHealthState, summarizeError } from './runtime.js';

// Configuration
const RPC_URL = process.env.RPC_URL || 'https://futurenet.sorobanrpc.com';
const NETWORK_PASSPHRASE =
  process.env.NETWORK_PASSPHRASE || 'Test SDF Future Network ; October 2022';
const PORT = process.env.PORT || 9090;

function positiveInt(raw, fallback) {
  const parsed = Number.parseInt(raw ?? '', 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

const POLL_INTERVAL_MS = positiveInt(process.env.POLL_INTERVAL_MS, 5000);
// Bound the RPC work: every request gets an abortable timeout, a small retry
// budget with jittered backoff, and a cap on how many response bytes we read.
const RPC_TIMEOUT_MS = positiveInt(process.env.RPC_TIMEOUT_MS, 10_000);
const RPC_MAX_ATTEMPTS = positiveInt(process.env.RPC_MAX_ATTEMPTS, 4);
const RPC_RETRY_BASE_MS = positiveInt(process.env.RPC_RETRY_BASE_MS, 250);
const RPC_RETRY_MAX_MS = positiveInt(process.env.RPC_RETRY_MAX_MS, 5_000);
const RPC_MAX_RESPONSE_BYTES = positiveInt(process.env.RPC_MAX_RESPONSE_BYTES, 1_048_576);
const HEALTH_STALE_AFTER_MS = positiveInt(
  process.env.HEALTH_STALE_AFTER_MS,
  POLL_INTERVAL_MS * 3
);
const HEALTH_FAILURE_THRESHOLD = positiveInt(process.env.HEALTH_FAILURE_THRESHOLD, 3);

// Durable ingestion state: aggregates and per-contract cursors are committed
// together to this file (see cursor-store.js).
const INDEXER_STATE_PATH = resolve(process.env.INDEXER_STATE_PATH || 'metrics-indexer-state.json');
const EVENTS_PAGE_LIMIT = 100;
// How many ledgers a provider may lag behind the committed position before its
// pages are refused instead of deduplicated.
const MAX_REWIND_LEDGERS = positiveInt(process.env.MAX_REWIND_LEDGERS, 120);
// Consecutive full pages that move nothing before the cursor counts as stalled.
const CURSOR_STALL_POLLS = positiveInt(process.env.CURSOR_STALL_POLLS, 3);
// Cursor used before anything has been committed (the RPC's default start).
const DEFAULT_CURSOR = '0';

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

// Initialize the bounded RPC client. It owns the timeout, retry, and response
// cap policy so the polling loop below stays free of transport concerns.
const server = createRpcClient({
  url: RPC_URL,
  timeoutMs: RPC_TIMEOUT_MS,
  maxAttempts: RPC_MAX_ATTEMPTS,
  retryBaseMs: RPC_RETRY_BASE_MS,
  retryMaxMs: RPC_RETRY_MAX_MS,
  maxResponseBytes: RPC_MAX_RESPONSE_BYTES,
  onRetry: ({ method, attempt, delay, error }) => {
    console.warn(
      `Retrying RPC ${method} (attempt ${attempt}) in ${delay}ms after ${summarizeError(error)}`
    );
  },
});

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

const KNOWN_METRICS = new Set(Object.keys(METRIC_SPECS));

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
    console.error(`Error parsing metric event: ${summarizeError(error)}`);
    return null;
  }
}

/**
 * Raised when the indexer has no contract to poll. This is a configuration
 * fault, not a transient RPC failure: it must surface as a poll failure
 * (degraded/unhealthy) rather than a silent no-op success.
 */
export class IndexerConfigurationError extends Error {
  constructor(message) {
    super(message);
    this.name = 'IndexerConfigurationError';
  }
}

/**
 * Build one indexer: Prometheus collectors, aggregates, and the polling body.
 *
 * Aggregates and per-contract cursors are committed together through `store`
 * before anything is published to Prometheus, so a crash between "events
 * counted" and "cursor saved" cannot happen: on restart the indexer reloads
 * both from the same checkpoint and resumes from that cursor.
 *
 * @param {object} options
 * @param {{ getEvents: Function }} options.client Bounded RPC client.
 * @param {Record<string, string|undefined>} options.contracts Name -> address.
 * @param {{ load: () => object|null, save: (checkpoint: object) => void }} options.store
 * @param {Registry} [options.registry]
 * @param {number} [options.pageLimit] Events requested per contract per poll.
 * @param {number} [options.maxRewindLedgers] How far a provider may lag behind.
 * @param {number} [options.stallPolls] Full, non-advancing pages before a stall is reported.
 * @param {() => number} [options.now]
 */
export function createIndexer({
  client,
  contracts = {},
  store,
  registry = new Registry(),
  pageLimit = EVENTS_PAGE_LIMIT,
  maxRewindLedgers = MAX_REWIND_LEDGERS,
  stallPolls = CURSOR_STALL_POLLS,
  now = () => Date.now(),
} = {}) {
  // Instantiate one Prometheus collector per spec. Every collector carries a
  // `contract` label so a single panel can break down by emitting contract.
  const metrics = {};
  for (const [metricName, spec] of Object.entries(METRIC_SPECS)) {
    const options = {
      name: spec.name,
      help: spec.help,
      labelNames: ['contract', ...spec.labels],
      registers: [registry],
    };
    metrics[metricName] = spec.kind === 'counter' ? new Counter(options) : new Gauge(options);
  }

  // Aggregates: `${metricName}|${seriesKey}` -> { metric, key, labels, value }.
  let series = new Map();
  let checkpoint = null;
  const progress = {};
  const health = { lastFailures: 0, lastError: null };

  // Fold one parsed event into `stage` and return what to publish, or null
  // when the event cannot be aggregated without corrupting a total.
  function stageMetric(stage, metricEvent) {
    const { contract, metricName, value, dimensions } = metricEvent;
    const spec = METRIC_SPECS[metricName];
    if (!spec) {
      console.log(`Unknown metric name: ${metricName}`);
      return null;
    }
    if (!Number.isFinite(value) || (spec.kind === 'counter' && value < 0)) {
      console.warn(`Ignoring invalid value for ${metricName}`);
      return null;
    }
    const key = `${metricName}|${getMetricKey(contract, dimensions)}`;
    const labels = buildLabels(spec, contract, dimensions);
    const total = spec.kind === 'gauge' ? value : (stage.get(key)?.value ?? 0) + value;
    stage.set(key, { metric: metricName, key, labels, value: total });
    return { metricName, kind: spec.kind, labels, value, total };
  }

  function publish(op) {
    if (op.kind === 'counter') {
      metrics[op.metricName].inc(op.labels, op.value);
    } else {
      metrics[op.metricName].set(op.labels, op.total);
    }
  }

  // Aggregate without persistence (synthetic-stream verification).
  function updateMetric(metricEvent) {
    const op = stageMetric(series, metricEvent);
    if (op) {
      publish(op);
    }
  }

  // Load the checkpoint once. A corrupt checkpoint fails every poll (and so
  // /health) until an operator fixes or removes it; it is never overwritten.
  function ensureLoaded() {
    if (checkpoint) {
      return;
    }
    const doc = store.load() ?? emptyCheckpoint();
    const restored = new Map();
    for (const entry of doc.series) {
      restored.set(entry.key, { ...entry });
      publish({
        metricName: entry.metric,
        kind: METRIC_SPECS[entry.metric].kind,
        labels: entry.labels,
        value: entry.value,
        total: entry.value,
      });
    }
    series = restored;
    checkpoint = doc;
  }

  // Fetch one page of events for a contract through the bounded RPC client.
  // The client enforces an abortable timeout, a bounded retry budget with
  // jitter, and a cap on the response body it will read.
  async function fetchContractEvents(contractAddress, cursor, signal) {
    return client.getEvents(
      {
        filters: [
          {
            type: 'contract',
            contractIds: [contractAddress],
          },
        ],
        cursor,
        limit: pageLimit,
      },
      { signal }
    );
  }

  async function pollContract(name, address, signal) {
    const state = checkpoint.contracts[address] ?? emptyContractState();
    const response = await fetchContractEvents(address, state.cursor ?? DEFAULT_CURSOR, signal);
    const plan = planPage({ state, response, limit: pageLimit, maxRewindLedgers });

    const stage = new Map(series);
    const ops = [];
    for (const event of plan.events) {
      const metricEvent = parseMetricEvent(event);
      const op = metricEvent && stageMetric(stage, metricEvent);
      if (op) {
        ops.push(op);
      }
    }

    const track = (progress[name] ??= { duplicatesSkipped: 0, stalledPolls: 0, lastAdvancedAt: null });
    if (plan.advanced) {
      const next = {
        version: checkpoint.version,
        contracts: { ...checkpoint.contracts, [address]: plan.next },
        series: [...stage.values()],
      };
      // Commit first: if this throws, neither the cursor nor any aggregate
      // moved, and the same page is fetched and counted once on the next poll.
      store.save(next);
      checkpoint = next;
      series = stage;
      ops.forEach(publish);
      track.lastAdvancedAt = now();
    }
    track.duplicatesSkipped += plan.duplicates;
    track.scannedLedger = plan.next.scannedLedger;
    track.lastEventId = plan.next.lastEventId;
    // A full page that moves nothing means the provider keeps serving the
    // same events: the cursor is stuck and new events are not being read.
    track.stalledPolls = !plan.advanced && response.events.length >= pageLimit ? track.stalledPolls + 1 : 0;
    if (track.stalledPolls >= stallPolls) {
      throw new CursorIntegrityError('stalled', `cursor has not advanced for ${track.stalledPolls} full pages`);
    }
  }

  // Poll every configured contract once.
  //
  // A failure for one address is tolerated so it cannot stall the others; the
  // poll only rejects when every configured contract failed, which is what turns
  // the health endpoint unhealthy. Errors are logged through `summarizeError`,
  // so no provider body is ever replayed in full.
  //
  // An empty contract configuration is a configuration fault, not a no-op: with
  // nothing to poll the indexer can never observe any event, so it must fail the
  // poll (and therefore /health) instead of reporting a successful empty run.
  async function processEvents({ signal, contracts: override = contracts } = {}) {
    const configured = Object.entries(override).filter(([, address]) => Boolean(address));

    if (configured.length === 0) {
      health.lastFailures = 0;
      health.lastError = {
        message: 'no contracts configured; set at least one *_ADDRESS environment variable',
        at: now(),
      };
      throw new IndexerConfigurationError(
        'no contracts configured: set at least one *_ADDRESS environment variable'
      );
    }

    try {
      ensureLoaded();
    } catch (error) {
      health.lastFailures = configured.length;
      health.lastError = { message: summarizeError(error), at: now() };
      throw error;
    }

    const failures = [];
    let succeeded = 0;

    for (const [name, address] of configured) {
      try {
        await pollContract(name, address, signal);
        succeeded += 1;
      } catch (error) {
        if (signal?.aborted) {
          throw error;
        }
        failures.push({ contract: name, error });
        console.error(`Error fetching events for ${name}: ${summarizeError(error)}`);
      }
    }

    health.lastFailures = failures.length;
    if (failures.length > 0) {
      health.lastError = { message: summarizeError(failures[0].error), at: now() };
    }

    if (configured.length > 0 && succeeded === 0) {
      throw new Error(`all ${configured.length} configured contract poll(s) failed`);
    }

    return { succeeded, failed: failures.length };
  }

  // Per-contract progress for /health. The opaque cursor itself is omitted.
  function cursorProgress() {
    const out = {};
    for (const [name, track] of Object.entries(progress)) {
      out[name] = {
        scannedLedger: track.scannedLedger ?? null,
        lastEventId: track.lastEventId ?? null,
        lastAdvancedAt: track.lastAdvancedAt,
        duplicatesSkipped: track.duplicatesSkipped,
        stalled: track.stalledPolls >= stallPolls,
      };
    }
    return out;
  }

  return { register: registry, health, processEvents, updateMetric, cursorProgress };
}

const indexer = createIndexer({
  client: server,
  contracts: CONTRACTS,
  store: createFileCheckpointStore({ path: INDEXER_STATE_PATH, knownMetrics: KNOWN_METRICS }),
});

// Initialize Prometheus registry
export const register = indexer.register;

// Update metric storage
export function updateMetric(metricEvent) {
  indexer.updateMetric(metricEvent);
}

export function processEvents(options) {
  return indexer.processEvents(options);
}

// Health + poll state shared with the /health endpoint.
const startedAt = Date.now();
let activePollController = null;

// Serialized poller: a slow poll delays the next tick instead of overlapping
// it, and `runOnce` guards against any concurrent manual invocation.
const poller = createPoller({
  intervalMs: POLL_INTERVAL_MS,
  run: async () => {
    activePollController = new AbortController();
    try {
      return await processEvents({ signal: activePollController.signal });
    } finally {
      activePollController = null;
    }
  },
  onError: (error) => console.error(`Poll failed: ${summarizeError(error)}`),
});

// Public, bounded health snapshot. Secrets stay out of it (only the redacted
// RPC host is reported) and the error summary is truncated.
export function healthSnapshot(now = Date.now()) {
  return {
    ...deriveHealthState({
      now,
      startedAt,
      stats: poller.stats,
      lastFailures: indexer.health.lastFailures,
      lastError: indexer.health.lastError,
      staleAfterMs: HEALTH_STALE_AFTER_MS,
      failureThreshold: HEALTH_FAILURE_THRESHOLD,
      rpcHost: redactUrl(RPC_URL),
      pollIntervalMs: POLL_INTERVAL_MS,
    }),
    cursors: indexer.cursorProgress(),
  };
}

export function start() {
  // Start Express server for Prometheus metrics
  const app = express();

  app.get('/metrics', async (req, res) => {
    try {
      res.set('Content-Type', register.contentType);
      res.end(await register.metrics());
    } catch (error) {
      res.status(500).end(summarizeError(error));
    }
  });

  // Degraded/unhealthy status is reported here. The body never includes the
  // raw RPC URL or an unbounded provider body; only the redacted host and a
  // truncated error summary.
  app.get('/health', (req, res) => {
    const snapshot = healthSnapshot();
    res.status(snapshot.status === 'unhealthy' ? 503 : 200).json(snapshot);
  });

  const httpServer = app.listen(PORT, () => {
    console.log(`Metrics indexer listening on port ${PORT}`);
    console.log(`Prometheus metrics available at http://localhost:${PORT}/metrics`);
    console.log(`RPC host: ${redactUrl(RPC_URL)}`);
    console.log(`Network: ${NETWORK_PASSPHRASE}`);
    console.log(
      `Polling every ${POLL_INTERVAL_MS}ms ` +
        `(timeout ${RPC_TIMEOUT_MS}ms, up to ${RPC_MAX_ATTEMPTS} attempts)`
    );

    // Initial poll and then a serialized loop; a slow poll cannot overlap the
    // next one.
    poller.start();
  });

  let shuttingDown = false;
  async function shutdown(signalName) {
    if (shuttingDown) {
      return;
    }
    shuttingDown = true;
    console.log(`${signalName} received, shutting down gracefully`);

    // Abort any in-flight RPC call, then let the loop finish.
    activePollController?.abort();
    await poller.stop();

    httpServer.close(() => process.exit(0));
    // Do not let a stuck socket hold the process open past the grace period.
    setTimeout(() => process.exit(0), 5000).unref();
  }

  process.on('SIGTERM', () => void shutdown('SIGTERM'));
  process.on('SIGINT', () => void shutdown('SIGINT'));

  return httpServer;
}

// Only start the HTTP server when run directly, so the parser and aggregator
// can be imported by verification scripts (see synthetic-stream.js).
if (process.argv[1] && import.meta.url === new URL(`file://${process.argv[1]}`).href) {
  start();
}
