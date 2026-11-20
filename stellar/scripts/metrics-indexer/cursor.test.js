// Restart, replay, rollback, retention-gap, corruption, and stall scenarios,
// driven through the production polling body (`createIndexer().processEvents`)
// with the real RPC client and the real file-backed checkpoint store. Only the
// HTTP transport is scripted.

import { afterEach, beforeEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { nativeToScVal, xdr } from '@stellar/stellar-sdk';

import { CursorIntegrityError, CursorStateCorruptError, createFileCheckpointStore } from './cursor-store.js';
import { createIndexer } from './index.js';
import { createRpcClient } from './rpc-client.js';

const ADDRESS = 'C'.repeat(55) + 'R';
const CONTRACTS = { stealthRegistry: ADDRESS };
const RPC_URL = 'https://user:secret@rpc.example.com/v1/topsecret?api_key=supersecret';

const symbol = (value) => xdr.ScVal.scvSymbol(value).toXDR('base64');

function metricValue(value, dimensions = [['scheme_id', 1]]) {
  const entries = dimensions.map(([key, val]) =>
    xdr.ScVal.scvVec([xdr.ScVal.scvSymbol(key), nativeToScVal(val, { type: 'i128' })])
  );
  return xdr.ScVal.scvVec([nativeToScVal(value, { type: 'i128' }), xdr.ScVal.scvVec(entries)]).toXDR(
    'base64'
  );
}

// One raw (wire-format) getEvents event at `ledger`, position `index`.
function event(ledger, index, metric = 'reg_cnt', value = 1) {
  return {
    type: 'contract',
    ledger,
    contractId: ADDRESS,
    id: `${String(ledger).padStart(19, '0')}-${String(index).padStart(10, '0')}`,
    topic: [symbol('metric'), symbol('st_reg'), symbol(metric)],
    value: metricValue(value),
  };
}

// Scripted RPC: `respond(requestParams)` returns a getEvents result.
function scriptedClient(respond) {
  const requests = [];
  const client = createRpcClient({
    url: RPC_URL,
    maxAttempts: 1,
    sleep: async () => {},
    fetchImpl: async (url, init) => {
      const params = JSON.parse(init.body).params;
      requests.push(params);
      return new Response(JSON.stringify({ jsonrpc: '2.0', id: 1, result: respond(params) }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      });
    },
  });
  return { client, requests };
}

async function total(indexer, name) {
  const metric = await indexer.register.getSingleMetric(name).get();
  return metric.values.reduce((sum, sample) => sum + sample.value, 0);
}

describe('reorg-safe cursor through the production poll loop', () => {
  let dir;
  let statePath;

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'metrics-indexer-'));
    statePath = join(dir, 'state.json');
  });

  afterEach(() => {
    rmSync(dir, { recursive: true, force: true });
  });

  const store = () => createFileCheckpointStore({ path: statePath });
  const build = (client, options = {}) =>
    createIndexer({ client, contracts: CONTRACTS, store: store(), ...options });

  it('resumes from the committed cursor and restored totals after a restart', async () => {
    const first = scriptedClient(() => ({
      latestLedger: 10,
      cursor: 'c-10',
      events: [event(9, 1), event(10, 1, 'send_vol', 250)],
    }));
    const before = build(first.client);
    await before.processEvents();
    assert.equal(first.requests[0].pagination.cursor, '0');
    assert.equal(await total(before, 'wraith_register_count'), 1);

    // Restart: a new process with fresh collectors and the same state file.
    const second = scriptedClient((params) => {
      assert.equal(params.pagination.cursor, 'c-10');
      return { latestLedger: 12, cursor: 'c-12', events: [event(12, 1), event(12, 2, 'send_vol', 100)] };
    });
    const after = build(second.client);
    await after.processEvents();

    assert.equal(second.requests.length, 1);
    assert.equal(await total(after, 'wraith_register_count'), 2);
    assert.equal(await total(after, 'wraith_send_volume'), 350);
    const saved = JSON.parse(readFileSync(statePath, 'utf8'));
    assert.equal(saved.contracts[ADDRESS].cursor, 'c-12');
    assert.equal(saved.contracts[ADDRESS].lastEventId, event(12, 2).id);
    const savedTotals = Object.fromEntries(saved.series.map((entry) => [entry.metric, entry.value]));
    assert.deepEqual(savedTotals, { register_count: 2, send_volume: 350 });
  });

  it('counts a duplicated page once', async () => {
    const page = { latestLedger: 10, cursor: 'c-10', events: [event(9, 1), event(10, 1)] };
    const { client } = scriptedClient(() => page);
    const indexer = build(client);

    await indexer.processEvents();
    await indexer.processEvents();

    assert.equal(await total(indexer, 'wraith_register_count'), 2);
    assert.equal(indexer.cursorProgress().stealthRegistry.duplicatesSkipped, 2);
  });

  it('applies nothing from a page whose commit fails, then counts it once', async () => {
    const { client, requests } = scriptedClient(() => ({
      latestLedger: 10,
      cursor: 'c-10',
      events: [event(9, 1), event(10, 1)],
    }));
    const real = store();
    let failNext = true;
    const flaky = {
      load: real.load,
      save: (checkpoint) => {
        if (failNext) {
          failNext = false;
          throw new Error('ENOSPC: no space left on device');
        }
        real.save(checkpoint);
      },
    };
    const indexer = createIndexer({ client, contracts: CONTRACTS, store: flaky });

    await assert.rejects(indexer.processEvents(), /all 1 configured contract poll\(s\) failed/);
    assert.equal(await total(indexer, 'wraith_register_count'), 0);
    assert.throws(() => readFileSync(statePath), { code: 'ENOENT' });

    await indexer.processEvents();
    assert.equal(requests[1].pagination.cursor, '0');
    assert.equal(await total(indexer, 'wraith_register_count'), 2);
  });

  it('refuses a page it cannot deduplicate without applying any of it', async () => {
    const broken = { ...event(10, 2) };
    delete broken.id;
    const { client } = scriptedClient(() => ({ latestLedger: 10, cursor: 'c-10', events: [event(10, 1), broken] }));
    const indexer = build(client);

    await assert.rejects(indexer.processEvents());
    assert.equal(await total(indexer, 'wraith_register_count'), 0);
    assert.match(indexer.health.lastError.message, /no valid id/);
    assert.throws(() => readFileSync(statePath), { code: 'ENOENT' });
  });

  it('deduplicates a lagging provider within the rewind bound and never moves backward', async () => {
    let lagging = false;
    const { client } = scriptedClient(() =>
      lagging
        ? { latestLedger: 95, cursor: 'c-95', events: [event(90, 1), event(95, 1)] }
        : { latestLedger: 100, cursor: 'c-100', events: [event(90, 1), event(95, 1)] }
    );
    const indexer = build(client, { maxRewindLedgers: 10 });
    await indexer.processEvents();

    lagging = true;
    await indexer.processEvents();

    assert.equal(await total(indexer, 'wraith_register_count'), 2);
    const saved = JSON.parse(readFileSync(statePath, 'utf8')).contracts[ADDRESS];
    assert.equal(saved.cursor, 'c-100');
    assert.equal(saved.scannedLedger, 100);
  });

  it('fails closed on a rollback beyond the rewind bound', async () => {
    let rolledBack = false;
    const { client } = scriptedClient(() =>
      rolledBack
        ? { latestLedger: 50, cursor: 'c-50', events: [event(48, 1)] }
        : { latestLedger: 100, cursor: 'c-100', events: [event(99, 1)] }
    );
    const indexer = build(client, { maxRewindLedgers: 10 });
    await indexer.processEvents();
    const committed = readFileSync(statePath, 'utf8');

    rolledBack = true;
    await assert.rejects(indexer.processEvents());

    assert.match(indexer.health.lastError.message, /more than 10 ledgers behind/);
    assert.equal(readFileSync(statePath, 'utf8'), committed);
    assert.equal(await total(indexer, 'wraith_register_count'), 1);
  });

  it('fails closed instead of skipping events lost to a retention gap', async () => {
    let pruned = false;
    const { client } = scriptedClient(() =>
      pruned
        ? { latestLedger: 400, oldestLedger: 300, cursor: 'c-400', events: [event(350, 1)] }
        : { latestLedger: 100, oldestLedger: 1, cursor: 'c-100', events: [event(99, 1)] }
    );
    const indexer = build(client);
    await indexer.processEvents();
    const committed = readFileSync(statePath, 'utf8');

    pruned = true;
    await assert.rejects(indexer.processEvents());

    assert.match(indexer.health.lastError.message, /retention starts at ledger 300/);
    assert.equal(readFileSync(statePath, 'utf8'), committed);
    assert.equal(await total(indexer, 'wraith_register_count'), 1);
  });

  for (const [label, contents] of [
    ['unparsable', '{"version":1,"contracts":'],
    ['schema-invalid', JSON.stringify({ version: 1, contracts: { [ADDRESS]: { cursor: 7 } }, series: [] })],
    ['unknown metric', JSON.stringify({ version: 1, contracts: {}, series: [{ metric: 'nope', key: 'k', labels: {}, value: 1 }] })],
  ]) {
    it(`fails closed on ${label} cursor storage and leaves it untouched`, async () => {
      writeFileSync(statePath, contents);
      const { client, requests } = scriptedClient(() => ({ latestLedger: 10, cursor: 'c-10', events: [event(10, 1)] }));
      const indexer = createIndexer({
        client,
        contracts: CONTRACTS,
        store: createFileCheckpointStore({ path: statePath, knownMetrics: new Set(['register_count']) }),
      });

      await assert.rejects(indexer.processEvents(), CursorStateCorruptError);
      await assert.rejects(indexer.processEvents(), CursorStateCorruptError);

      assert.equal(requests.length, 0);
      assert.equal(readFileSync(statePath, 'utf8'), contents);
      assert.ok(indexer.health.lastError.message.startsWith('CursorStateCorruptError'));
      assert.equal(await total(indexer, 'wraith_register_count'), 0);
    });
  }

  it('reports a stalled cursor when full pages stop advancing', async () => {
    const page = { latestLedger: 20, cursor: 'c-10', events: [event(9, 1), event(10, 1)] };
    const { client } = scriptedClient(() => page);
    const indexer = build(client, { pageLimit: 2, stallPolls: 2 });

    await indexer.processEvents();
    await indexer.processEvents();
    assert.equal(indexer.cursorProgress().stealthRegistry.stalled, false);
    await assert.rejects(indexer.processEvents());

    const progress = indexer.cursorProgress().stealthRegistry;
    assert.equal(progress.stalled, true);
    assert.match(indexer.health.lastError.message, /cursor has not advanced for 2 full pages/);
    assert.equal(await total(indexer, 'wraith_register_count'), 2);
  });

  it('reports progress without the opaque cursor or RPC credentials', async () => {
    const { client } = scriptedClient(() => ({ latestLedger: 10, cursor: 'opaque-cursor-token', events: [event(10, 1)] }));
    const indexer = build(client);
    await indexer.processEvents();

    const progress = indexer.cursorProgress();
    assert.deepEqual(Object.keys(progress.stealthRegistry).sort(), [
      'duplicatesSkipped',
      'lastAdvancedAt',
      'lastEventId',
      'scannedLedger',
      'stalled',
    ]);
    assert.equal(progress.stealthRegistry.scannedLedger, 10);
    const serialized = JSON.stringify({ progress, health: indexer.health });
    for (const secret of ['opaque-cursor-token', 'supersecret', 'topsecret', 'user:secret']) {
      assert.ok(!serialized.includes(secret), secret);
    }
  });

  it('exposes CursorIntegrityError kinds for operators', () => {
    assert.equal(new CursorIntegrityError('rollback', 'x').kind, 'rollback');
  });
});
