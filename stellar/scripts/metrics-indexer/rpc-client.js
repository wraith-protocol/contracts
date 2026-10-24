// Bounded JSON-RPC transport for the metrics indexer.
//
// The Stellar SDK's `rpc.Server` posts through a shared Axios instance and
// does not forward an `AbortSignal` per call, so a hung RPC node can pin a
// poll forever. This module talks JSON-RPC directly with `fetch` instead,
// which lets every request carry:
//
//   * an abortable timeout (the socket is actually released, not abandoned),
//   * a bounded retry budget with full-jitter backoff,
//   * a hard cap on how many response bytes we will read, and
//   * errors whose messages are truncated, so a huge provider body can never
//     be replayed into the logs.
//
// Event decoding reuses the SDK's `xdr`/`scValToNative`, so the parsed events
// match what the aggregator already expects (native JS topics and values).

import { xdr, scValToNative } from '@stellar/stellar-sdk';

import { redactUrl, truncate } from './redact.js';

export const DEFAULT_TIMEOUT_MS = 10_000;
export const DEFAULT_MAX_ATTEMPTS = 4;
export const DEFAULT_RETRY_BASE_MS = 250;
export const DEFAULT_RETRY_MAX_MS = 5_000;
export const DEFAULT_MAX_RESPONSE_BYTES = 1_048_576;

// Cursor-persistence helpers: a cursor is only trusted if it matches a strict
// shape, so a corrupted or tampered checkpoint file cannot steer the poller.
const CURSOR_PATTERN = /^[A-Za-z0-9_\-]{1,256}$/;

/**
 * Validate a persisted cursor value. Returns the cursor if well-formed, or
 * null if it is missing or corrupted (caller falls back to a safe default).
 *
 * @param {unknown} value
 * @returns {string|null}
 */
export function validateCursor(value) {
  if (typeof value !== 'string' || !CURSOR_PATTERN.test(value)) {
    return null;
  }
  return value;
}

/**
 * Deduplicate a page of events against a set of already-seen event ids so a
 * replayed page (after restart or rewind) is not indexed twice.
 *
 * @param {Array<{id: string}>} events
 * @param {Set<string>} seenIds
 * @returns {Array<{id: string}>} events not previously seen
 */
export function dedupeEvents(events, seenIds) {
  const fresh = [];
  for (const event of events) {
    if (!seenIds.has(event.id)) {
      seenIds.add(event.id);
      fresh.push(event);
    }
  }
  return fresh;
}

/**
 * Compute the cursor to resume from after a reorg. Rewinds by `depth` ledgers
 * and never returns a cursor newer than the last confirmed one.
 *
 * @param {{ lastConfirmedLedger: number, depth?: number }} state
 * @returns {number} ledger to rewind the poll start to
 */
export function reorgRewindLedger({ lastConfirmedLedger, depth = 12 }) {
  return Math.max(1, lastConfirmedLedger - depth);
}

// JSON-RPC request id. We issue one request at a time, so a constant is enough
// and it lets us verify the provider echoed the id back.
const REQUEST_ID = 1;

/** Base class for transport failures, carrying retry policy metadata. */
export class RpcError extends Error {
  constructor(message, { method, retryable = false, status, code, bodyPreview } = {}) {
    super(message);
    this.name = 'RpcError';
    this.method = method;
    this.retryable = retryable;
    if (status !== undefined) {
      this.status = status;
    }
    if (code !== undefined) {
      this.code = code;
    }
    if (bodyPreview !== undefined) {
      this.bodyPreview = bodyPreview;
    }
  }
}

/** The request exceeded the configured timeout and was aborted. */
export class RpcTimeoutError extends RpcError {
  constructor({ method, timeoutMs }) {
    super(`RPC ${method} timed out after ${timeoutMs}ms`, { method, retryable: true });
    this.name = 'RpcTimeoutError';
    this.timeoutMs = timeoutMs;
  }
}

/** The caller's own AbortSignal fired; retrying would be wrong. */
export class RpcAbortError extends RpcError {
  constructor({ method }) {
    super(`RPC ${method} aborted`, { method, retryable: false });
    this.name = 'RpcAbortError';
  }
}

/** The RPC endpoint returned a non-2xx HTTP status. */
export class RpcHttpError extends RpcError {
  constructor({ method, status, retryable, bodyPreview }) {
    super(`RPC ${method} failed with HTTP ${status}`, { method, retryable, status, bodyPreview });
    this.name = 'RpcHttpError';
  }
}

