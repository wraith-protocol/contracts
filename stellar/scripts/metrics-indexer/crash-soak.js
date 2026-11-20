import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { CursorStateCorruptError, createFileCheckpointStore } from './cursor-store.js';
import { createIndexer } from './index.js';

const ADDRESS = 'C'.repeat(55) + 'S';
const CONTRACTS = { stealthRegistry: ADDRESS };
const SCRIPT_PATH = fileURLToPath(import.meta.url);
const WORKER_ENV = 'WRAITH_METRICS_CRASH_WORKER';
const WORKER_EXIT_CODES = { before: 71, after: 72 };

function event(ledger, index, value) {
  return {
    type: 'contract',
    ledger,
    contractId: ADDRESS,
    id: `${String(ledger).padStart(19, '0')}-${String(index).padStart(10, '0')}`,
    topic: ['metric', 'st_reg', 'reg_cnt'],
    value: [value, [['scheme_id', 1]]],
  };
}

function metricTotal(indexer) {
  return indexer.register
    .getSingleMetric('wraith_register_count')
    .get()
    .then((metric) => metric.values.reduce((sum, sample) => sum + sample.value, 0));
}

function createClient(page, requests = []) {
  return {
    async getEvents(params) {
      requests.push(params);
      return page;
    },
  };
}

async function expectPollFailure(run, expected) {
  const originalError = console.error;
  const errors = [];
  console.error = (...parts) => errors.push(parts.join(' '));
  try {
    await assert.rejects(run(), expected);
    assert.ok(errors.length > 0, 'expected poll failure was not logged');
  } finally {
    console.error = originalError;
  }
}

function createMemoryStore() {
  let checkpoint = null;
  return {
    load: () => checkpoint,
    save(next) {
      checkpoint = JSON.parse(JSON.stringify(next));
    },
    snapshot: () => checkpoint && JSON.parse(JSON.stringify(checkpoint)),
  };
}

function createRandom(seed) {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let value = state;
    value = Math.imul(value ^ (value >>> 15), value | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return ((value ^ (value >>> 14)) >>> 0) / 4294967296;
  };
}

async function runCrashWorker() {
  const config = JSON.parse(process.env[WORKER_ENV] ?? 'null');
  if (!config || !Object.hasOwn(WORKER_EXIT_CODES, config.boundary)) {
    throw new Error('crash worker configuration is missing or invalid');
  }

  const fileStore = createFileCheckpointStore({ path: config.statePath });
  const store = {
    load: () => fileStore.load(),
    save(checkpoint) {
      if (config.boundary === 'before') {
        process.exit(WORKER_EXIT_CODES.before);
      }
      fileStore.save(checkpoint);
      process.exit(WORKER_EXIT_CODES.after);
    },
  };
  const indexer = createIndexer({
    client: createClient(config.page),
    contracts: CONTRACTS,
    store,
    pageLimit: 2,
  });

  await indexer.processEvents();
  throw new Error('crash injection did not terminate the worker');
}

async function verifyCrashBoundaries(root) {
  const page = {
    latestLedger: 20,
    cursor: 'crash-cursor-20',
    events: [event(19, 1, 2), event(20, 1, 3)],
  };
  const expectedTotal = 5;

  for (const boundary of ['before', 'after']) {
    const statePath = join(root, `crash-${boundary}.json`);
    const child = spawnSync(process.execPath, [SCRIPT_PATH, '--worker'], {
      encoding: 'utf8',
      timeout: 15_000,
      env: {
        ...process.env,
        [WORKER_ENV]: JSON.stringify({ boundary, page, statePath }),
      },
    });

    assert.ifError(child.error);
    assert.equal(child.signal, null, `${boundary}-write worker was signalled`);
    assert.equal(child.status, WORKER_EXIT_CODES[boundary], child.stderr);

    if (boundary === 'before') {
      assert.equal(existsSync(statePath), false, 'kill-before-write persisted a checkpoint');
    } else {
      const saved = JSON.parse(readFileSync(statePath, 'utf8'));
      assert.equal(saved.contracts[ADDRESS].cursor, page.cursor);
      assert.equal(saved.series[0].value, expectedTotal);
    }

    const requests = [];
    const restarted = createIndexer({
      client: createClient(page, requests),
      contracts: CONTRACTS,
      store: createFileCheckpointStore({ path: statePath }),
      pageLimit: 2,
      stallPolls: 3,
    });
    await restarted.processEvents();
    assert.equal(requests[0].cursor, boundary === 'before' ? '0' : page.cursor);
    assert.equal(await metricTotal(restarted), expectedTotal);

    const committed = readFileSync(statePath, 'utf8');
    await restarted.processEvents();
    assert.equal(readFileSync(statePath, 'utf8'), committed);
    assert.equal(await metricTotal(restarted), expectedTotal);
  }
}

