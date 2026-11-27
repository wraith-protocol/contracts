// Durable, reorg-safe ingestion state for the metrics indexer.
//
// The indexer's metric totals and its per-contract RPC cursors live in ONE
// checkpoint document that is replaced atomically (write temp file, fsync,
// rename). Cursor and aggregates can therefore never advance separately: after
// a crash the indexer resumes from the last committed cursor with exactly the
// totals that cursor accounts for.
//
// `planPage` decides, for one `getEvents` page, which events are new and what
// the next per-contract state is. It is pure so every replay, rollback, and gap
// scenario can be tested without a network.

import { closeSync, fsyncSync, openSync, readFileSync, renameSync, writeSync } from 'node:fs';
import { basename, dirname, join } from 'node:path';

export const CHECKPOINT_VERSION = 1;

/** Raised when the persisted checkpoint cannot be trusted. Never auto-repaired. */
export class CursorStateCorruptError extends Error {
  constructor(message) {
    super(message);
    this.name = 'CursorStateCorruptError';
  }
}

/** A page that would make the indexer skip or re-count events. Nothing from it is applied. */
export class CursorIntegrityError extends Error {
  constructor(kind, message) {
    super(message);
    this.name = 'CursorIntegrityError';
    this.kind = kind;
  }
}

// Stellar RPC event ids are `<toid>-<index>`; a bare `<toid>` is index 0.
const EVENT_ID_RE = /^(\d+)(?:-(\d+))?$/;

function parseEventId(id) {
  const match = typeof id === 'string' ? EVENT_ID_RE.exec(id) : null;
  if (!match) {
    return null;
  }
  return [BigInt(match[1]), BigInt(match[2] ?? 0)];
}

/** Order two Stellar event ids (`<toid>-<index>`). */
export function compareEventIds(a, b) {
  const [aToid, aIndex] = parseEventId(a);
  const [bToid, bIndex] = parseEventId(b);
  if (aToid !== bToid) {
    return aToid < bToid ? -1 : 1;
  }
  if (aIndex !== bIndex) {
    return aIndex < bIndex ? -1 : 1;
  }
  return 0;
}

export function emptyContractState() {
  return { cursor: null, scannedLedger: null, lastEventId: null };
}

export function emptyCheckpoint() {
  return { version: CHECKPOINT_VERSION, contracts: {}, series: [] };
}

const isNullableString = (v) => v === null || (typeof v === 'string' && v.length > 0);
const isNullableLedger = (v) => v === null || (Number.isSafeInteger(v) && v >= 0);

/**
 * Validate a parsed checkpoint. Anything unexpected is corruption: silently
 * dropping a field would either replay history (double count) or forget it.
 */
export function validateCheckpoint(doc, { knownMetrics } = {}) {
  if (doc === null || typeof doc !== 'object' || Array.isArray(doc)) {
    throw new CursorStateCorruptError('checkpoint is not a JSON object');
  }
  if (doc.version !== CHECKPOINT_VERSION) {
    throw new CursorStateCorruptError(`unsupported checkpoint version ${JSON.stringify(doc.version)}`);
  }
  if (doc.contracts === null || typeof doc.contracts !== 'object' || Array.isArray(doc.contracts)) {
    throw new CursorStateCorruptError('checkpoint.contracts is not an object');
  }
  for (const [address, state] of Object.entries(doc.contracts)) {
    if (
      state === null ||
      typeof state !== 'object' ||
      !isNullableString(state.cursor) ||
      !isNullableLedger(state.scannedLedger) ||
      !(state.lastEventId === null || parseEventId(state.lastEventId))
    ) {
      throw new CursorStateCorruptError(`checkpoint state for contract ${address.slice(0, 8)}… is invalid`);
    }
  }
  if (!Array.isArray(doc.series)) {
    throw new CursorStateCorruptError('checkpoint.series is not an array');
  }
  for (const entry of doc.series) {
    if (
      entry === null ||
      typeof entry !== 'object' ||
      typeof entry.metric !== 'string' ||
      typeof entry.key !== 'string' ||
      entry.labels === null ||
      typeof entry.labels !== 'object' ||
      !Number.isFinite(entry.value) ||
      (knownMetrics && !knownMetrics.has(entry.metric))
    ) {
      throw new CursorStateCorruptError('checkpoint.series has an invalid entry');
    }
  }
  return doc;
}

/**
 * File-backed checkpoint store.
 *
 * `load()` returns `null` when no checkpoint exists yet (first start) and
 * throws `CursorStateCorruptError` when one exists but cannot be trusted. The
 * corrupt file is left in place for the operator; it is never overwritten.
 */
