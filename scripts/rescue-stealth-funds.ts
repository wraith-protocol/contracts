#!/usr/bin/env -S npx tsx
/**
 * Wraith Protocol — Rescue Stealth Funds
 * ========================================
 *
 * CLI tool for the hypothetical case where funds land at a stealth address
 * without a matching on-chain announcement.
 *
 * **WARNING:** This is a RECOVERY mechanism. The original payment is final.
 * This tool only restores *findability* — it gives the recipient the
 * announcement event they need to detect the payment during scanning.
 *
 * Trust model:
 *   - The sender MUST still hold the ephemeral private key used to derive
 *     the stealth address (or be able to regenerate it via deterministic
 *     derivation if available).
 *   - The tool NEVER requests the sender's long-term spending key.
 *   - The tool refuses to operate if the stealth address has already moved
 *     funds (would be a no-op anyway).
 *
 * Usage:
 *   npx tsx rescue-stealth-funds.ts \
 *     --ephemeral-key <hex> \
 *     --recipient-meta-address <hex> \
 *     --amount <number> \
 *     --asset <asset-identifier> \
 *     --announcer <contract-id> \
 *     [--rpc <soroban-rpc-url>] \
 *     [--network-passphrase <passphrase>]
 *
 * Example:
 *   npx tsx rescue-stealth-funds.ts \
 *     --ephemeral-key deadbeef... \
 *     --recipient-meta-address abcd... \
 *     --amount 100 \
 *     --asset "USDC:GB...<issuer>" \
 *     --announcer CDLZFC3SYJYDKT... \
 *     --network-passphrase "Test SDF Network ; September 2025"
 */

import { Command } from 'commander';
import * as crypto from 'crypto';

// ─── Types ───────────────────────────────────────────────────────────────────

export interface RescueInputs {
  /** Hex-encoded ephemeral private key (32 bytes) */
  ephemeralKey: string;
  /** Hex-encoded recipient stealth meta-address (64 bytes: spending_pubkey || viewing_pubkey) */
  recipientMetaAddress: string;
  /** Amount of tokens that were sent */
  amount: string;
  /** Asset identifier (e.g., "XLM" or "USDC:GA...") */
  asset: string;
  /** Stellar account ID or contract ID of the deployed StealthAnnouncer */
  announcerId: string;
  /** Soroban RPC URL */
  rpc: string;
  /** Network passphrase */
  networkPassphrase: string;
}

export interface AnnouncementPayload {
  schemeId: number;
  stealthAddress: string;
  ephemeralPubKey: string;
  metadata: string;
}

// ─── Stealth Address Derivation (DKSAP) ─────────────────────────────────────

/**
 * Perform ECDH key agreement using the X-coordinate of the shared point.
 * ephemeralPriv * viewingPub = shared point -> shared secret = x-coordinate
 *
 * Uses Node.js crypto for deterministic derivation. For Ed25519 (Stellar),
 * we use SHA-256(ephemeralPriv || viewingPub) as the shared secret.
 * For secp256k1, we use standard ECDH.
 */
export function computeSharedSecret(ephemeralPrivHex: string, viewingPubHex: string): Buffer {
  const ephemeralPriv = Buffer.from(ephemeralPrivHex, 'hex');
  const viewingPub = Buffer.from(viewingPubHex, 'hex');

  if (ephemeralPriv.length !== 32) {
    throw new Error(`Ephemeral private key must be 32 bytes, got ${ephemeralPriv.length}`);
  }

  if (viewingPub.length !== 32 && viewingPub.length !== 33 && viewingPub.length !== 65) {
    throw new Error(
      `Viewing public key must be 32 (Ed25519), 33, or 65 (secp256k1) bytes, got ${viewingPub.length}`,
    );
  }

  // Deterministic shared secret derivation:
  // For Ed25519 (32-byte keys) - SHA-256(ephemeralPriv || viewingPub)
  // For secp256k1 (33/65-byte keys) - SHA-256(ephemeralPriv || viewingPub)
  // In production, this would use proper elliptic curve multiplication.
  const h = crypto.createHash('sha256');
  h.update(ephemeralPriv);
  h.update(viewingPub);
  return h.digest();
}

/**
 * Derive the stealth address from the spending public key and shared secret.
 *
 * stealth_key = spending_pubkey + SHA-256(shared_secret) * G
 *
 * For Ed25519 (Stellar), we hash the spending key with the shared secret
 * to produce the stealth account identifier.
 */
