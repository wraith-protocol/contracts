import { ed25519 } from '@noble/curves/ed25519.js';
import { createHash, randomBytes } from 'crypto';
import { Keypair, StrKey } from '@stellar/stellar-sdk';

// Ed25519 stealth derivation (DKSAP-style), shared by the rescue tool and the recovery drill.
const L = 2n ** 252n + 27742317777372353535851937790883648493n;
const Point = ed25519.Point;

const sha512 = (...parts: Uint8Array[]): Buffer => {
  const h = createHash('sha512');
  parts.forEach((p) => h.update(p));
  return h.digest();
};
const leToBig = (b: Uint8Array): bigint => {
  let n = 0n;
  for (let i = b.length - 1; i >= 0; i--) n = (n << 8n) | BigInt(b[i]);
  return n;
};
const bigToLe32 = (n: bigint): Buffer => {
  const out = Buffer.alloc(32);
  let x = n;
  for (let i = 0; i < 32; i++) {
    out[i] = Number(x & 0xffn);
    x >>= 8n;
  }
  return out;
};
const mod = (n: bigint): bigint => ((n % L) + L) % L;
const randomScalar = (): bigint => mod(leToBig(randomBytes(64))) || 1n;
const pubHex = (scalar: bigint): string => Point.BASE.multiply(scalar).toHex();

export interface Recipient {
  spendScalar: bigint;
  viewScalar: bigint;
  metaAddress: string; // spend_pub || view_pub (64 bytes hex)
}

export function generateRecipient(): Recipient {
  const spendScalar = randomScalar();
  const viewScalar = randomScalar();
  return { spendScalar, viewScalar, metaAddress: pubHex(spendScalar) + pubHex(viewScalar) };
}

function hashFromShared(sharedPointHex: string): { h: bigint; prefix: Buffer } {
  const shared = Buffer.from(sharedPointHex, 'hex');
  const h = mod(leToBig(sha512(Buffer.from('wraith-drill-h'), shared)));
  const prefix = sha512(Buffer.from('wraith-drill-prefix'), shared).subarray(0, 32);
  return { h, prefix };
}

/** Sender side: derive the stealth account from an ephemeral scalar + recipient meta-address. */
export function senderDerive(ephScalar: bigint, metaAddressHex: string) {
  const spendPubHex = metaAddressHex.slice(0, 64);
  const viewPubHex = metaAddressHex.slice(64, 128);
  const shared = Point.fromHex(viewPubHex).multiply(ephScalar).toHex();
  const { h } = hashFromShared(shared);
  const stealthPub = Point.fromHex(spendPubHex).add(Point.BASE.multiply(h));
  const stealthPubBytes = Buffer.from(stealthPub.toHex(), 'hex');
  return {
    ephPubHex: pubHex(ephScalar),
    stealthPubBytes,
    address: StrKey.encodeEd25519PublicKey(stealthPubBytes),
  };
}

/** Recipient side: rederive the same account plus the private scalar needed to sign for it. */
export function recipientDerive(r: Recipient, ephPubHex: string) {
  const shared = Point.fromHex(ephPubHex).multiply(r.viewScalar).toHex();
  const { h, prefix } = hashFromShared(shared);
  const stealthScalar = mod(r.spendScalar + h);
  const stealthPubBytes = Buffer.from(pubHex(stealthScalar), 'hex');
  return {
    stealthScalar,
    prefix,
    stealthPubBytes,
    address: StrKey.encodeEd25519PublicKey(stealthPubBytes),
  };
}

/** Standard Ed25519 signature using a raw scalar + nonce prefix (no seed needed). */
export function signWithScalar(
  msg: Uint8Array,
  stealthScalar: bigint,
  prefix: Uint8Array,
  pubBytes: Uint8Array,
): Buffer {
  const r = mod(leToBig(sha512(prefix, msg)));
  const R = Buffer.from(pubHex(r), 'hex');
  const k = mod(leToBig(sha512(R, pubBytes, msg)));
  const S = mod(r + k * stealthScalar);
  return Buffer.concat([R, bigToLe32(S)]);
}

export const randomEphemeralScalar = randomScalar;

/** Reduce a 32-byte hex string to a valid Ed25519 scalar (little-endian, mod L). */
export function scalarFromHex(hex: string): bigint {
  const buf = Buffer.from(hex, 'hex');
  if (buf.length !== 32) {
    throw new Error(`Ephemeral private key must be 32 bytes, got ${buf.length}`);
  }
  const s = mod(leToBig(buf));
  if (s === 0n) throw new Error('Ephemeral private key reduces to zero');
  return s;
}

/** Public key (32-byte hex) for a scalar. */
export const publicKeyHex = pubHex;

/** ECDH shared point (32-byte hex): scalar * viewing public key. */
export function sharedPointHex(scalar: bigint, viewPubHex: string): string {
  return Point.fromHex(viewPubHex).multiply(scalar).toHex();
}

/** Stealth account from the spending public key and a shared point. */
export function stealthFromShared(spendPubHex: string, sharedHex: string) {
  const { h } = hashFromShared(sharedHex);
  const stealthPub = Point.fromHex(spendPubHex).add(Point.BASE.multiply(h));
  const stealthPubBytes = Buffer.from(stealthPub.toHex(), 'hex');
  return { stealthPubBytes, address: StrKey.encodeEd25519PublicKey(stealthPubBytes) };
}

// ── Self-test: npx tsx scripts/stealth-derivation.ts ──
if (process.argv[1]?.replace(/\\/g, '/').endsWith('stealth-derivation.ts')) {
  const recipient = generateRecipient();
  const eph = randomScalar();
  const s = senderDerive(eph, recipient.metaAddress);
  const r = recipientDerive(recipient, s.ephPubHex);
  console.log('sender address   :', s.address);
  console.log('recipient address:', r.address);
  if (s.address !== r.address) throw new Error('FAIL: addresses differ');
  const msg = randomBytes(32);
  const sig = signWithScalar(msg, r.stealthScalar, r.prefix, r.stealthPubBytes);
  const ok = Keypair.fromPublicKey(r.address).verify(msg, sig);
  if (!ok) throw new Error('FAIL: signature did not verify');
  console.log('SELF-TEST PASSED: addresses match and Stellar verifies the signature');
}