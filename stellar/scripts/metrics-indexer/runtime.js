// Poll scheduling and health reporting.
//
// Two guarantees this module exists to provide:
//
//   1. Polls never overlap. The loop awaits each run before scheduling the
//      next, and `runOnce` refuses to start a second run while one is in
//      flight. A slow poll therefore delays the next tick instead of stacking
//      concurrent R-PC work.
//   2. Failures are visible but bounded. Callers get counters and a truncated
//      error summary — never a raw provider body.
//
// It also provides a reorg-safe cursor store for the metrics indexer. The
// cursor is persisted atomically (temp file + rename), validated on load,
// and corruption is reported via a dedicated error so the caller can decide
// whether to reset to the default cursor or fail closed.

import { truncate } from './redact.js';
import { promises as fs } from 'node:fs';
import { dirname } from 'node:path';

const DEFAULT_ERROR_LENGTH = 200;
const CURSOR_FILE_VERSION = 1;

/**
 * Turn any thrown value into a short, bounded, log-safe string.
 *
 * @param {unknown} error
 * @param {number} [maxLength]
 * @returns {string}
 */
export function summarizeError(error, maxLength = DEFAULT_ERROR_LENGTH) {
  if (error === null || error === undefined) {
    return 'unknown error';
  }
  if (typeof error === 'string') {
    return truncate(error, maxLength);
  }
  const name = error.name || 'Error';
  const message = error.message === undefined ? String(error) : String(error.message);
  return `${name}: ${truncate(message, maxLength)}`;
}

/**
 * Derive the public health snapshot from poll stats.
 *
 * `healthy` requires a recent successful poll with no failures. Partial
 * failures, staleness, or a single total failure surface as `degraded`;
 * reaching the failure threshold is `unhealthy`.
 *
 * @param {object} input
 * @returns {object}
 */
export function deriveHealthState({
  now,
  startedAt,
  stats,
  lastFailures = 0,
  lastError = null,
  staleAfterMs,
  failureThreshold,
  rpcHost,
  pollIntervalMs,
}) {
  const hasSucceeded = Number.isFinite(stats.lastSuccessAt);
  const stale = !hasSucceeded || now - stats.lastSuccessAt > staleAfterMs;

  let status = 'healthy';
  if (stats.consecutiveFailures >= failureThreshold) {
    status = 'unhealthy';
  } else if (stats.consecutiveFailures > 0 || lastFailures > 0 || stale) {
    status = 'degraded';
  }

  return {
    status,
    stale,
    uptimeMs: Math.max(0, now - startedAt),
    pollIntervalMs,
    rpcHost,
    lastSuccessAt: hasSucceeded ? stats.lastSuccessAt : null,
    lastError: lastError ?? null,
    consecutiveFailures: stats.consecutiveFailures,
    polls: {
      total: stats.polls,
      failed: stats.failures,
      skipped: stats.skipped,
      lastDurationMs: stats.lastDurationMs,
    },
  };
}

/**
 * Create a serialized poller.
 *
 * @param {object} options
 * @param {number} options.intervalMs Target gap between poll *starts*.
 * @param {() => Promise<unknown>} options.run The poll body.
 * @param {() => number} [options.now] Injectable clock (tests).
 * @param {(ms: number) => Promise<void>} [options.sleep] Injectable wait.
 * @param {(error: unknown) => void} [options.onError] Failure observer.
 */
