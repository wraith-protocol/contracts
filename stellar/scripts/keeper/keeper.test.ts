import { describe, it, expect, vi } from 'vitest';
import { spawnSync } from 'child_process';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import {
  DecodedEvent,
  EventPage,
  KeeperError,
  KeeperRpc,
  loadIndexFile,
  nameHash,
  runKeeper,
} from './keeper-core';

const LATEST = 1_000_000;

/** Deterministic in-memory RPC. Entries map name -> liveUntil (absent = no live entry). */
function fakeRpc(opts: {
  pages?: EventPage[];
  liveUntil?: Record<string, number>;
  oldestLedger?: number;
  failTtlFor?: string;
  failExtendFor?: string;
  failEvents?: boolean;
  failLatest?: boolean;
}) {
  const extendCalls: { name: string; to: number }[] = [];
  // Mutable so the fake can model the contract: extend_name_ttl(name, to) takes a TTL
  // relative to the current ledger and tops the entry up to LATEST + to.
  const ttlByHash = new Map(
    Object.entries(opts.liveUntil ?? {}).map(([n, v]) => [nameHash(n), v] as const),
  );
  const pages = opts.pages ?? [];
  let pageIdx = 0;
  const rpc: KeeperRpc = {
    getLatestLedger: async () => {
      if (opts.failLatest) throw new Error('rpc down');
      return LATEST;
    },
    getNameEvents: async () => {
      if (opts.failEvents) throw new Error('getEvents unavailable');
      const p = pages[pageIdx++] ?? { events: [], oldestLedger: 0, latestLedger: LATEST };
      return { ...p, oldestLedger: opts.oldestLedger ?? p.oldestLedger };
    },
    getNameLiveUntil: async (h) => {
      if (opts.failTtlFor && nameHash(opts.failTtlFor) === h) throw new Error('getLedgerEntries 503');
      return ttlByHash.has(h) ? (ttlByHash.get(h) as number) : null;
    },
    extendName: async (name, to) => {
      if (opts.failExtendFor === name) throw new Error('tx failed');
      extendCalls.push({ name, to });
      const h = nameHash(name);
      const current = ttlByHash.get(h) ?? 0;
      if (current - LATEST < to) ttlByHash.set(h, LATEST + to);
      return `tx-${name}`;
    },
  };
  return { rpc, extendCalls, liveUntilOf: (n: string) => ttlByHash.get(nameHash(n)) };
}

const ev = (kind: DecodedEvent['kind'], name: string): DecodedEvent => ({ ledger: 10, kind, name });
const page = (events: DecodedEvent[], cursor?: string): EventPage => ({
  events,
  cursor,
  oldestLedger: 0,
  latestLedger: LATEST,
});
const base = { threshold: 1000, extendTo: 500_000, dryRun: false, startLedger: 1 };

describe('enumeration', () => {
  it('replays register/update/release events across pages', async () => {
    const { rpc } = fakeRpc({
      pages: [
        page([ev('register', 'alice'), ev('register', 'bob')], 'c1'),
        page([ev('release', 'bob'), ev('update', 'alice'), ev('register', 'carol')]),
      ],
      liveUntil: { alice: LATEST + 10, carol: LATEST + 10 },
    });
    const report = await runKeeper(rpc, { ...base, dryRun: true });
    expect(report.total).toBe(2);
  });

  it('merges an index file with events and de-duplicates', async () => {
    const { rpc } = fakeRpc({
      pages: [page([ev('register', 'alice')])],
      liveUntil: { alice: LATEST + 10, dave: LATEST + 10 },
    });
    const report = await runKeeper(rpc, { ...base, dryRun: true, indexedNames: ['alice', 'dave'] });
    expect(report.total).toBe(2);
  });

  it('works from an index file alone', async () => {
    const { rpc } = fakeRpc({ liveUntil: { dave: LATEST + 10 } });
    const report = await runKeeper(rpc, {
      threshold: 1000,
      extendTo: 500_000,
      dryRun: true,
      indexedNames: ['dave'],
    });
    expect(report.total).toBe(1);
  });

  it('fails closed when event history is truncated before --start-ledger', async () => {
    const { rpc } = fakeRpc({ pages: [page([ev('register', 'alice')])], oldestLedger: 500 });
    await expect(runKeeper(rpc, { ...base, startLedger: 100 })).rejects.toMatchObject({
      stage: 'enumeration',
    });
  });

  it('fails closed when the events RPC is unavailable', async () => {
    const { rpc, extendCalls } = fakeRpc({ failEvents: true });
    await expect(runKeeper(rpc, base)).rejects.toMatchObject({ stage: 'enumeration' });
    expect(extendCalls).toHaveLength(0);
  });

  it('fails closed when the latest ledger cannot be read', async () => {
    const { rpc } = fakeRpc({ failLatest: true });
    await expect(runKeeper(rpc, base)).rejects.toMatchObject({ stage: 'enumeration' });
  });

  it('refuses to run with no name source configured', async () => {
    const { rpc } = fakeRpc({});
    await expect(
      runKeeper(rpc, { threshold: 1000, extendTo: 500_000, dryRun: true }),
    ).rejects.toMatchObject({ stage: 'config' });
  });
});