async function verifyTruncatedState(path) {
  const contents = '{"version":1,"contracts":';
  writeFileSync(path, contents);
  let requests = 0;
  const indexer = createIndexer({
    client: {
      async getEvents() {
        requests += 1;
        return { latestLedger: 1, cursor: 'unexpected', events: [] };
      },
    },
    contracts: CONTRACTS,
    store: createFileCheckpointStore({ path }),
  });

  await assert.rejects(indexer.processEvents(), CursorStateCorruptError);
  assert.equal(requests, 0, 'truncated checkpoint must fail before polling');
  assert.equal(readFileSync(path, 'utf8'), contents, 'truncated checkpoint was overwritten');
}

export async function runCrashSoak({ seed = 279, pages = 5000 } = {}) {
  if (!Number.isSafeInteger(seed) || seed < 0 || seed > 0xffffffff) {
    throw new RangeError('seed must be an integer from 0 to 4294967295');
  }
  if (!Number.isSafeInteger(pages) || pages < 1 || pages > 100_000) {
    throw new RangeError('pages must be an integer from 1 to 100000');
  }

  const root = mkdtempSync(join(tmpdir(), 'wraith-metrics-crash-soak-'));
  const statePath = join(root, 'indexer-state.json');
  const random = createRandom(seed);
  const counters = {
    generatedPages: pages,
    generatedEvents: 0,
    duplicatePages: 0,
    rewoundPages: 0,
    reorderedDuplicatePages: 0,
    emptyPages: 0,
    restarts: 0,
    killBeforeWrite: 1,
    killAfterWrite: 1,
    truncatedStates: 1,
    cursorStallChecks: 1,
  };

  try {
    await verifyCrashBoundaries(root);
    await verifyTruncatedState(join(root, 'truncated.json'));

    let currentPage = null;
    let ledger = 0;
    let expectedTotal = 0;
    let actualTotal = 0;
    const store = createMemoryStore();
    const buildIndexer = () =>
      createIndexer({
        client: {
          async getEvents() {
            return currentPage;
          },
        },
        contracts: CONTRACTS,
        store,
        pageLimit: 2,
        stallPolls: 3,
      });
    let indexer = buildIndexer();
    let previousPage = null;

    for (let pageNumber = 1; pageNumber <= pages; pageNumber += 1) {
      if (pageNumber % 23 === 0) {
        currentPage = {
          latestLedger: ledger + 1,
          cursor: `empty-cursor-${pageNumber}`,
          events: [],
        };
        ledger += 1;
        await indexer.processEvents();
        counters.emptyPages += 1;
      }

      const events = [];
      for (let index = 0; index < 2; index += 1) {
        const value = 1 + Math.floor(random() * 5);
        ledger += 1;
        events.push(event(ledger, index, value));
        expectedTotal += value;
      }
      currentPage = {
        latestLedger: ledger,
        cursor: `page-cursor-${pageNumber}`,
        events,
      };

      if (pageNumber % 13 === 0) {
        const committed = store.snapshot();
        currentPage = { ...currentPage, events: [events[1], events[0], events[1]] };
        await expectPollFailure(
          () => indexer.processEvents(),
          /all 1 configured contract poll\(s\) failed/
        );
        assert.deepEqual(store.snapshot(), committed);
        assert.equal(await metricTotal(indexer), actualTotal);
        counters.reorderedDuplicatePages += 1;
        currentPage = { ...currentPage, events };
      }

      await indexer.processEvents();
      actualTotal = expectedTotal;
      counters.generatedEvents += events.length;

      const acceptedPage = currentPage;
      const restarted = pageNumber % 97 === 0;
      if (restarted) {
        const committed = store.snapshot();
        indexer = buildIndexer();
        await indexer.processEvents();
        assert.deepEqual(store.snapshot(), committed);
        assert.equal(await metricTotal(indexer), expectedTotal);

        if (previousPage) {
          currentPage = previousPage;
          await indexer.processEvents();
          assert.deepEqual(store.snapshot(), committed);
          assert.equal(await metricTotal(indexer), expectedTotal);
          counters.rewoundPages += 1;
        }

        currentPage = acceptedPage;
        counters.restarts += 1;
      }

      if (pageNumber % 5 === 0 && !restarted) {
        const committed = store.snapshot();
        await indexer.processEvents();
        assert.deepEqual(store.snapshot(), committed);
        assert.equal(await metricTotal(indexer), expectedTotal);
        counters.duplicatePages += 1;
      }

      if (pageNumber % 7 === 0 && pageNumber > 1 && !restarted) {
        const previousPage = {
          latestLedger: ledger - 2,
          cursor: `page-cursor-${pageNumber - 1}`,
          events: [
            event(ledger - 3, 0, 1),
            event(ledger - 2, 1, 1),
          ],
        };
        const committed = store.snapshot();
        currentPage = previousPage;
        await indexer.processEvents();
        assert.deepEqual(store.snapshot(), committed);
        assert.equal(await metricTotal(indexer), expectedTotal);
        counters.rewoundPages += 1;
      }

      previousPage = acceptedPage;
    }

    const lastCommittedPage = currentPage;
    const committed = store.snapshot();
    const stalledIndexer = createIndexer({
      client: createClient(lastCommittedPage),
      contracts: CONTRACTS,
      store,
      pageLimit: 2,
      stallPolls: 3,
    });
    await stalledIndexer.processEvents();
    await stalledIndexer.processEvents();
    await expectPollFailure(
      () => stalledIndexer.processEvents(),
      /all 1 configured contract poll\(s\) failed/
    );
    assert.deepEqual(store.snapshot(), committed);
    assert.equal(await metricTotal(stalledIndexer), expectedTotal);

    const report = {
      seed,
      ...counters,
      metricsTotal: expectedTotal,
      verification: 'passed',
      replay: `npm run test:crash-soak -- --seed ${seed} --pages ${pages}`,
    };
    return report;
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

function parseOptions(args) {
  const options = { seed: 279, pages: 5000, report: null };
  for (let index = 0; index < args.length; index += 1) {
    const option = args[index];
    if (!['--seed', '--pages', '--report'].includes(option) || args[index + 1] === undefined) {
      throw new Error(`unexpected or incomplete option: ${option}`);
    }
    const value = args[++index];
    if (option === '--seed') {
      options.seed = Number(value);
    } else if (option === '--pages') {
      options.pages = Number(value);
    } else {
      options.report = resolve(value);
    }
  }
  return options;
}

async function main() {
  const options = parseOptions(process.argv.slice(2));
  const report = await runCrashSoak(options);
  const serialized = `${JSON.stringify(report, null, 2)}\n`;
  if (options.report) {
    writeFileSync(options.report, serialized);
  }
  process.stdout.write(serialized);
}

if (process.env[WORKER_ENV]) {
  await runCrashWorker();
} else if (process.argv[1] && resolve(process.argv[1]) === SCRIPT_PATH) {
  main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
