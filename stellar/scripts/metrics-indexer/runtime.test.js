import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

import { redactUrl, truncate } from './redact.js';
import {
  createCursorStore,
  createPoller,
  deriveHealthState,
  dedupeEvents,
  summarizeError,
} from './runtime.js';

const baseStats = {
  polls: 4,
  failures: 0,
  skipped: 0,
  consecutiveFailures: 0,
  lastDurationMs: 12,
  lastSuccessAt: 9_000,
};

describe('createPoller', () => {
  it('never overlaps two polls and skips concurrent manual runs', async () => {
    let active = 0;
    let maxActive = 0;

    const run = async () => {
      active += 1;
      maxActive = Math.max(maxActive, active);
      await new Promise((resolve) => setTimeout(resolve, 10));
      active -= 1;
    };

    const poller = createPoller({ intervalMs: 1, run, sleep: () => Promise.resolve() });

    const [first, second] = await Promise.all([poller.runOnce(), poller.runOnce()]);

    assert.equal(maxActive, 1);
    assert.equal(poller.stats.polls, 1);
    assert.equal(poller.stats.skipped, 1);
    assert.equal(first.skipped, false);
    assert.equal(second.skipped, true);
  });

  it('serializes scheduled polls and shuts down cleanly', async () => {
    let active = 0;
    let maxActive = 0;
    let runs = 0;

    const poller = createPoller({
      intervalMs: 5,
      run: async () => {
        active += 1;
        maxActive = Math.max(maxActive, active);
        runs += 1;
        await new Promise((resolve) => setTimeout(resolve, 5));
        active -= 1;
      },
      sleep: () => new Promise((resolve) => setTimeout(resolve, 1)),
    });

    assert.equal(poller.start(), true);
    assert.equal(poller.start(), false);
    await new Promise((resolve) => setTimeout(resolve, 40));
    await poller.stop();

    assert.ok(runs >= 1);
    assert.equal(maxActive, 1);
    assert.equal(poller.running, false);
    assert.equal(poller.inFlight, false);
  });

  it('counts failures and clears the streak on the next success', async () => {
    let shouldFail = true;
    const poller = createPoller({
      intervalMs: 1,
      run: async () => {
        if (shouldFail) {
          throw new Error('rpc down');
        }
        return 'ok';
      },
      sleep: () => Promise.resolve(),
    });

    await poller.runOnce();
    await poller.runOnce();
    assert.equal(poller.stats.consecutiveFailures, 2);
    assert.equal(poller.stats.failures, 2);

    shouldFail = false;
    await poller.runOnce();
    assert.equal(poller.stats.consecutiveFailures, 0);
    assert.equal(poller.stats.failures, 2);
  });
});

describe('deriveHealthState', () => {
  const shared = {
    now: 10_000,
    startedAt: 0,
    staleAfterMs: 5_000,
    failureThreshold: 3,
    rpcHost: 'https://rpc.example.com',
    pollIntervalMs: 1000,
  };

  it('reports healthy after a recent successful poll', () => {
    const health = deriveHealthState({ ...shared, stats: { ...baseStats }, lastFailures: 0 });

    assert.equal(health.status, 'healthy');
    assert.equal(health.stale, false);
    assert.equal(health.lastSuccessAt, 9_000);
    assert.equal(health.rpcHost, 'https://rpc.example.com');
    assert.deepEqual(health.polls, { total: 4, failed: 0, skipped: 0, lastDurationMs: 12 });
    assert.equal(health.lastError, null);
  });

  it('reports degraded when the last success is stale', () => {
    const health = deriveHealthState({
      ...shared,
      stats: { ...baseStats, lastSuccessAt: 1_000 },
      lastFailures: 0,
    });

    assert.equal(health.status, 'degraded');
    assert.equal(health.stale, true);
  });

  it('reports degraded on a partial contract failure', () => {
    const health = deriveHealthState({
      ...shared,
      stats: { ...baseStats, lastSuccessAt: 9_500 },
      lastFailures: 1,
      lastError: { message: 'RpcTimeoutError: RPC getEvents timed out after 10000ms', at: 9_600 },
    });

    assert.equal(health.status, 'degraded');
    assert.ok(health.lastError.message.length < 200);
  });

  it('reports unhealthy once the failure threshold is reached', () => {
    const health = deriveHealthState({
      ...shared,
      stats: { ...baseStats, failures: 3, consecutiveFailures: 3, lastSuccessAt: 1_000 },
      lastFailures: 2,
    });

    assert.equal(health.status, 'unhealthy');
    assert.equal(health.consecutiveFailures, 3);
    assert.equal(health.polls.failed, 3);
  });

  it('reports degraded before the very first success', () => {
    const health = deriveHealthState({
      ...shared,
      stats: { ...baseStats, polls: 0, lastSuccessAt: null },
      lastFailures: 0,
    });

    assert.equal(health.status, 'degraded');
    assert.equal(health.lastSuccessAt, null);
  });
});