describe('ttl reads and extension', () => {
  it('extends only names below the threshold, passing extendTo as a relative TTL', async () => {
    const { rpc, extendCalls } = fakeRpc({
      pages: [page([ev('register', 'alice'), ev('register', 'bob'), ev('register', 'carol')])],
      liveUntil: { alice: LATEST + 500, bob: LATEST + 5000, carol: LATEST + 999 },
    });
    const report = await runKeeper(rpc, base);
    expect(extendCalls).toEqual([
      { name: 'alice', to: 500_000 },
      { name: 'carol', to: 500_000 },
    ]);
    expect(report.atRisk).toBe(2);
    expect(report.extended.map((e) => e.txHash)).toEqual(['tx-alice', 'tx-carol']);
  });

  it('the resulting liveUntil is latest + extendTo, not latest counted twice', async () => {
    const { rpc, extendCalls, liveUntilOf } = fakeRpc({
      pages: [page([ev('register', 'alice')])],
      liveUntil: { alice: LATEST + 500 },
    });
    await runKeeper(rpc, base);
    const sent = extendCalls[0].to;
    expect(sent).toBe(base.extendTo);
    expect(sent).toBeLessThan(LATEST); // a relative TTL, never an absolute ledger
    expect(liveUntilOf('alice')! - LATEST).toBe(base.extendTo);
  });

  it('treats remaining == threshold as healthy', async () => {
    const { rpc, extendCalls } = fakeRpc({
      pages: [page([ev('register', 'alice')])],
      liveUntil: { alice: LATEST + 1000 },
    });
    await runKeeper(rpc, base);
    expect(extendCalls).toHaveLength(0);
  });

  it('does nothing and succeeds when every name is healthy', async () => {
    const { rpc, extendCalls } = fakeRpc({
      pages: [page([ev('register', 'alice')])],
      liveUntil: { alice: LATEST + 10_000 },
    });
    const report = await runKeeper(rpc, base);
    expect(report.atRisk).toBe(0);
    expect(extendCalls).toHaveLength(0);
  });

  it('fails when a TTL read errors', async () => {
    const { rpc, extendCalls } = fakeRpc({
      pages: [page([ev('register', 'alice'), ev('register', 'bob')])],
      liveUntil: { alice: LATEST + 1, bob: LATEST + 1 },
      failTtlFor: 'bob',
    });
    await expect(runKeeper(rpc, base)).rejects.toMatchObject({ stage: 'ttl-read' });
    expect(extendCalls).toHaveLength(0); // nothing submitted on partial knowledge
  });

  it('fails when a name has no live ledger entry', async () => {
    const { rpc } = fakeRpc({
      pages: [page([ev('register', 'alice')])],
      liveUntil: {},
    });
    await expect(runKeeper(rpc, base)).rejects.toMatchObject({ stage: 'ttl-read' });
  });

  it('fails with a submission error and reports prior progress', async () => {
    const { rpc, extendCalls } = fakeRpc({
      pages: [page([ev('register', 'alice'), ev('register', 'bob')])],
      liveUntil: { alice: LATEST + 1, bob: LATEST + 1 },
      failExtendFor: 'bob',
    });
    const err = await runKeeper(rpc, base).catch((e) => e);
    expect(err).toBeInstanceOf(KeeperError);
    expect(err.stage).toBe('submission');
    expect(err.message).toContain('after 1 successful');
    expect(extendCalls.map((c) => c.name)).toEqual(['alice']);
  });
});

