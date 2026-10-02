import { describe, it, expect, vi, afterEach } from 'vitest';
import { StrKey } from '@stellar/stellar-sdk';
import { generateRecipient, recipientDerive } from '../stealth-derivation';
import {
  parseMetaAddress,
  recomputeStealthAddress,
  buildAnnouncementPayload,
  balanceMatches,
  queryBalance,
  hasFundsBeenMoved,
  type RescueInputs,
} from '../rescue-stealth-funds';

const EPHEMERAL_KEY = '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';

function inputsFor(metaAddress: string): RescueInputs {
  return {
    ephemeralKey: EPHEMERAL_KEY,
    recipientMetaAddress: metaAddress,
    amount: '100.0',
    asset: 'XLM',
    announcerId: 'CDLZFC3SYJYDKTNBT7YIJ4HPN5XKKBYYY7QB7QY7PJY7PJY7PJY7PJY',
    rpc: 'https://soroban-testnet.stellar.org',
    horizon: 'https://horizon-testnet.stellar.org',
    networkPassphrase: 'Test SDF Network ; September 2015',
  };
}

afterEach(() => vi.unstubAllGlobals());

describe('parseMetaAddress', () => {
  it('splits a 64-byte meta-address into spending and viewing keys', () => {
    const meta = 'aa'.repeat(32) + 'bb'.repeat(32);
    const r = parseMetaAddress(meta);
    expect(r.spendingPubKey).toBe('aa'.repeat(32));
    expect(r.viewingPubKey).toBe('bb'.repeat(32));
  });

  it('rejects meta-addresses that are not exactly 64 bytes', () => {
    expect(() => parseMetaAddress('aabb')).toThrow('exactly 64 bytes');
    expect(() => parseMetaAddress('aa'.repeat(65))).toThrow('exactly 64 bytes');
    expect(() => parseMetaAddress('zz'.repeat(32))).toThrow();
  });
});

describe('recomputeStealthAddress', () => {
  it('produces a real, valid Stellar G... account', () => {
    const recipient = generateRecipient();
    const { stealthAddress } = recomputeStealthAddress(EPHEMERAL_KEY, recipient.metaAddress);
    expect(stealthAddress).toMatch(/^G/);
    expect(StrKey.isValidEd25519PublicKey(stealthAddress)).toBe(true);
  });

  it('is deterministic and differs per recipient', () => {
    const a = generateRecipient();
    const b = generateRecipient();
    const a1 = recomputeStealthAddress(EPHEMERAL_KEY, a.metaAddress);
    const a2 = recomputeStealthAddress(EPHEMERAL_KEY, a.metaAddress);
    const b1 = recomputeStealthAddress(EPHEMERAL_KEY, b.metaAddress);
    expect(a1.stealthAddress).toBe(a2.stealthAddress);
    expect(a1.stealthAddress).not.toBe(b1.stealthAddress);
  });

  it('lets the recipient rederive the same address from the announced ephemeral key', () => {
    const recipient = generateRecipient();
    const { stealthAddress, ephemeralPubKey } = recomputeStealthAddress(EPHEMERAL_KEY, recipient.metaAddress);
    expect(recipientDerive(recipient, ephemeralPubKey).address).toBe(stealthAddress);
  });

  it('rejects an invalid meta-address or ephemeral key', () => {
    expect(() => recomputeStealthAddress(EPHEMERAL_KEY, 'aabb')).toThrow();
    expect(() => recomputeStealthAddress('aabb', generateRecipient().metaAddress)).toThrow('32 bytes');
  });
});

describe('buildAnnouncementPayload', () => {
  it('targets the v2 announcer with a real ephemeral key and a one-byte view tag', () => {
    const recipient = generateRecipient();
    const inputs = inputsFor(recipient.metaAddress);
    const { stealthAddress, ephemeralPubKey } = recomputeStealthAddress(EPHEMERAL_KEY, recipient.metaAddress);
    const payload = buildAnnouncementPayload(inputs, stealthAddress);
    expect(payload.schemeId).toBe(2);
    expect(payload.stealthAddress).toBe(stealthAddress);
    expect(payload.ephemeralPubKey).toBe(ephemeralPubKey);
    expect(payload.metadata).toMatch(/^[0-9a-f]{2}$/);
  });

  it('honours an explicit scheme id', () => {
    const recipient = generateRecipient();
    const { stealthAddress } = recomputeStealthAddress(EPHEMERAL_KEY, recipient.metaAddress);
    const payload = buildAnnouncementPayload({ ...inputsFor(recipient.metaAddress), schemeId: 7 }, stealthAddress);
    expect(payload.schemeId).toBe(7);
  });
});

describe('balanceMatches', () => {
  it('compares balance against the expected amount', () => {
    expect(balanceMatches('100.0', '100.0')).toBe(true);
    expect(balanceMatches('150.0', '100.0')).toBe(true);
    expect(balanceMatches('50.0', '100.0')).toBe(false);
    expect(balanceMatches(null, '100.0')).toBe(false);
  });
});

describe('balance guard', () => {
  const addr = recomputeStealthAddress(EPHEMERAL_KEY, generateRecipient().metaAddress).stealthAddress;
  const horizon = 'https://horizon.example';
  const stubBalance = (balance: string) =>
    vi.stubGlobal('fetch', vi.fn(async () => ({
      ok: true,
      json: async () => ({ balances: [{ asset_type: 'native', balance }] }),
    })));

  it('reads the native balance of a real G... address', async () => {
    stubBalance('100.0000000');
    expect(await queryBalance(addr, 'XLM', horizon)).toBe('100.0000000');
  });

  it('flags funds as moved when the account no longer exists', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => ({ ok: false, status: 404 })));
    expect(await hasFundsBeenMoved(addr, '100', horizon)).toBe(true);
  });

  it('flags funds as moved when the balance is nearly empty, but not when funds are present', async () => {
    stubBalance('5.0000000');
    expect(await hasFundsBeenMoved(addr, '100', horizon)).toBe(true);
    stubBalance('100.0000000');
    expect(await hasFundsBeenMoved(addr, '100', horizon)).toBe(false);
  });
});