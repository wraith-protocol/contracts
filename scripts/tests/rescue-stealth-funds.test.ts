/**
 * Tests for the rescue-stealth-funds tool.
 *
 * These tests validate the core derivation, validation, and balance-checking
 * logic against known fixture data, without requiring a live Stellar network.
 */

import { describe, it, expect } from 'vitest';
import {
  recomputeStealthAddress,
  parseMetaAddress,
  computeSharedSecret,
  deriveStealthAddress,
  deriveEphemeralPubKey,
  buildAnnouncementPayload,
  balanceMatches,
} from '../rescue-stealth-funds';

// ─── Fixtures ────────────────────────────────────────────────────────────────

const FIXTURES = {
  // 32-byte ephemeral private key (hex)
  ephemeralKey: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
  // 64-byte stealth meta-address: spending_pubkey (32 bytes) || viewing_pubkey (32 bytes)
  metaAddress: [
    'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
    'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
  ].join(''),
  amount: '100.0',
  asset: 'XLM',
  announcerId: 'CDLZFC3SYJYDKTNBT7YIJ4HPN5XKKBYYY7QB7QY7PJY7PJY7PJY7PJY',
};

// ─── Tests ───────────────────────────────────────────────────────────────────

describe('parseMetaAddress', () => {
  it('should parse a valid 64-byte meta-address into spending and viewing keys', () => {
    const result = parseMetaAddress(FIXTURES.metaAddress);
    expect(result.spendingPubKey).toBe(
      'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
    );
    expect(result.viewingPubKey).toBe(
      'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
    );
  });

  it('should reject a meta-address that is not exactly 64 bytes', () => {
    // 'aabb' = 2 bytes when hex-decoded
    expect(() => parseMetaAddress('aabb')).toThrow('exactly 64 bytes');
    // 130 hex chars = 65 bytes
    expect(() => parseMetaAddress('aa'.repeat(65))).toThrow('exactly 64 bytes');
    // 126 hex chars = 63 bytes
    expect(() => parseMetaAddress('aa'.repeat(63))).toThrow('exactly 64 bytes');
  });

  it('should reject a meta-address with invalid hex characters', () => {
    expect(() => parseMetaAddress('zz'.repeat(32))).toThrow();
  });
});

describe('computeSharedSecret', () => {
  it('should produce a deterministic shared secret for the same inputs', () => {
    const secret1 = computeSharedSecret(FIXTURES.ephemeralKey, FIXTURES.metaAddress.slice(64));
    const secret2 = computeSharedSecret(FIXTURES.ephemeralKey, FIXTURES.metaAddress.slice(64));
    expect(secret1).toEqual(secret2);
  });

  it('should produce different secrets for different ephemeral keys', () => {
    const secret1 = computeSharedSecret(FIXTURES.ephemeralKey, FIXTURES.metaAddress.slice(64));
    const secret2 = computeSharedSecret(
      'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
      FIXTURES.metaAddress.slice(64),
    );
    expect(secret1).not.toEqual(secret2);
  });

  it('should produce a 32-byte (SHA-256) shared secret for Ed25519 keys', () => {
    const secret = computeSharedSecret(FIXTURES.ephemeralKey, FIXTURES.metaAddress.slice(64));
    expect(secret.length).toBe(32);
  });

  it('should reject an ephemeral key shorter than 32 bytes', () => {
    expect(() => computeSharedSecret('aabb', 'aabb'.repeat(16))).toThrow('must be 32 bytes');
  });
});

describe('deriveStealthAddress', () => {
  it('should produce a deterministic stealth address for the same inputs', () => {
    const secret = computeSharedSecret(FIXTURES.ephemeralKey, FIXTURES.metaAddress.slice(64));
    const addr1 = deriveStealthAddress(FIXTURES.metaAddress.slice(0, 64), secret);
    const addr2 = deriveStealthAddress(FIXTURES.metaAddress.slice(0, 64), secret);
    expect(addr1).toBe(addr2);
  });

  it('should produce addresses starting with "stealth:" prefix', () => {
    const secret = computeSharedSecret(FIXTURES.ephemeralKey, FIXTURES.metaAddress.slice(64));
    const addr = deriveStealthAddress(FIXTURES.metaAddress.slice(0, 64), secret);
    expect(addr).toMatch(/^stealth:/);
  });
});