describe('dry run', () => {
  it('reads TTLs and logs, but submits nothing', async () => {
    const { rpc, extendCalls } = fakeRpc({
      pages: [page([ev('register', 'alice')])],
      liveUntil: { alice: LATEST + 5 },
    });
    const log = vi.fn();
    const report = await runKeeper(rpc, { ...base, dryRun: true }, log);
    expect(extendCalls).toHaveLength(0);
    expect(report.atRisk).toBe(1);
    expect(report.extended).toHaveLength(0);
    expect(log.mock.calls.some((c) => String(c[0]).includes('[dry-run] would extend "alice"'))).toBe(true);
  });

  it('still fails closed on unreadable TTLs', async () => {
    const { rpc } = fakeRpc({ pages: [page([ev('register', 'alice')])], liveUntil: {} });
    await expect(runKeeper(rpc, { ...base, dryRun: true })).rejects.toMatchObject({ stage: 'ttl-read' });
  });
});

describe('option validation', () => {
  it.each([
    [{ ...base, threshold: 0 }],
    [{ ...base, threshold: Number.NaN }],
    [{ ...base, extendTo: 1000 }],
    [{ ...base, extendTo: 10 }],
  ])('rejects %j', async (opts) => {
    const { rpc } = fakeRpc({});
    await expect(runKeeper(rpc, opts)).rejects.toMatchObject({ stage: 'config' });
  });
});

describe('index file', () => {
  const tmp = (content: string) => {
    const p = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'keeper-')), 'names.json');
    fs.writeFileSync(p, content);
    return p;
  };
  it('loads a JSON array of names', () => {
    expect(loadIndexFile(tmp('["alice","bob"]'))).toEqual(['alice', 'bob']);
  });
  it('rejects malformed content', () => {
    expect(() => loadIndexFile(tmp('{"a":1}'))).toThrow(KeeperError);
    expect(() => loadIndexFile(tmp('not json'))).toThrow(KeeperError);
    expect(() => loadIndexFile('/nonexistent/names.json')).toThrow(KeeperError);
  });
});

describe('nameHash', () => {
  it('is sha256 of the utf8 name (matches contract hash_name)', () => {
    expect(nameHash('alice')).toBe(
      '2bd806c97f0e00af1a1fc3328fa763a9269723c8db8fac4f93af71db186d6e90',
    );
  });
});

describe('cli exit codes', () => {
  const cli = path.resolve(__dirname, 'keeper.ts');
  const run = (args: string[], env: Record<string, string> = {}) =>
    spawnSync('npx', ['tsx', cli, 'extend', ...args], {
      encoding: 'utf8',
      env: { PATH: process.env.PATH as string, HOME: process.env.HOME as string, ...env },
      timeout: 60_000,
    });

  it('exits nonzero without a contract id', () => {
    const r = run(['--dry-run', '--start-ledger', '1']);
    expect(r.status).toBe(1);
    expect(r.stderr).toContain('[config]');
  });

  it('exits nonzero when the RPC is unreachable (dry run, no network needed)', () => {
    const r = run([
      '--contract', 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM',
      '--rpc-url', 'http://127.0.0.1:1',
      '--start-ledger', '1',
      '--dry-run',
    ]);
    expect(r.status).toBe(1);
    expect(r.stderr).toContain('keeper failed');
  });

  it('exits nonzero when not dry-run and no secret key is set', () => {
    const r = run([
      '--contract', 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM',
      '--start-ledger', '1',
    ]);
    expect(r.status).toBe(1);
    expect(r.stderr).toContain('[config]');
  });
});
