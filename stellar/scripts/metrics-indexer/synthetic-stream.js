#!/usr/bin/env node
//
// Feeds a synthetic WraithMetricEvent stream through the indexer's parser and
// aggregator, then checks that every Prometheus metric referenced by
// grafana-dashboard.json is present in the exported scrape output.
//
// This is the offline substitute for pointing Grafana at a live network: if a
// panel references a metric no contract emits (or that the indexer drops on the
// floor), the panel renders blank and this script fails.
//
//   node synthetic-stream.js          # verify
//   node synthetic-stream.js --print  # verify and dump the scrape output

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { parseMetricEvent, updateMetric, register } from './index.js';

const HERE = dirname(fileURLToPath(import.meta.url));
const DASHBOARD_PATH = join(HERE, 'grafana-dashboard.json');

const ASSET_A = 'CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC';
const ASSET_B = 'CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA';

// Events as the contracts emit them: abbreviated wire symbols in the topic,
// (value, dimensions) in the data. Every metric in stellar/METRICS.md appears
// at least once so the dashboard has something to draw for each panel.
const SYNTHETIC_EVENTS = [
  // stealth-registry
  ['st_reg', 'reg_cnt', 1, [['scheme_id', 1]]],
  ['st_reg', 'reg_cnt', 1, [['scheme_id', 1]]],
  ['st_reg', 'rem_cnt', 1, [['scheme_id', 1]]],
  ['st_reg', 'lkp_cnt', 1, [['scheme_id', 1]]],

  // stealth-sender
  ['st_send', 'send_cnt', 1, [['scheme_id', 1], ['tok_addr', ASSET_A]]],
  ['st_send', 'send_vol', 5_000_000, [['scheme_id', 1], ['tok_addr', ASSET_A]]],
  ['st_send', 'bat_send', 1, [['scheme_id', 1], ['tok_addr', ASSET_A]]],
  ['st_send', 'bat_vol', 12_000_000, [['scheme_id', 1], ['tok_addr', ASSET_A]]],
  ['st_send', 'bat_size', 4, [['scheme_id', 1], ['tok_addr', ASSET_A]]],

  // stealth-batch-sender
  ['st_bat_sd', 'bat_send', 1, [['ast_addr', ASSET_B]]],
  ['st_bat_sd', 'bat_vol', 3_000_000, [['ast_addr', ASSET_B]]],
  ['st_bat_sd', 'bat_size', 12, [['ast_addr', ASSET_B]]],

  // wraith-names
  ['wr_names', 'reg_cnt', 1, []],
  ['wr_names', 'reg_cnt', 1, []],
  ['wr_names', 'reg_cnt', 1, []],
  ['wr_names', 'renew_cnt', 1, []],
  ['wr_names', 'renew_cnt', 5, []], // bulk_renew carries the batch size
  ['wr_names', 'rel_cnt', 1, []],
  ['wr_names', 'res_hit', 1, []],
  ['wr_names', 'res_hit', 1, []],
  ['wr_names', 'res_miss', 1, []],

  // stealth-splitter
  ['st_split', 'crt_cnt', 1, [['ast_addr', ASSET_A]]],
  ['st_split', 'benef_cnt', 3, [['ast_addr', ASSET_A]]],
  ['st_split', 'fund_cnt', 1, [['ast_addr', ASSET_A]]],
  ['st_split', 'fund_vol', 9_000_000, [['ast_addr', ASSET_A]]],

  // stealth-vault
  ['st_vault', 'dep_cnt', 1, [['ast_addr', ASSET_A]]],
  ['st_vault', 'dep_vol', 1_000_000, [['ast_addr', ASSET_A]]],
  ['st_vault', 'dep_cnt', 1, [['ast_addr', ASSET_B]]],
  ['st_vault', 'dep_vol', 2_500_000, [['ast_addr', ASSET_B]]],
  ['st_vault', 'clm_cnt', 1, [['ast_addr', ASSET_A]]],
  ['st_vault', 'rfnd_cnt', 1, [['ast_addr', ASSET_B]]],

  // governance
  ['gov', 'prop_cnt', 1, [['prop_id', 0]]],
  ['gov', 'vote_cnt', 1, [['prop_id', 0], ['support', true]]],
  ['gov', 'vote_cnt', 1, [['prop_id', 0], ['support', false]]],
  ['gov', 'exec_cnt', 1, [['prop_id', 0]]],
].map(([contract, metricName, value, dimensions]) => ({
  topic: ['metric', contract, metricName],
  value: [value, dimensions],
}));

// Pull every `wraith_*` metric name out of the dashboard's panel expressions.
function dashboardMetricNames() {
  const dashboard = JSON.parse(readFileSync(DASHBOARD_PATH, 'utf8'));
  const referenced = new Map();

  for (const panel of dashboard.panels ?? []) {
    for (const target of panel.targets ?? []) {
      for (const name of target.expr.match(/wraith_[a-z0-9_]+/g) ?? []) {
        if (!referenced.has(name)) {
          referenced.set(name, []);
        }
        referenced.get(name).push(panel.title);
      }
    }
  }

  return referenced;
}

// A metric is only "rendered" if the scrape carries a sample for it, not just a
// HELP/TYPE header — prom-client emits the header for a labelled collector that
// has never been observed.
function sampledMetricNames(scrape) {
  const sampled = new Set();
  for (const line of scrape.split('\n')) {
    if (line.startsWith('#') || line.trim() === '') {
      continue;
    }
    const name = line.split(/[{ ]/)[0];
    if (name) {
      sampled.add(name);
    }
  }
  return sampled;
}

async function main() {
  let parsed = 0;
  for (const event of SYNTHETIC_EVENTS) {
    const metricEvent = parseMetricEvent(event);
    if (!metricEvent) {
      console.error(`FAIL: could not parse synthetic event ${JSON.stringify(event.topic)}`);
      process.exit(1);
    }
    updateMetric(metricEvent);
    parsed += 1;
  }

  const scrape = await register.metrics();
  const sampled = sampledMetricNames(scrape);
  const referenced = dashboardMetricNames();

  const missing = [];
  for (const [name, panels] of referenced) {
    if (!sampled.has(name)) {
      missing.push(`${name} (panels: ${[...new Set(panels)].join(', ')})`);
    }
  }

  if (process.argv.includes('--print')) {
    console.log(scrape);
  }

  console.log(`Replayed ${parsed} synthetic metric events.`);
  console.log(`Dashboard references ${referenced.size} distinct wraith_* metrics.`);

  if (missing.length > 0) {
    console.error('FAIL: dashboard panels reference metrics with no samples:');
    for (const entry of missing) {
      console.error(`  - ${entry}`);
    }
    process.exit(1);
  }

  console.log('OK: every dashboard panel has data to render.');
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