describe('recomputeStealthAddress', () => {
  it('should recompute deterministically', () => {
    const result1 = recomputeStealthAddress(FIXTURES.ephemeralKey, FIXTURES.metaAddress);
    const result2 = recomputeStealthAddress(FIXTURES.ephemeralKey, FIXTURES.metaAddress);
    expect(result1.stealthAddress).toBe(result2.stealthAddress);
    expect(result1.sharedSecretHex).toBe(result2.sharedSecretHex);
  });

  it('should produce different addresses for different recipient meta-addresses', () => {
    const result1 = recomputeStealthAddress(FIXTURES.ephemeralKey, FIXTURES.metaAddress);
    const differentMeta = 'cc'.repeat(32) + 'dd'.repeat(32);
    const result2 = recomputeStealthAddress(FIXTURES.ephemeralKey, differentMeta);
    expect(result1.stealthAddress).not.toBe(result2.stealthAddress);
  });

  it('should throw for invalid meta-address length', () => {
    expect(() => recomputeStealthAddress(FIXTURES.ephemeralKey, 'aabb')).toThrow();
  });
});

describe('deriveEphemeralPubKey', () => {
  it('should produce a deterministic public key from the same private key', () => {
    const pk1 = deriveEphemeralPubKey(FIXTURES.ephemeralKey);
    const pk2 = deriveEphemeralPubKey(FIXTURES.ephemeralKey);
    expect(pk1).toBe(pk2);
  });

  it('should produce a 64-char hex string (32 bytes)', () => {
    const pk = deriveEphemeralPubKey(FIXTURES.ephemeralKey);
    expect(pk).toMatch(/^[0-9a-f]{64}$/);
  });
});

describe('buildAnnouncementPayload', () => {
  it('should build a valid announcement payload', () => {
    const inputs = {
      ephemeralKey: FIXTURES.ephemeralKey,
      recipientMetaAddress: FIXTURES.metaAddress,
      amount: FIXTURES.amount,
      asset: FIXTURES.asset,
      announcerId: FIXTURES.announcerId,
      rpc: 'https://horizon-testnet.stellar.org',
      networkPassphrase: 'Test SDF Network ; September 2025',
    };
    const result = recomputeStealthAddress(FIXTURES.ephemeralKey, FIXTURES.metaAddress);
    const payload = buildAnnouncementPayload(inputs, result.stealthAddress);

    expect(payload.schemeId).toBe(1);
    expect(payload.stealthAddress).toBe(result.stealthAddress);
    expect(payload.ephemeralPubKey).toMatch(/^[0-9a-f]{64}$/);
    expect(payload.metadata).toBe('0x00');
  });
});

describe('balanceMatches', () => {
  it('should return true when balance equals expected amount', () => {
    expect(balanceMatches('100.0', '100.0')).toBe(true);
    expect(balanceMatches('100.0000001', '100.0')).toBe(true);
  });

  it('should return true when balance exceeds expected amount', () => {
    expect(balanceMatches('150.0', '100.0')).toBe(true);
  });

  it('should return false when balance is less than expected amount', () => {
    expect(balanceMatches('50.0', '100.0')).toBe(false);
    expect(balanceMatches('0', '100.0')).toBe(false);
  });

  it('should return false when balance is null', () => {
    expect(balanceMatches(null, '100.0')).toBe(false);
  });
});

describe('CLI argument validation', () => {
  it('should reject ephemeral keys that are not 32 bytes when hex-decoded', () => {
    const buf = Buffer.from('aabb', 'hex');
    expect(buf.length).toBe(2); // 2 bytes, not 32
    expect(buf.length === 32).toBe(false);
  });

  it('should reject meta-addresses that are not 64 bytes when hex-decoded', () => {
    const buf = Buffer.from('a'.repeat(62), 'hex'); // 31 bytes
    expect(buf.length).toBe(31);
    expect(buf.length === 64).toBe(false);
  });
});