export function createFileCheckpointStore({ path, knownMetrics } = {}) {
  if (typeof path !== 'string' || path.length === 0) {
    throw new TypeError('createFileCheckpointStore requires a path');
  }

  function load() {
    let text;
    try {
      text = readFileSync(path, 'utf8');
    } catch (error) {
      if (error?.code === 'ENOENT') {
        return null;
      }
      throw new CursorStateCorruptError(`checkpoint ${basename(path)} is unreadable (${error?.code ?? 'error'})`);
    }
    let doc;
    try {
      doc = JSON.parse(text);
    } catch {
      throw new CursorStateCorruptError(`checkpoint ${basename(path)} is not valid JSON`);
    }
    return validateCheckpoint(doc, { knownMetrics });
  }

  function save(checkpoint) {
    const tmp = join(dirname(path), `.${basename(path)}.${process.pid}.tmp`);
    const fd = openSync(tmp, 'w', 0o600);
    try {
      writeSync(fd, `${JSON.stringify(checkpoint)}\n`);
      fsyncSync(fd);
    } finally {
      closeSync(fd);
    }
    renameSync(tmp, path);
  }

  return { path, load, save };
}

/**
 * Decide what to do with one `getEvents` page for one contract.
 *
 * Policy:
 *   - Events at or below the committed `lastEventId` are replays and are
 *     dropped (restart replay, duplicate page, provider serving an older page).
 *   - A provider may lag behind the committed position by at most
 *     `maxRewindLedgers`; within that bound its page is deduplicated and the
 *     committed position never moves backward. Beyond it the page is refused.
 *   - If the provider no longer retains the ledger after our committed scan
 *     position, events may have been pruned: the page is refused instead of
 *     silently skipping them.
 *   - Events inside a page must be strictly ordered and carry an id; otherwise
 *     the page cannot be deduplicated safely and is refused.
 *
 * @returns {{ events: object[], next: object, duplicates: number, advanced: boolean }}
 */
export function planPage({ state = emptyContractState(), response, limit, maxRewindLedgers }) {
  const prev = { ...emptyContractState(), ...state };
  const latest = response.latestLedger;

  if (prev.scannedLedger !== null && latest < prev.scannedLedger - maxRewindLedgers) {
    throw new CursorIntegrityError(
      'rollback',
      `provider latest ledger ${latest} is more than ${maxRewindLedgers} ledgers behind committed ledger ${prev.scannedLedger}`
    );
  }
  if (
    prev.scannedLedger !== null &&
    Number.isFinite(response.oldestLedger) &&
    response.oldestLedger > prev.scannedLedger + 1
  ) {
    throw new CursorIntegrityError(
      'retention_gap',
      `provider retention starts at ledger ${response.oldestLedger}, after committed ledger ${prev.scannedLedger}; events in between may be lost`
    );
  }

  const fresh = [];
  let duplicates = 0;
  let previousId = null;
  for (const event of response.events) {
    if (!parseEventId(event.id)) {
      throw new CursorIntegrityError('protocol', 'getEvents event has no valid id; cannot deduplicate');
    }
    if (previousId !== null && compareEventIds(event.id, previousId) <= 0) {
      throw new CursorIntegrityError('protocol', 'getEvents page is not strictly ordered by event id');
    }
    previousId = event.id;
    if (prev.lastEventId !== null && compareEventIds(event.id, prev.lastEventId) <= 0) {
      duplicates += 1;
      continue;
    }
    fresh.push(event);
  }

  const behind = prev.scannedLedger !== null && latest < prev.scannedLedger;
  const last = response.events.at(-1);
  const pageFull = response.events.length >= limit;
  const pageScanned = pageFull ? (Number.isSafeInteger(last?.ledger) ? last.ledger : prev.scannedLedger) : latest;
  const pageCursor =
    (typeof response.cursor === 'string' && response.cursor) || last?.pagingToken || last?.id || null;

  const next = {
    // A lagging provider's cursor would rewind us; keep ours.
    cursor: behind ? prev.cursor : pageCursor ?? prev.cursor,
    scannedLedger:
      prev.scannedLedger === null ? pageScanned ?? null : Math.max(prev.scannedLedger, pageScanned ?? prev.scannedLedger),
    lastEventId: fresh.length ? fresh.at(-1).id : prev.lastEventId,
  };
  const advanced =
    fresh.length > 0 || next.cursor !== prev.cursor || next.scannedLedger !== prev.scannedLedger;
  return { events: fresh, next, duplicates, advanced };
}