/** The provider tried to return more bytes than we are willing to read. */
export class RpcResponseTooLargeError extends RpcError {
  constructor({ method, maxResponseBytes }) {
    super(`RPC ${method} response exceeded ${maxResponseBytes} bytes`, { method, retryable: false });
    this.name = 'RpcResponseTooLargeError';
    this.maxResponseBytes = maxResponseBytes;
  }
}

/**
 * The provider returned a 2xx body that is not a well-formed JSON-RPC response
 * for the request we sent. Treated as fatal: a malformed envelope or result
 * must never be mistaken for an empty (successful) result.
 */
export class RpcProtocolError extends RpcError {
  constructor({ method, reason, bodyPreview }) {
    super(`RPC ${method} returned a malformed JSON-RPC response: ${reason}`, {
      method,
      retryable: false,
      bodyPreview,
    });
    this.name = 'RpcProtocolError';
    this.reason = reason;
  }
}

// Read at most `maxResponseBytes` from a fetch Response, then release the
// socket. Without this a misbehaving provider could stream unbounded data into
// memory before we ever get to parse it.
async function readTextCapped(response, maxResponseBytes, method) {
  const { body } = response;

  if (!body || typeof body.getReader !== 'function') {
    const text = await response.text();
    if (Buffer.byteLength(text, 'utf8') > maxResponseBytes) {
      throw new RpcResponseTooLargeError({ method, maxResponseBytes });
    }
    return text;
  }

  const reader = body.getReader();
  const chunks = [];
  let size = 0;

  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) {
        break;
      }
      size += value.byteLength;
      if (size > maxResponseBytes) {
        throw new RpcResponseTooLargeError({ method, maxResponseBytes });
      }
      chunks.push(value);
    }
  } finally {
    // Cancels the stream on the cap/abort path and is a no-op once drained.
    await reader.cancel().catch(() => {});
  }

  return Buffer.concat(chunks.map((chunk) => Buffer.from(chunk))).toString('utf8');
}

// Abortable sleep so a retry wait or the poller's idle gap can be interrupted
// promptly on shutdown.
function defaultSleep(ms, signal) {
  if (!signal) {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }
  if (signal.aborted) {
    return Promise.reject(new RpcAbortError({ method: 'sleep' }));
  }
  return new Promise((resolve, reject) => {
    const onAbort = () => {
      clearTimeout(timer);
      reject(new RpcAbortError({ method: 'sleep' }));
    };
    const timer = setTimeout(() => {
      signal.removeEventListener('abort', onAbort);
      resolve();
    }, ms);
    signal.addEventListener('abort', onAbort, { once: true });
  });
}

function buildGetEventsParams(request = {}) {
  const params = { filters: request.filters ?? [] };
  const pagination = {};
  if (request.cursor) {
    pagination.cursor = request.cursor;
  }
  if (request.limit) {
    pagination.limit = request.limit;
  }
  if (Object.keys(pagination).length > 0) {
    params.pagination = pagination;
  }
  if (request.startLedger) {
    params.startLedger = request.startLedger;
  }
  return params;
}

function decodeScVal(encoded, method = 'getEvents') {
  if (typeof encoded !== 'string') {
    throw new RpcProtocolError({ method, reason: 'event ScVal was not a base64 string' });
  }
  try {
    return scValToNative(xdr.ScVal.fromXDR(encoded, 'base64'));
  } catch (error) {
    throw new RpcProtocolError({
      method,
      reason: `event ScVal could not be decoded: ${truncate(error?.message ?? error, 120)}`,
    });
  }
}

// A well-formed JSON-RPC 2.0 response echoes our id and carries exactly one of
// `result`/`error`. A 200 body that omits both (for example
// `{"jsonrpc":"2.0","id":1}`) is a provider bug: without this check it would be
// parsed as a successful, empty result and the poll would report success while
// silently indexing nothing.
function validateEnvelope(payload, method) {
  if (payload === null || typeof payload !== 'object' || Array.isArray(payload)) {
    throw new RpcProtocolError({ method, reason: 'response body was not a JSON object' });
  }
  if (payload.jsonrpc !== '2.0') {
    throw new RpcProtocolError({ method, reason: 'response omitted jsonrpc "2.0"' });
  }
  if (!('id' in payload) || String(payload.id) !== String(REQUEST_ID)) {
    throw new RpcProtocolError({ method, reason: 'response id did not match the request id' });
  }
  const hasResult = Object.prototype.hasOwnProperty.call(payload, 'result');
  const hasError = Object.prototype.hasOwnProperty.call(payload, 'error');
  if (hasResult && hasError) {
    throw new RpcProtocolError({ method, reason: 'response contained both result and error' });
  }
  if (!hasResult && !hasError) {
    throw new RpcProtocolError({ method, reason: 'response contained neither result nor error' });
  }
}