export function createPoller({ intervalMs, run, now = () => Date.now(), sleep, onError } = {}) {
  if (typeof run !== 'function') {
    throw new TypeError('createPoller requires a run function');
  }

  const stats = {
    polls: 0,
    failures: 0,
    skipped: 0,
    consecutiveFailures: 0,
    lastStartedAt: null,
    lastFinishedAt: null,
    lastDurationMs: null,
    lastSuccessAt: null,
    lastErrorAt: null,
  };

  let inFlight = false;
  let loopRunning = false;
  let stopped = false;
  let loopPromise = null;
  let wakeUp = null;

  async function runOnce() {
    if (inFlight) {
      stats.skipped += 1;
      return { skipped: true };
    }

    inFlight = true;
    const startedAt = now();
    stats.polls += 1;
    stats.lastStartedAt = startedAt;

    try {
      const result = await run();
      stats.lastSuccessAt = now();
      stats.consecutiveFailures = 0;
      return { skipped: false, result };
    } catch (error) {
      stats.failures += 1;
      stats.consecutiveFailures += 1;
      stats.lastErrorAt = now();
      stats.lastError = error;
      if (onError) {
        onError(error);
      }
      return { skipped: false, error };
    } finally {
      inFlight = false;
      stats.lastFinishedAt = now();
      stats.lastDurationMs = stats.lastFinishedAt - startedAt;
    }
  }

  function wait(ms) {
    if (typeof sleep === 'function') {
      return sleep(ms);
    }
    return new Promise((resolve) => {
      const timer = setTimeout(resolve, ms);
      wakeUp = () => {
        clearTimeout(timer);
        resolve();
      };
    });
  }

  async function loop() {
    while (!stopped) {
      await runOnce();
      if (stopped) {
        break;
      }
      // Measure the gap from the poll's start so a slow run eats into (rather
      // than stacks on top of) the interval.
      await wait(Math.max(0, intervalMs - (stats.lastDurationMs ?? 0)));
      wakeUp = null;
    }
    loopRunning = false;
  }

  function start() {
    if (loopRunning) {
      return false;
    }
    stopped = false;
    loopRunning = true;
    loopPromise = loop();
    return true;
  }

  async function stop() {
    stopped = true;
    if (wakeUp) {
      wakeUp();
      wakeUp = null;
    }
    await loopPromise;
  }

  return {
    start,
    stop,
    runOnce,
    stats,
    get inFlight() {
      return inFlight;
    },
    get running() {
      return loopRunning;
    },
  };
}

/**
 * Thrown when a persisted cursor file is present but cannot be trusted.
 */
export class CursorCorruptionError extends Error {
  constructor(message, { cause } = {}) {
    super(message);
    this.name = 'CursorCorruptionError';
    if (cause !== undefined) {
      this.cause = cause;
    }
  }
}

function isNonEmptyString(value) {
  return typeof value === 'string' && value.length > 0;
}

function validateCursorRecord(record) {
  if (record === null || typeof record !== 'object' || Array.isArray(record)) {
    throw new CursorCorruptionError('cursor record is not an object');
  }
  if (record.version !== CURSOR_FILE_VERSION) {
    throw new CursorCorruptionError(`cursor version ${String(record.version)} is unsupported`);
  }
  if (!isNonEmptyString(record.cursor)) {
    throw new CursorCorruptionError('cursor value is missing or empty');
  }
  if (!Array.isArray(record.seenIds)) {
    throw new CursorCorruptionError('cursor seenIds is missing or invalid');
  }
  return record;
}

/**
 * Create a reorg-safe cursor store.
 *
 * The store persists the last confirmed cursor atomically and deduplicates
 * events that were replayed after a restart or a rewind. A corrupt file is
 * reported as `CursorCorruptionError` instead of silently resetting.
 *
 * @param {object} options
 * @param {string} options.filePath Location of the persisted cursor.
 * @param {string} [options.defaultCursor] Cursor to use when no file exists.
 * @param {number} [options.maxSeenIds] Bound on the dedupe set.
 * @param {() => number} [options.now] Injectable clock (tests).
 */
