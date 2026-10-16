import fs from 'node:fs';
import { base64 } from '@scure/base';
import { sha256Hex } from '../hash.js';
import { decodeStellarContractId } from '../ids.js';
import type { Contract } from '../manifest.js';
import { jsonRpc } from './rpc.js';

// XDR enum values from Stellar-ledger-entries.x / Stellar-contract.x.
const LEDGER_ENTRY_CONTRACT_DATA = 6;
const SC_ADDRESS_CONTRACT = 1;
const SCV_LEDGER_KEY_CONTRACT_INSTANCE = 20;
const DURABILITY_PERSISTENT = 1;

const i32be = (n: number) =>
  Uint8Array.of((n >>> 24) & 0xff, (n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff);

/** XDR LedgerKey for a contract's instance entry, base64 encoded for getLedgerEntries. */
export function contractInstanceKey(contractId: string): string {
  return base64.encode(
    Uint8Array.from([
      ...i32be(LEDGER_ENTRY_CONTRACT_DATA),
      ...i32be(SC_ADDRESS_CONTRACT),
      ...decodeStellarContractId(contractId),
      ...i32be(SCV_LEDGER_KEY_CONTRACT_INSTANCE),
      ...i32be(DURABILITY_PERSISTENT),
    ]),
  );
}

/** Wasm hash from a contract instance entry (LedgerEntryData XDR, base64). */
export function wasmHashFromInstanceEntry(xdrBase64: string): string | null {
  const bytes = base64.decode(xdrBase64);
  // type(4) ext(4) SCAddress(4+32) key(4) durability(4) SCVal type(4) executable type(4) hash(32)
  if (bytes.length < 92) return null;
  const executableType = (bytes[56] << 24) | (bytes[57] << 16) | (bytes[58] << 8) | bytes[59];
  if (executableType !== 0) return null;
  return `0x${Buffer.from(bytes.subarray(60, 92)).toString('hex')}`;
}

export async function stellarInstance(rpc: string, contractId: string) {
  const result = await jsonRpc<{
    entries: Array<{ xdr: string; lastModifiedLedgerSeq: number }> | null;
  }>(rpc, 'getLedgerEntries', { keys: [contractInstanceKey(contractId)] });
  return result.entries?.[0] ?? null;
}

export async function stellarNetworkId(rpc: string): Promise<string> {
  return (await jsonRpc<{ passphrase: string }>(rpc, 'getNetwork', {})).passphrase;
}

export interface StellarTarget {
  name: string;
  contractId: string;
  /** Absolute path to the deployed (optimized) WASM. */
  wasmPath: string;
  deployer?: string;
}

/**
 * Height is the ledger in which the instance entry was last modified. Record
 * right after `contract deploy` and before any call that writes instance
 * storage (such as `init`), and it is the deployment ledger.
 */
export async function recordStellarContract(
  rpc: string,
  target: StellarTarget,
): Promise<Omit<Contract, 'version' | 'sourceCommit'>> {
  const entry = await stellarInstance(rpc, target.contractId);
  if (!entry) throw new Error(`${target.name}: no instance entry for ${target.contractId}`);

  const wasmHash = wasmHashFromInstanceEntry(entry.xdr);
  if (!wasmHash) {
    throw new Error(`${target.name}: unable to decode WASM hash from instance entry`);
  }

  const localHash = `0x${sha256Hex(fs.readFileSync(target.wasmPath))}`;
  if (localHash !== wasmHash) {
    throw new Error(
      `${target.name}: WASM hash mismatch (on-chain ${wasmHash}, local ${localHash})`,
    );
  }

  return {
    name: target.name,
    id: target.contractId,
    artifactHash: { algorithm: 'sha256', value: localHash },
    deployment: { height: entry.lastModifiedLedgerSeq },
  };
}
