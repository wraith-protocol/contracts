#!/usr/bin/env -S npx tsx
/**
 * Wraith Protocol — Rescue Stealth Funds
 * ========================================
 *
 * Recovery tool for funds that landed at a stealth address without a matching
 * on-chain announcement. It recomputes the stealth address from the sender's
 * ephemeral key and the recipient's meta-address, checks the real balance on
 * Horizon, and publishes the missing announcement through a real Soroban
 * transaction so the recipient's scanner can find (and then sweep) the payment.
 *
 * The original payment is final. This tool only restores findability.
 * It never asks for a spending key: only the ephemeral key, plus a fee-paying
 * account secret supplied through the RESCUE_SOURCE_SECRET environment variable.
 *
 * Usage:
 *   RESCUE_SOURCE_SECRET=S... npx tsx rescue-stealth-funds.ts \
 *     --ephemeral-key <32-byte hex> \
 *     --recipient-meta-address <64-byte hex> \
 *     --amount <number> --asset XLM \
 *     --announcer <contract id> \
 *     [--rpc <soroban rpc url>] [--horizon <horizon url>] \
 *     [--network-passphrase <passphrase>] [--scheme-id 2] --yes
 */

import { Command } from 'commander';
import { createHash } from 'crypto';
import {
  Address,
  BASE_FEE,
  Contract,
  Keypair,
  Networks,
  TransactionBuilder,
  nativeToScVal,
  rpc,
  xdr,
} from '@stellar/stellar-sdk';
import { publicKeyHex, scalarFromHex, sharedPointHex, stealthFromShared } from './stealth-derivation';

// ─── Types ───────────────────────────────────────────────────────────────────

export interface RescueInputs {
  /** Hex-encoded ephemeral private key (32 bytes) */
  ephemeralKey: string;
  /** Hex-encoded recipient meta-address (64 bytes: spending_pubkey || viewing_pubkey) */
  recipientMetaAddress: string;
  /** Amount that was sent */
  amount: string;
  /** "XLM" or "CODE:ISSUER" */
  asset: string;
  /** Contract ID of the deployed StealthAnnouncer */
  announcerId: string;
  /** Soroban RPC URL */
  rpc: string;
  /** Horizon URL (balance queries) */
  horizon: string;
  /** Network passphrase */
  networkPassphrase: string;
  /** Fee-paying account secret (S...). Never a spending key. */
  sourceSecret?: string;
  /** Announcer scheme id (2 = Stellar v2 announcer) */
  schemeId?: number;
}

export interface AnnouncementPayload {
  schemeId: number;
  stealthAddress: string;
  /** 32-byte hex */
  ephemeralPubKey: string;
  /** hex, no 0x prefix: one view-tag byte */
  metadata: string;
}

export const DEFAULT_SCHEME_ID = 2;
export const TESTNET_HORIZON = 'https://horizon-testnet.stellar.org';
export const TESTNET_RPC = 'https://soroban-testnet.stellar.org';

// ─── Stealth address derivation ─────────────────────────────────────────────

/** Parse a 64-byte meta-address into spending and viewing public keys. */
export function parseMetaAddress(metaAddressHex: string): {
  spendingPubKey: string;
  viewingPubKey: string;
} {
  const buf = Buffer.from(metaAddressHex, 'hex');
  if (buf.length !== 64) {
    throw new Error(`Stealth meta-address must be exactly 64 bytes, got ${buf.length}`);
  }
  return {
    spendingPubKey: buf.subarray(0, 32).toString('hex'),
    viewingPubKey: buf.subarray(32, 64).toString('hex'),
  };
}

/**
 * Recompute the real stealth account (a valid G... address) from the sender's
 * ephemeral private key and the recipient's meta-address.
 */
export function recomputeStealthAddress(
  ephemeralKeyHex: string,
  metaAddressHex: string,
): { stealthAddress: string; ephemeralPubKey: string; sharedPoint: string } {
  const { spendingPubKey, viewingPubKey } = parseMetaAddress(metaAddressHex);
  const scalar = scalarFromHex(ephemeralKeyHex);
  const shared = sharedPointHex(scalar, viewingPubKey);
  const { address } = stealthFromShared(spendingPubKey, shared);
  return { stealthAddress: address, ephemeralPubKey: publicKeyHex(scalar), sharedPoint: shared };
}

function viewTagHex(sharedPoint: string): string {
  return createHash('sha256').update(Buffer.from(sharedPoint, 'hex')).digest().subarray(0, 1).toString('hex');
}

// ─── Balance queries (Horizon) ──────────────────────────────────────────────