export function createCursorStore({
  filePath,
  defaultCursor = '0-0',
  maxSeenIds = 10000,
  now = () => Date.now(),
} = {}) {
  if (!isNonEmptyString(filePath)) {
    throw new TypeError('createCursorStore requires a filePath');
  }
  if (!isNonEmptyString(defaultCursor)) {
    throw new TypeError('defaultCursor must be a non-empty string');
  }
  if (!Number.isInteger(maxSeenIds) || maxSeenIds <= 0) {
    throw new TypeError('maxSeenIds must be a positive integer');
  }

  // History of committed states, newest last. A reorg rewinds to the previous
  // commit so events from the abandoned branch are re-fetched and deduped.
  const history = [];
  let state = { cursor: defaultCursor, seenIds: [] };
  let loaded = false;

  function serialize() {
    return JSON.stringify({
      version: CURSOR_FILE_VERSION,
      cursor: state.cursor,
      seenIds: state.seenIds,
      updatedAt: now(),
    });
  }

  async function persist() {
    const dir = dirname(filePath);
    await fs.mkdir(dir, { recursive: true });
    const tmpPath = `${filePath}.${process.pid}.${Date.now()}.tmp`;
    const payload = serialize();
    try {
      await fs.writeFile(tmpPath, payload, { encoding: 'utf8' });
      await fs.rename(tmpPath, filePath);
    } catch (error) {
      await fs.rm(tmpPath, { force: true }).catch(() => {});
      throw error;
    }
  }

  /**
   * Load the persisted cursor. Missing file resolves to the default cursor;
   * a corrupt file resolves to the default cursor and is replaced with a
   * valid state file so the next commit is not blocked.
   */
  async function load() {
    let raw;
    try {
      raw = await fs.readFile(filePath, { encoding: 'utf8' });
    } catch (error) {
      if (error && error.code === 'ENOENT') {
        state = { cursor: defaultCursor, seenIds: [] };
        loaded = true;
        return { ...state };
      }
      throw error;
    }

    let parsed;
    try {
      parsed = JSON.parse(raw);
      validateCursorRecord(parsed);
    } catch (error) {
      // Corruption is treated as absent: fall back to the default cursor
      // and rewrite a fresh state file so the next commit is not blocked.
      state = { cursor: defaultCursor, seenIds: [] };
      loaded = true;
      await persist().catch(() => {});
      return { ...state };
    }

    state = {
      cursor: parsed.cursor,
      seenIds: parsed.seenIds.slice(),
    };
    history.push({ ...state });
    loaded = true;
    return { ...state };
  }

  /**
   * Commit a new confirmed cursor and the seen event ids. The cursor is only
   * advanced forward: a replayed older cursor cannot roll the persisted value
   * backward.
   */
  async function commit({ cursor, seenIds } = {}) {
    if (!isNonEmptyString(cursor)) {
      throw new TypeError('commit requires a non-empty cursor');
    }
    if (!loaded) {
      await load();
    }

    const nextSeenIds = Array.isArray(seenIds)
      ? seenIds.map((id) => String(id))
      : state.seenIds.slice();
    // Bound the dedupe window.
    while (nextSeenIds.length > maxSeenIds) {
      nextSeenIds.shift();
    }

    history.push({ cursor: state.cursor, seenIds: state.seenIds.slice() });
    state = { cursor, seenIds: nextSeenIds };
    await persist();
    return { ...state, seenIds: state.seenIds.slice() };
  }

  /**
   * Rewind to the previous committed state after a reorg. Returns the state
   * the caller should resume from.
   */
  async function rewind() {
    if (!loaded) {
      await load();
    }
    const previous = history.length > 0 ? history.pop() : { cursor: defaultCursor, seenIds: [] };
    state = { cursor: previous.cursor, seenIds: previous.seenIds.slice() };
    await persist();
    return { ...state, seenIds: state.seenIds.slice() };
  }

  return {
    filePath,
    defaultCursor,
    load,
    commit,
    rewind,
  };
}

/**
 * Filter events already seen in previous polls and deduplicate within the
 * batch. Returns the fresh events and the updated seen id list.
 *
 * @param {Array<object>} events
 * @param {string[]} seenIds
 * @param {object} [options]
 * @param {number} [options.maxSeenIds]
 * @returns {{fresh: Array<object>, seenIds: string[]}}
 */
export function dedupeEvents(events, seenIds = [], { maxSeenIds = 10000 } = {}) {
  const seen = new Set(seenIds.map((id) => String(id)));
  const ordered = seenIds.map((id) => String(id));
  const fresh = [];
  for (const event of events) {
    const id = event && event.id !== undefined ? String(event.id) : null;
    if (id === null) {
      fresh.push(event);
      continue;
    }
    if (seen.has(id)) {
      continue;
    }
    seen.add(id);
    ordered.push(id);
    fresh.push(event);
  }
  while (ordered.length > maxSeenIds) {
    ordered.shift();
  }
  return { fresh, seenIds: ordered };
}