// Mirror `rpc.Server.getEvents` but convert topics/values to native JS, which
// is what `parseMetricEvent` consumes. The `getEvents` result shape is
// validated, not defaulted: a missing or invalid `events` array must fail the
// poll instead of being reported as an empty (successful) page.
function parseEvents(raw) {
  const method = 'getEvents';
  if (raw === null || typeof raw !== 'object' || Array.isArray(raw)) {
    throw new RpcProtocolError({ method, reason: 'getEvents result was not an object' });
  }
  if (!Number.isFinite(raw.latestLedger)) {
    throw new RpcProtocolError({
      method,
      reason: 'getEvents result omitted a numeric latestLedger',
    });
  }
  if (!Array.isArray(raw.events)) {
    throw new RpcProtocolError({ method, reason: 'getEvents result.events was not an array' });
  }

  return {
    ...raw,
    events: raw.events.map((event) => {
      if (event === null || typeof event !== 'object' || Array.isArray(event)) {
        throw new RpcProtocolError({ method, reason: 'getEvents event entry was not an object' });
      }
      if (!Array.isArray(event.topic)) {
        throw new RpcProtocolError({ method, reason: 'getEvents event topic was not an array' });
      }
      return {
        ...event,
        topic: event.topic.map((encoded) => decodeScVal(encoded, method)),
        value: event.value === undefined ? undefined : decodeScVal(event.value, method),
      };
    }),
  };
}

/**
 * Create a bounded RPC client.
 *
 * @param {object} [options]
 * @param {string} options.url JSON-RPC endpoint.
 * @param {number} [options.timeoutMs] Per-attempt timeout; the request is aborted.
 * @param {number} [options.maxAttempts] Total attempts, including the first.
 * @param {number} [options.retryBaseMs] Base delay for jittered backoff.
 * @param {number} [options.retryMaxMs] Ceiling for the jittered delay.
 * @param {number} [options.maxResponseBytes] Hard cap on response bytes read.
 * @param {typeof fetch} [options.fetchImpl] Injectable transport (tests).
 * @param {(ms: number, signal?: AbortSignal) => Promise<void>} [options.sleep]
 * @param {() => number} [options.random] Injectable RNG (tests).
 * @param {(info: object) => void} [options.onRetry] Retry observer.
 */
export function createRpcClient(options = {}) {
  const {
    url,
    timeoutMs = DEFAULT_TIMEOUT_MS,
    maxAttempts = DEFAULT_MAX_ATTEMPTS,
    retryBaseMs = DEFAULT_RETRY_BASE_MS,
    retryMaxMs = DEFAULT_RETRY_MAX_MS,
    maxResponseBytes = DEFAULT_MAX_RESPONSE_BYTES,
    fetchImpl = globalThis.fetch,
    sleep = defaultSleep,
    random = Math.random,
    onRetry,
  } = options;

  if (typeof url !== 'string' || url.length === 0) {
    throw new TypeError('createRpcClient requires a url');
  }
  if (typeof fetchImpl !== 'function') {
    throw new TypeError('createRpcClient requires a fetch implementation');
  }

  const redactedUrl = options.redactedUrl ?? redactUrl(url);

  // Full-jitter backoff: uniform over [0, min(retryMaxMs, retryBaseMs * 2^attempt)].
  function backoffDelay(attempt) {
    const ceiling = Math.min(retryMaxMs, retryBaseMs * 2 ** attempt);
    return Math.floor(random() * ceiling);
  }

  return {
    redactedUrl,
    backoffDelay,
    parseEvents,
    buildGetEventsParams,
    validateEnvelope,
    readTextCapped,
    defaults: { timeoutMs, maxAttempts, maxResponseBytes },
    fetchImpl,
    sleep,
    onRetry,
  };
}
