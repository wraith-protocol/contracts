import { after, before, beforeEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';

const ADDRESS_A = 'C'.repeat(55) + 'A';
const ADDRESS_B = 'C'.repeat(55) + 'B';

// Configure the module before importing it: index.js reads env and captures
// `globalThis.fetch` at load time.
process.env.RPC_URL = 'https://user:secret@rpc.example.com/v1/topsecret?api_key=supersecret';
process.env.STEALTH_REGISTRY_ADDRESS = ADDRESS_A;
process.env.STEALTH_SENDER_ADDRESS = ADDRESS_B;
process.env.RPC_MAX_ATTEMPTS = '2';
process.env.RPC_RETRY_BASE_MS = '1';
process.env.RPC_RETRY_MAX_MS = '1';

let mode = 'both-ok';
const originalFetch = globalThis.fetch;

globalThis.fetch = async (url, init) => {
  const body = JSON.parse(init.body);
  const contractId = body.params?.filters?.[0]?.contractIds?.[0];

  if (mode === 'all-fail' || (mode === 'a-fails' && contractId === ADDRESS_A)) {
    return new Response('provider exploded', { status: 500 });
  }

  if (mode === 'malformed') {
    // A 200 envelope with neither `result` nor `error`.
    return new Response(JSON.stringify({ jsonrpc: '2.0', id: 1 }), {
      status: 200,
      headers: { 'Content-Type': 'application/json' },
    });
  }

  return new Response(
    JSON.stringify({ jsonrpc: '2.0', id: 1, result: { latestLedger: 1, events: [] } }),
    { status: 200, headers: { 'Content-Type': 'application/json' } }
  );
};

let index;

describe('metrics indexer poll wiring', () => {
  before(async () => {
    index = await import('./index.js');
  });

  beforeEach(() => {
    // Reset shared health state so each case observes only its own outcome.
    index.resetHealthForTests?.();
  });

  after(() => {
    globalThis.fetch = originalFetch;
  });

  it('succeeds when every configured contract responds', async () => {
    mode = 'both-ok';
    const result = await index.processEvents();
    assert.deepEqual(result, { succeeded: 2, failed: 0 });
  });

  it('tolerates a single contract failure and records a bounded error', async () => {
    mode = 'a-fails';
    const result = await index.processEvents();
    assert.deepEqual(result, { succeeded: 1, failed: 1 });

    const health = index.healthSnapshot();
    assert.equal(health.status, 'degraded');
    assert.ok(health.lastError);
    assert.ok(health.lastError.message.length < 300);
  });

  it('rejects when every configured contract fails', async () => {
    mode = 'all-fail';
    await assert.rejects(index.processEvents(), /all 2 configured contract poll\(s\) failed/);
  });

  it('treats a malformed JSON-RPC response as a poll failure', async () => {
    mode = 'malformed';
    await assert.rejects(index.processEvents(), /all 2 configured contract poll\(s\) failed/);
    assert.equal(index.healthSnapshot().status, 'degraded');
  });

  it('fails closed when no contracts are configured', async () => {
    await assert.rejects(index.processEvents({ contracts: {} }), (error) => {
      assert.ok(error instanceof index.IndexerConfigurationError);
      return true;
    });

    const health = index.healthSnapshot();
    assert.notEqual(health.status, 'healthy');
    assert.ok(health.lastError);
  });

  it('never exposes the raw RPC URL or its secrets in health output', async () => {
    mode = 'both-ok';
    await index.processEvents();
    const snapshot = index.healthSnapshot();
    const serialized = JSON.stringify(snapshot);
    assert.equal(snapshot.rpcHost, 'https://rpc.example.com');
    assert.ok(!serialized.includes('supersecret'));
    assert.ok(!serialized.includes('topsecret'));
    assert.ok(!serialized.includes('user:secret'));
  });
});