/** Balance of a G... account for an asset, or null if the account does not exist. */
export async function queryBalance(
  address: string,
  asset: string,
  horizonUrl: string,
): Promise<string | null> {
  try {
    if (!address.startsWith('G')) return null;
    const response = await fetch(`${horizonUrl.replace(/\/$/, '')}/accounts/${address}`);
    if (!response.ok) return null;
    const data = (await response.json()) as { balances?: Array<Record<string, string>> };
    if (!data.balances) return null;

    if (asset === 'XLM' || asset === 'native') {
      const b = data.balances.find((x) => x.asset_type === 'native');
      return b ? b.balance : '0';
    }
    const [code, issuer] = asset.split(':');
    const b = data.balances.find(
      (x) => x.asset_code === code && x.asset_issuer?.toUpperCase() === issuer?.toUpperCase(),
    );
    return b ? b.balance : '0';
  } catch {
    return null;
  }
}

/** True when the balance is at least the expected amount (7-decimal precision). */
export function balanceMatches(balance: string | null, expectedAmount: string): boolean {
  if (balance === null) return false;
  const bal = BigInt(Math.floor(parseFloat(balance) * 10_000_000));
  const exp = BigInt(Math.floor(parseFloat(expectedAmount) * 10_000_000));
  return bal >= exp;
}

/**
 * True when there is nothing left to rescue: the account no longer exists
 * (never funded, or swept and merged) or holds under 10% of the expected amount.
 */
export async function hasFundsBeenMoved(
  address: string,
  expectedAmount: string,
  horizonUrl: string,
  asset = 'native',
): Promise<boolean> {
  const currentBalance = await queryBalance(address, asset, horizonUrl);
  if (currentBalance === null) return true;
  const expected = parseFloat(expectedAmount);
  return expected > 0 && parseFloat(currentBalance) < expected * 0.1;
}

// ─── Announcement ───────────────────────────────────────────────────────────

/** Build the announce payload the announcer contract expects. */
export function buildAnnouncementPayload(
  inputs: RescueInputs,
  stealthAddress: string,
): AnnouncementPayload {
  const { viewingPubKey } = parseMetaAddress(inputs.recipientMetaAddress);
  const scalar = scalarFromHex(inputs.ephemeralKey);
  const shared = sharedPointHex(scalar, viewingPubKey);
  return {
    schemeId: inputs.schemeId ?? DEFAULT_SCHEME_ID,
    stealthAddress,
    ephemeralPubKey: publicKeyHex(scalar),
    metadata: viewTagHex(shared),
  };
}

/**
 * Submit a real Soroban `announce` transaction and wait for it to succeed.
 * Returns the transaction hash.
 */
export async function broadcastAnnouncement(
  payload: AnnouncementPayload,
  announcerId: string,
  rpcUrl: string,
  networkPassphrase: string,
  sourceSecret: string,
): Promise<string> {
  const server = new rpc.Server(rpcUrl);
  const source = Keypair.fromSecret(sourceSecret);
  const account = await server.getAccount(source.publicKey());

  const tx = new TransactionBuilder(account, { fee: BASE_FEE, networkPassphrase })
    .addOperation(
      new Contract(announcerId).call(
        'announce',
        nativeToScVal(payload.schemeId, { type: 'u32' }),
        new Address(payload.stealthAddress).toScVal(),
        xdr.ScVal.scvBytes(Buffer.from(payload.ephemeralPubKey, 'hex')),
        xdr.ScVal.scvBytes(Buffer.from(payload.metadata, 'hex')),
      ),
    )
    .setTimeout(60)
    .build();

  const prepared = await server.prepareTransaction(tx);
  prepared.sign(source);
  const sent = await server.sendTransaction(prepared);
  if (sent.status === 'ERROR') {
    throw new Error(`announce submission failed: ${JSON.stringify(sent.errorResult)}`);
  }

  let result = await server.getTransaction(sent.hash);
  for (let i = 0; i < 30 && result.status === rpc.Api.GetTransactionStatus.NOT_FOUND; i++) {
    await new Promise((r) => setTimeout(r, 1000));
    result = await server.getTransaction(sent.hash);
  }
  if (result.status !== rpc.Api.GetTransactionStatus.SUCCESS) {
    throw new Error(`announce transaction ${sent.hash} did not succeed (status: ${result.status})`);
  }
  return sent.hash;
}

// ─── Orchestration ──────────────────────────────────────────────────────────

export interface RescueResult {
  stealthAddress: string;
  ephemeralPubKey: string;
  balance: string;
  /** undefined on a dry run */
  txHash?: string;
}

/**
 * Full rescue flow: derive, check the real balance, refuse if nothing is left
 * to rescue, then publish the announcement on-chain (unless dryRun).
 */