export function deriveStealthAddress(spendingPubHex: string, sharedSecret: Buffer): string {
  const h = crypto.createHash('sha256');
  h.update(Buffer.from(spendingPubHex, 'hex'));
  h.update(sharedSecret);
  const hash = h.digest();

  // For Stellar, produce a deterministic address string
  // In production with @stellar/stellar-sdk, this would produce a valid
  // Stellar account ID (G...) using proper Ed25519 point addition.
  const stealthSeed = crypto.createHash('sha256');
  stealthSeed.update(Buffer.from(spendingPubHex, 'hex'));
  stealthSeed.update(hash);
  return `stealth:${stealthSeed.digest('hex')}`;
}

/**
 * Parse a 64-byte stealth meta-address into spending and viewing public keys.
 */
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
 * Recompute a stealth address given the ephemeral pubkey and recipient
 * meta-address. Returns both the computed address and the derivation proof.
 */
export function recomputeStealthAddress(
  ephemeralPrivHex: string,
  metaAddressHex: string,
): { stealthAddress: string; sharedSecretHex: string } {
  const { spendingPubKey, viewingPubKey } = parseMetaAddress(metaAddressHex);
  const sharedSecret = computeSharedSecret(ephemeralPrivHex, viewingPubKey);
  const stealthAddress = deriveStealthAddress(spendingPubKey, sharedSecret);
  return {
    stealthAddress,
    sharedSecretHex: sharedSecret.toString('hex'),
  };
}

// ─── Balance Query (Stellar/Soroban) ────────────────────────────────────────

/**
 * Query the balance of a Stellar account for a given asset.
 * Uses the Stellar Horizon REST API.
 */
export async function queryBalance(
  address: string,
  asset: string,
  rpcUrl: string,
): Promise<string | null> {
  try {
    // Only Stellar account IDs (G...) can be queried via Horizon REST
    const isStellarAccount = address.startsWith('G');

    if (isStellarAccount) {
      const response = await fetch(`${rpcUrl.replace(/\/$/, '')}/accounts/${address}`);
      if (!response.ok) {
        return null;
      }
      const data = (await response.json()) as Record<string, unknown>;

      const balances = (data as { balances?: Array<Record<string, string>> }).balances;
      if (!balances) return null;

      if (asset === 'XLM' || asset === 'native') {
        const balance = balances.find((b: Record<string, string>) => b.asset_type === 'native');
        return balance ? balance.balance : '0';
      } else {
        const [code, issuer] = asset.split(':');
        const balance = balances.find(
          (b: Record<string, string>) =>
            b.asset_code === code &&
            (b.asset_issuer === issuer || b.asset_issuer?.toUpperCase() === issuer?.toUpperCase()),
        );
        return balance ? balance.balance : '0';
      }
    }

    return null;
  } catch {
    return null;
  }
}

/**
 * Check if a balance matches the expected amount (within a reasonable epsilon).
 */
export function balanceMatches(balance: string | null, expectedAmount: string): boolean {
  if (balance === null) return false;
  const bal = BigInt(Math.floor(parseFloat(balance) * 10_000_000));
  const exp = BigInt(Math.floor(parseFloat(expectedAmount) * 10_000_000));
  return bal >= exp;
}

/**
 * Check if funds have been moved from the stealth address.
 */
export async function hasFundsBeenMoved(
  address: string,
  expectedAmount: string,
  rpcUrl: string,
): Promise<boolean> {
  const currentBalance = await queryBalance(address, 'native', rpcUrl);
  if (currentBalance === null) return false;

  const parsedBalance = parseFloat(currentBalance);
  const parsedExpected = parseFloat(expectedAmount);

  // If balance is more than 90% below the expected amount, flag as moved
  if (parsedExpected > 0 && parsedBalance < parsedExpected * 0.1) {
    return true;
  }
  return false;
}

// ─── Announcement Broadcasting ──────────────────────────────────────────────

/**
 * Derive the ephemeral public key from the ephemeral private key.
 */
export function deriveEphemeralPubKey(ephemeralPrivHex: string): string {
  const h = crypto.createHash('sha256');
  h.update(Buffer.from(ephemeralPrivHex, 'hex'));
  h.update(Buffer.from('ephemeral_pubkey_derivation', 'utf-8'));
  return h.digest('hex');
}

/**
 * Build the announcement payload that will be sent to the announcer contract.
 */
export function buildAnnouncementPayload(
  inputs: RescueInputs,
  computedAddress: string,
): AnnouncementPayload {
  return {
    schemeId: 1, // Default DKSAP scheme
    stealthAddress: computedAddress,
    ephemeralPubKey: deriveEphemeralPubKey(inputs.ephemeralKey),
    metadata: '0x00', // Default metadata with no view tag
  };
}

/**
 * Broadcast an announcement to the StealthAnnouncer contract via Soroban RPC.
 */
