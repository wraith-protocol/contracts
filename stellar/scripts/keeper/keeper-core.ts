/**
 * Wraith Names TTL keeper - core logic.
 *
 * Everything that touches the network goes through the `KeeperRpc` interface so
 * the flow can be tested deterministically with a fake RPC (see keeper.test.ts).
 *
 * The keeper fails closed: if names cannot be enumerated, a TTL cannot be read,
 * or an extend transaction cannot be submitted, `runKeeper` throws a
 * `KeeperError` and the CLI exits nonzero. It never reports success for work it
 * did not do.
 */

import { createHash } from 'crypto';
import * as fs from 'fs';

export class KeeperError extends Error {
  constructor(
    public readonly stage: 'config' | 'enumeration' | 'ttl-read' | 'submission',
    message: string,
  ) {
    super(message);
    this.name = 'KeeperError';
  }
}

/** A contract event, already decoded from XDR to plain values. */
export interface DecodedEvent {
  ledger: number;
  kind: 'register' | 'update' | 'release';
  name: string;
}

export interface EventPage {
  events: DecodedEvent[];
  /** Opaque cursor for the next page, or undefined when exhausted. */
  cursor?: string;
  /** Oldest ledger the RPC still serves events for. */
  oldestLedger: number;
  latestLedger: number;
}

export interface KeeperRpc {
  getLatestLedger(): Promise<number>;
  /** Page through register/update/release events from `startLedger`. */
  getNameEvents(startLedger: number, cursor?: string): Promise<EventPage>;
  /** liveUntilLedgerSeq of the persistent Name(hash) entry, or null if no live entry. */
  getNameLiveUntil(nameHash: string): Promise<number | null>;
  /**
   * Submit extend_name_ttl(name, extendTo) and wait for it to land. Returns the tx hash.
   * `extendTo` is a TTL in ledgers counted from the current ledger (not a ledger sequence).
   */
  extendName(name: string, extendTo: number): Promise<string>;
}

export interface KeeperOptions {
  /** Extend names whose remaining TTL (ledgers) is below this. */
  threshold: number;
  /** TTL, in ledgers from now, to extend to (relative, not an absolute ledger). */
  extendTo: number;
  dryRun: boolean;
  /** First ledger to read events from (e.g. the contract deployment ledger). */
  startLedger?: number;
  /** Names from an off-chain index (e.g. the indexer), merged with events. */
  indexedNames?: string[];
}

export interface NameStatus {
  name: string;
  liveUntil: number;
  remaining: number;
  atRisk: boolean;
}

export interface KeeperReport {
  latestLedger: number;
  total: number;
  atRisk: number;
  extended: { name: string; txHash: string }[];
  dryRun: boolean;
}

export function nameHash(name: string): string {
  return createHash('sha256').update(Buffer.from(name, 'utf8')).digest('hex');
}

export function validateOptions(o: KeeperOptions): void {
  if (!Number.isInteger(o.threshold) || o.threshold <= 0) {
    throw new KeeperError('config', `threshold must be a positive integer, got ${o.threshold}`);
  }
  if (!Number.isInteger(o.extendTo) || o.extendTo <= o.threshold) {
    throw new KeeperError(
      'config',
      `extend-to (${o.extendTo}) must be an integer greater than threshold (${o.threshold})`,
    );
  }
  if (o.startLedger === undefined && !(o.indexedNames && o.indexedNames.length > 0)) {
    throw new KeeperError(
      'config',
      'no name source configured: pass --start-ledger (event scan) and/or --index-file',
    );
  }
}

/**
 * Build the set of live names. Events are replayed in order (register/update add,
 * release removes); indexed names are merged in afterwards. If the event scan was
 * requested but the RPC no longer holds events back to `startLedger`, the history
 * is incomplete and we refuse to continue rather than silently skip names.
 */