describe('redaction and truncation', () => {
  it('drops credentials, path secrets, query, and fragment from URLs', () => {
    assert.equal(
      redactUrl('https://user:pass@rpc.example.com/v1/secret-key?api_key=abc#frag'),
      'https://rpc.example.com'
    );
    assert.equal(redactUrl('http://localhost:9090'), 'http://localhost:9090');
    assert.equal(redactUrl('not a url'), '<invalid-url>');
    assert.equal(redactUrl(''), '<unset>');
    assert.equal(redactUrl(undefined), '<unset>');
  });

  it('bounds long strings', () => {
    const out = truncate('a'.repeat(1000), 100);
    assert.ok(out.startsWith('a'.repeat(100)));
    assert.ok(out.length < 200);
  });

  it('summarizes errors without exposing unbounded messages', () => {
    const long = new Error('z'.repeat(5000));
    const summary = summarizeError(long);
    assert.ok(summary.startsWith('Error: zzz'));
    assert.ok(summary.length < 400);
    assert.equal(summarizeError(null), 'unknown error');
  });
});

describe('cursor persistence', () => {
  let dir;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), 'metrics-cursor-'));
  });

  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it('returns the default cursor when no state file exists', async () => {
    const store = createCursorStore({ filePath: join(dir, 'cursor.json') });
    const state = await store.load();
    assert.equal(state.cursor, store.defaultCursor);
    assert.deepEqual(state.seenIds, []);
  });

  it('persists the confirmed cursor across restarts', async () => {
    const filePath = join(dir, 'cursor.json');
    const first = createCursorStore({ filePath });
    await first.commit({ cursor: '1234567890-0', seenIds: ['evt-1', 'evt-2'] });

    const second = createCursorStore({ filePath });
    const state = await second.load();
    assert.equal(state.cursor, '1234567890-0');
    assert.deepEqual(state.seenIds, ['evt-1', 'evt-2']);
  });

  it('rewinds to the last confirmed cursor on reorg', async () => {
    const filePath = join(dir, 'cursor.json');
    const store = createCursorStore({ filePath });
    await store.commit({ cursor: '100-0', seenIds: ['evt-1'] });
    await store.commit({ cursor: '200-0', seenIds: ['evt-2'] });

    const rewinded = await store.rewind();
    assert.equal(rewinded.cursor, '100-0');
    assert.deepEqual(rewinded.seenIds, ['evt-1']);

    const reloaded = await createCursorStore({ filePath }).load();
    assert.equal(reloaded.cursor, '100-0');
  });

  it('recovers from corrupted state files without losing the default cursor', async () => {
    const filePath = join(dir, 'cursor.json');
    await writeFile(filePath, '{ not json');

    const store = createCursorStore({ filePath });
    const state = await store.load();
    assert.equal(state.cursor, store.defaultCursor);

    const raw = await readFile(filePath, 'utf8');
    assert.doesNotThrow(() => JSON.parse(raw));
  });

  it('rejects invalid commits', async () => {
    const store = createCursorStore({ filePath: join(dir, 'cursor.json') });
    await assert.rejects(() => store.commit({ cursor: '' }));
  });
});

describe('dedupeEvents', () => {
  it('filters events already seen in previous polls', () => {
    const seen = ['evt-1', 'evt-2'];
    const events = [
      { id: 'evt-1', tx: 'a' },
      { id: 'evt-3', tx: 'b' },
      { id: 'evt-2', tx: 'c' },
      { id: 'evt-4', tx: 'd' },
    ];
    const result = dedupeEvents(events, seen);
    assert.deepEqual(result.fresh.map((e) => e.id), ['evt-3', 'evt-4']);
    assert.deepEqual(result.seenIds, ['evt-1', 'evt-2', 'evt-3', 'evt-4']);
  });

  it('deduplicates within a single batch', () => {
    const events = [
      { id: 'evt-1' },
      { id: 'evt-1' },
      { id: 'evt-2' },
    ];
    const result = dedupeEvents(events, []);
    assert.deepEqual(result.fresh.map((e) => e.id), ['evt-1', 'evt-2']);
  });

  it('bounds the seen cache size', () => {
    const events = Array.from({ length: 10 }, (_, i) => ({ id: `evt-${i}` }));
    const result = dedupeEvents(events, [], { maxSeenIds: 3 });
    assert.equal(result.seenIds.length, 3);
    assert.deepEqual(result.seenIds, ['evt-7', 'evt-8', 'evt-9']);
  });
});
