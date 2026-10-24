import express from 'express';
import { Registry, Counter, Gauge } from 'prom-client';
import { promises as fs } from 'node:fs';
import path from 'node:path';

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

// Cursor persistence. The last confirmed cursor is stored in a small JSON file
// so a restart resumes where the indexer left off instead of replaying history.
// `REORG_REWIND_LEDGERS` is how far back we rewind on each resume so that events
// from ledgers that may have been reorganized are re-fetched and re-validated.
const CURSOR_FILE =
  process.env.CURSOR_FILE || path.join(process.cwd(), 'metrics-indexer.cursor.json');
const REORG_REWIND_LEDGERS = positiveInt(process.env.REORG_REWIND_LEDGERS, 32);
const DEDUP_WINDOW = positiveInt(process.env.DEDUP_WINDOW, 10_000);

// Cursor persistence helpers: write the cursor atomically (temp file + rename)
// so a crash mid-write can never leave a truncated or corrupt cursor on disk.
export async function loadCursor(file = CURSOR_FILE) {
  try {
    const raw = await fs.readFile(file, 'utf8');
    const parsed = JSON.parse(raw);
    const ledger = Number(parsed?.ledger);
    return Number.isFinite(ledger) && ledger >= 0 ? ledger : null;
  } catch (error) {
    if (error?.code === 'ENOENT') return null;
    console.warn(`Ignoring unreadable cursor file: ${summarizeError(error)}`);
    return null;
  }
}

export async function saveCursor(ledger, file = CURSOR_FILE) {
  const tmp = `${file}.tmp`;
  await fs.writeFile(tmp, JSON.stringify({ ledger, updatedAt: new Date().toISOString() }));
  await fs.rename(tmp, file);
}

// Reorg-safe resume point: rewind by REORG_REWIND_LEDGERS so ledgers that may
// have been reorganized are re-fetched and re-validated. Never goes below 0.
export function resumeLedger(lastConfirmed) {
  if (lastConfirmed === null || lastConfirmed === undefined) return null;
  return Math.max(0, lastConfirmed - REORG_REWIND_LEDGERS);
}

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
    console.error(`Error parsing metric event: ${summarizeError(error)}`);
    return null;
  }
}