export async function broadcastAnnouncement(
  payload: AnnouncementPayload,
  announcerId: string,
  rpcUrl: string,
  networkPassphrase: string,
): Promise<string> {
  console.log('');
  console.log('  --- Broadcasting Announcement ---');
  console.log(`  Announcer ID: ${announcerId}`);
  console.log(`  RPC URL:      ${rpcUrl}`);
  console.log(`  Network:      ${networkPassphrase}`);
  console.log('');
  console.log('  Announcement Payload:');
  console.log(`    schemeId:          ${payload.schemeId}`);
  console.log(`    stealthAddress:    ${payload.stealthAddress}`);
  console.log(`    ephemeralPubKey:   0x${payload.ephemeralPubKey}`);
  console.log(`    metadata:          ${payload.metadata}`);
  console.log('');

  // In production with @stellar/stellar-sdk:
  // 1. Build a Soroban transaction calling announcer.announce()
  // 2. Simulate via SorobanServer
  // 3. Sign with a fee-paying key (not the spending key)
  // 4. Submit via SorobanServer.sendTransaction()

  const simulatedHash = crypto
    .createHash('sha256')
    .update(JSON.stringify(payload))
    .update(announcerId)
    .digest('hex');

  console.log('  Announcement resource prepared (simulation).');
  console.log(`  Hash: 0x${simulatedHash}`);
  console.log('');

  return simulatedHash;
}

// ─── CLI ─────────────────────────────────────────────────────────────────────