export async function runRescue(
  inputs: RescueInputs,
  opts: { dryRun?: boolean; log?: (msg: string) => void } = {},
): Promise<RescueResult> {
  const log = opts.log ?? console.log;

  const { stealthAddress, ephemeralPubKey } = recomputeStealthAddress(
    inputs.ephemeralKey,
    inputs.recipientMetaAddress,
  );
  log(`Computed stealth address: ${stealthAddress}`);

  const balance = await queryBalance(stealthAddress, inputs.asset, inputs.horizon);
  if (balance === null) {
    throw new Error(
      `No account found at ${stealthAddress}. It was never funded or has already been swept: nothing to rescue.`,
    );
  }
  log(`Balance: ${balance} ${inputs.asset} (expected ${inputs.amount})`);

  if (await hasFundsBeenMoved(stealthAddress, inputs.amount, inputs.horizon, inputs.asset)) {
    throw new Error('Funds appear to have been moved from this address; the rescue would be a no-op.');
  }
  if (!balanceMatches(balance, inputs.amount)) {
    log('Warning: balance is below the expected amount; continuing because funds are still present.');
  }

  const payload = buildAnnouncementPayload(inputs, stealthAddress);
  log(`Announcement: scheme ${payload.schemeId}, ephemeral ${payload.ephemeralPubKey}, metadata ${payload.metadata}`);

  if (opts.dryRun) return { stealthAddress, ephemeralPubKey, balance };

  if (!inputs.sourceSecret) {
    throw new Error('A fee-paying account secret is required (set RESCUE_SOURCE_SECRET).');
  }
  const txHash = await broadcastAnnouncement(
    payload,
    inputs.announcerId,
    inputs.rpc,
    inputs.networkPassphrase,
    inputs.sourceSecret,
  );
  log(`Announcement transaction: ${txHash}`);
  return { stealthAddress, ephemeralPubKey, balance, txHash };
}

// ─── CLI ─────────────────────────────────────────────────────────────────────

async function main(): Promise<void> {
  const program = new Command();

  program
    .name('rescue-stealth-funds')
    .description(
      'Publish the missing on-chain announcement for funds sitting at a stealth address,\n' +
        'so the recipient can find and sweep them. The original payment is final.',
    )
    .requiredOption('--ephemeral-key <hex>', 'Ephemeral private key (32 bytes hex). NEVER a long-term spending key.')
    .requiredOption('--recipient-meta-address <hex>', 'Recipient meta-address (64 bytes hex: spending_pub || viewing_pub).')
    .requiredOption('--amount <string>', 'Amount that was sent to the stealth address.')
    .requiredOption('--asset <string>', '"XLM" or "CODE:ISSUER".')
    .requiredOption('--announcer <string>', 'Contract ID (C...) of the deployed StealthAnnouncer.')
    .option('--rpc <url>', 'Soroban RPC URL.', TESTNET_RPC)
    .option('--horizon <url>', 'Horizon URL (balance queries).', TESTNET_HORIZON)
    .option('--network-passphrase <string>', 'Network passphrase.', Networks.TESTNET)
    .option('--scheme-id <number>', 'Announcer scheme id.', String(DEFAULT_SCHEME_ID))
    .option('--yes', 'Broadcast the announcement (without it the tool only shows what it would do).');

  program.parse(process.argv);
  const opts = program.opts();

  const inputs: RescueInputs = {
    ephemeralKey: opts.ephemeralKey,
    recipientMetaAddress: opts.recipientMetaAddress,
    amount: opts.amount,
    asset: opts.asset,
    announcerId: opts.announcer,
    rpc: opts.rpc,
    horizon: opts.horizon,
    networkPassphrase: opts.networkPassphrase,
    schemeId: Number(opts.schemeId),
    sourceSecret: process.env.RESCUE_SOURCE_SECRET,
  };

  console.log('\n  Wraith Protocol: Rescue Stealth Funds\n');
  const result = await runRescue(inputs, { dryRun: !opts.yes });

  if (!opts.yes) {
    console.error('\n  Dry run only. Pass --yes to broadcast the announcement.');
    process.exit(1);
  }
  console.log('\n  Announcement published. The recipient can now scan for and sweep this payment.');
  console.log(`  Stealth address: ${result.stealthAddress}`);
  console.log(`  Tx hash:         ${result.txHash}\n`);
}

// Run only when executed directly (not when imported by tests or the drill).
if (process.argv[1]?.replace(/\\/g, '/').endsWith('rescue-stealth-funds.ts')) {
  main().catch((err) => {
    console.error(`\n  Error: ${err instanceof Error ? err.message : err}`);
    process.exit(1);
  });
}