export async function enumerateNames(rpc: KeeperRpc, o: KeeperOptions): Promise<string[]> {
  const names = new Set<string>();

  if (o.startLedger !== undefined) {
    let cursor: string | undefined;
    let first = true;
    try {
      do {
        const page = await rpc.getNameEvents(o.startLedger, cursor);
        if (first && page.oldestLedger > o.startLedger) {
          throw new KeeperError(
            'enumeration',
            `RPC event history starts at ledger ${page.oldestLedger}, after --start-ledger ${o.startLedger}; ` +
              'names registered earlier cannot be discovered from events. Supply --index-file or a longer-retention RPC.',
          );
        }
        first = false;
        for (const ev of page.events) {
          if (ev.kind === 'release') names.delete(ev.name);
          else names.add(ev.name);
        }
        cursor = page.cursor;
      } while (cursor);
    } catch (e) {
      if (e instanceof KeeperError) throw e;
      throw new KeeperError('enumeration', `event enumeration failed: ${(e as Error).message}`);
    }
  }

  for (const n of o.indexedNames ?? []) names.add(n);
  return [...names].sort();
}

export async function readStatuses(
  rpc: KeeperRpc,
  names: string[],
  latestLedger: number,
  threshold: number,
): Promise<NameStatus[]> {
  const out: NameStatus[] = [];
  const unreadable: string[] = [];
  for (const name of names) {
    let liveUntil: number | null;
    try {
      liveUntil = await rpc.getNameLiveUntil(nameHash(name));
    } catch (e) {
      throw new KeeperError('ttl-read', `TTL read failed for "${name}": ${(e as Error).message}`);
    }
    if (liveUntil === null) {
      unreadable.push(name);
      continue;
    }
    const remaining = liveUntil - latestLedger;
    out.push({ name, liveUntil, remaining, atRisk: remaining < threshold });
  }
  if (unreadable.length > 0) {
    throw new KeeperError(
      'ttl-read',
      `no live ledger entry for ${unreadable.length} name(s): ${unreadable.join(', ')} ` +
        '(released, expired or archived - restore or remove from the index before re-running)',
    );
  }
  return out;
}

export async function runKeeper(
  rpc: KeeperRpc,
  o: KeeperOptions,
  log: (msg: string) => void = () => {},
): Promise<KeeperReport> {
  validateOptions(o);

  let latestLedger: number;
  try {
    latestLedger = await rpc.getLatestLedger();
  } catch (e) {
    throw new KeeperError('enumeration', `could not read latest ledger: ${(e as Error).message}`);
  }
  log(`latest ledger ${latestLedger}`);

  const names = await enumerateNames(rpc, o);
  log(`found ${names.length} registered name(s)`);

  const statuses = await readStatuses(rpc, names, latestLedger, o.threshold);
  const risky = statuses.filter((s) => s.atRisk);
  log(`${risky.length}/${statuses.length} below threshold ${o.threshold}`);

  const report: KeeperReport = {
    latestLedger,
    total: statuses.length,
    atRisk: risky.length,
    extended: [],
    dryRun: o.dryRun,
  };

  // The contract takes a relative TTL, so pass extendTo as is. Adding latestLedger here
  // would count the current ledger twice and extend far past the requested window.
  const liveUntilAfter = latestLedger + o.extendTo;
  for (const s of risky) {
    if (o.dryRun) {
      log(
        `[dry-run] would extend "${s.name}" (remaining ${s.remaining}) by ${o.extendTo} ledgers, live until ~ledger ${liveUntilAfter}`,
      );
      continue;
    }
    try {
      const txHash = await rpc.extendName(s.name, o.extendTo);
      report.extended.push({ name: s.name, txHash });
      log(`extended "${s.name}" by ${o.extendTo} ledgers, live until ~ledger ${liveUntilAfter} (tx ${txHash})`);
    } catch (e) {
      throw new KeeperError(
        'submission',
        `extend_name_ttl failed for "${s.name}" after ${report.extended.length} successful extension(s): ${(e as Error).message}`,
      );
    }
  }
  return report;
}

export function loadIndexFile(path: string): string[] {
  let parsed: unknown;
  try {
    parsed = JSON.parse(fs.readFileSync(path, 'utf8'));
  } catch (e) {
    throw new KeeperError('enumeration', `cannot read index file ${path}: ${(e as Error).message}`);
  }
  if (!Array.isArray(parsed) || !parsed.every((n) => typeof n === 'string')) {
    throw new KeeperError('enumeration', `index file ${path} must be a JSON array of name strings`);
  }
  return parsed;
}