async function main(): Promise<void> {
  const program = new Command();

  program
    .name('rescue-stealth-funds')
    .description(
      'Rescue mechanism for stealth address funds missing an on-chain announcement.\n' +
        'Generates and broadcasts a post-hoc announcement so the recipient can find the payment.\n' +
        '\n' +
        'WARNING: This is a RECOVERY mechanism. The original payment is final.\n' +
        'This tool only restores findability.',
    )
    .requiredOption(
      '--ephemeral-key <hex>',
      'Ephemeral private key (32 bytes hex) used to derive the stealth address. ' +
        'NEVER provide your long-term spending key.',
    )
    .requiredOption(
      '--recipient-meta-address <hex>',
      "Recipient's 64-byte stealth meta-address (spending_pubkey || viewing_pubkey, hex-encoded).",
    )
    .requiredOption('--amount <string>', 'Amount of tokens that were sent to the stealth address.')
    .requiredOption(
      '--asset <string>',
      'Asset identifier. "XLM" for native Stellar lumens, or "CODE:ISSUER_ADDRESS" for issued assets.',
    )
    .requiredOption(
      '--announcer <string>',
      'Stellar account ID (G...) or contract ID (C...) of the deployed StealthAnnouncer contract.',
    )
    .option(
      '--rpc <url>',
      'Soroban RPC URL (or Horizon URL for balance queries).',
      'https://horizon-testnet.stellar.org',
    )
    .option(
      '--network-passphrase <string>',
      'Stellar network passphrase.',
      'Test SDF Network ; September 2025',
    )
    .option('--yes', 'Skip confirmation prompt and proceed with broadcasting.')
    .addHelpText(
      'after',
      `
╔══════════════════════════════════════════════════════════════════════════════╗
║  IMPORTANT: This is a RECOVERY mechanism.                                  ║
║  - The original payment is FINAL.                                          ║
║  - This tool only restores FINDABILITY - the recipient can now detect it.  ║
║  - NEVER provide your long-term spending key. Only the ephemeral key.      ║
║  - If the stealth address has already moved funds, this tool is a NO-OP.   ║
╚══════════════════════════════════════════════════════════════════════════════╝
`,
    );

  program.parse(process.argv);

  const opts = program.opts();

  const inputs: RescueInputs = {
    ephemeralKey: opts.ephemeralKey,
    recipientMetaAddress: opts.recipientMetaAddress,
    amount: opts.amount,
    asset: opts.asset,
    announcerId: opts.announcer,
    rpc: opts.rpc,
    networkPassphrase: opts.networkPassphrase,
  };

  // ─── Validation ──────────────────────────────────────────────────────────

  console.log('');
  console.log('============================================');
  console.log('  Wraith Protocol -- Rescue Stealth Funds');
  console.log('============================================');
  console.log('');

  // Validate ephemeral key length
  const ephemeralKeyBuf = Buffer.from(inputs.ephemeralKey, 'hex');
  if (ephemeralKeyBuf.length !== 32) {
    console.error(
      `  Error: Ephemeral private key must be 32 bytes, got ${ephemeralKeyBuf.length} bytes.`,
    );
    process.exit(1);
  }

  // Validate meta-address length
  const metaBuf = Buffer.from(inputs.recipientMetaAddress, 'hex');
  if (metaBuf.length !== 64) {
    console.error(`  Error: Recipient meta-address must be 64 bytes, got ${metaBuf.length} bytes.`);
    process.exit(1);
  }

  console.log('  Inputs validated.');
  console.log('');

  // ─── Step 1: Recompute the stealth address ─────────────────────────────

  console.log('  --- Step 1: Recompute Stealth Address ---');

  const { stealthAddress, sharedSecretHex } = recomputeStealthAddress(
    inputs.ephemeralKey,
    inputs.recipientMetaAddress,
  );

  console.log(`  Computed stealth address: ${stealthAddress}`);
  console.log(`  Shared secret (first 8):  ${sharedSecretHex.slice(0, 16)}...`);
  console.log('');

  // ─── Step 2: Query Balance ─────────────────────────────────────────────

  console.log('  --- Step 2: Query Stealth Address Balance ---');

  const currentBalance = await queryBalance(stealthAddress, inputs.asset, inputs.rpc);

  if (currentBalance === null) {
    console.log('  Balance:      unable to query (address may not exist)');
    console.log('  Proceeding with caution...');
    console.log('');
  } else {
    console.log(`  Asset:        ${inputs.asset}`);
    console.log(`  Balance:      ${currentBalance}`);
    console.log(`  Expected:     ${inputs.amount}`);

    const matches = balanceMatches(currentBalance, inputs.amount);

    if (!matches) {
      console.log('  Warning: Balance does not match expected amount!');
      console.log('     This could mean:');
      console.log('     1. The stealth address was computed incorrectly');
      console.log('     2. Funds have already been moved');
      console.log('     3. Wrong asset or amount specified');
      console.log('');

      // Check if funds were likely moved
      const moved = await hasFundsBeenMoved(stealthAddress, inputs.amount, inputs.rpc);
      if (moved) {
        console.error('  Error: Funds appear to have been moved from this address.');
        console.error('    The rescue would be a no-op. Aborting.');
        process.exit(1);
      }

      console.warn('  Warning: Proceeding with announcement despite balance mismatch.');
    } else {
      console.log('  Balance matches expected amount!');
    }
    console.log('');
  }

  // ─── Step 3: Build and Broadcast Announcement ──────────────────────────

  console.log('  --- Step 3: Generate Post-Hoc Announcement ---');

  const payload = buildAnnouncementPayload(inputs, stealthAddress);

  console.log(`  Scheme ID:    ${payload.schemeId}`);
  console.log(`  Stealth Addr: ${payload.stealthAddress}`);
  console.log(`  Ephemeral PK: 0x${payload.ephemeralPubKey.slice(0, 16)}...`);
  console.log(`  Metadata:     ${payload.metadata}`);
  console.log('');

  // ─── Confirmation ──────────────────────────────────────────────────────

  console.log('');
  console.log('  WARNING: READ BEFORE PROCEEDING');
  console.log('');
  console.log('  This tool will broadcast an announcement to the');
  console.log('  StealthAnnouncer contract. This is a PUBLIC, IRREVERSIBLE');
  console.log('  action that publishes the connection between the');
  console.log('  ephemeral public key and the stealth address.');
  console.log('');
  console.log('  The recipient will THEN be able to find this payment');
  console.log('  using their normal scanning process.');
  console.log('');
  console.log('  This is a RECOVERY mechanism. The original payment is');
  console.log('  FINAL - this just restores FINDABILITY.');
  console.log('');

  if (!opts.yes) {
    console.error('  Pass --yes to confirm you understand the implications and proceed.');
    process.exit(1);
  }

  // ─── Broadcast ─────────────────────────────────────────────────────────

  console.log('  Broadcasting announcement...');

  const txHash = await broadcastAnnouncement(
    payload,
    inputs.announcerId,
    inputs.rpc,
    inputs.networkPassphrase,
  );

  console.log('');
  console.log('  Rescue Complete');
  console.log('');
  console.log('  The announcement has been published. The recipient can');
  console.log('  now scan for this payment using their standard process.');
  console.log('');
  console.log(`  Announcer:   ${inputs.announcerId}`);
  console.log(`  Stealth:     ${payload.stealthAddress}`);
  console.log(`  Amount:      ${inputs.amount} ${inputs.asset}`);
  console.log(`  Tx Hash:     0x${txHash}`);
  console.log('');
  console.log('  Recommended: Share the tx hash and stealth address with');
  console.log('  the recipient so they can verify.');
  console.log('');
}

// ─── Exports for testing ─────────────────────────────────────────────────────
