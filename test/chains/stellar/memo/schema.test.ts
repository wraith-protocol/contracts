import { describe, it, expect } from 'vitest';
import {
  SCHEMA_VERSION,
  MemoKind,
  encode,
  decode,
  decodeText,
  decodeReference,
} from '../../../../src/chains/stellar/memo/schema';

function hex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

// ---------------------------------------------------------------------------
// encode
// ---------------------------------------------------------------------------

describe('encode', () => {
  it('encodes a Plaintext memo with a short string', () => {
    const { bytes, truncated } = encode(MemoKind.Plaintext, 'hello');
    expect(truncated).toBe(false);
    expect(bytes[0]).toBe(SCHEMA_VERSION);
    expect(bytes[1]).toBe(MemoKind.Plaintext);
    expect(bytes[2]).toBe(5); // length of "hello"
    expect(new TextDecoder().decode(bytes.slice(3, 8))).toBe('hello');
    // remaining bytes should be zeroed
    expect(bytes.slice(8).every((b) => b === 0)).toBe(true);
  });

  it('encodes a Reference memo with binary data', () => {
    const ref = new Uint8Array([0xab, 0xcd, 0xef]);
    const { bytes, truncated } = encode(MemoKind.Reference, ref);
    expect(truncated).toBe(false);
    expect(bytes[1]).toBe(MemoKind.Reference);
    expect(bytes[2]).toBe(3);
    expect(bytes.slice(3, 6)).toEqual(new Uint8Array([0xab, 0xcd, 0xef]));
  });

  it('truncates payloads larger than 29 bytes', () => {
    const long = 'a'.repeat(50);
    const { bytes, truncated } = encode(MemoKind.Plaintext, long);
    expect(truncated).toBe(true);
    expect(bytes[2]).toBe(29);
    const decoded = new TextDecoder().decode(bytes.slice(3, 32));
    expect(decoded).toBe('a'.repeat(29));
  });

  it('Raw kind bypasses schema header', () => {
    const raw = new Uint8Array([0x01, 0x02, 0x03]);
    const { bytes, truncated } = encode(MemoKind.Raw, raw);
    expect(truncated).toBe(false);
    expect(bytes).toEqual(raw);
  });

  it('Raw kind truncates at 32 bytes', () => {
    const raw = new Uint8Array(40).fill(0xff);
    const { bytes, truncated } = encode(MemoKind.Raw, raw);
    expect(truncated).toBe(true);
    expect(bytes.length).toBe(32);
    expect(bytes.every((b) => b === 0xff)).toBe(true);
  });

  it('Raw kind accepts a string', () => {
    const { bytes, truncated } = encode(MemoKind.Raw, 'abc');
    expect(truncated).toBe(false);
    expect(bytes).toEqual(new TextEncoder().encode('abc'));
  });

  it('produces exactly 32 bytes for non-Raw kinds', () => {
    const { bytes } = encode(MemoKind.Plaintext, 'hi');
    expect(bytes.length).toBe(32);
  });

  it('handles empty data string', () => {
    const { bytes, truncated } = encode(MemoKind.Plaintext, '');
    expect(truncated).toBe(false);
    expect(bytes[2]).toBe(0);
  });

  it('handles empty binary data', () => {
    const { bytes, truncated } = encode(MemoKind.Reference, new Uint8Array(0));
    expect(truncated).toBe(false);
    expect(bytes[2]).toBe(0);
  });

  it('encodes with maximum 29-byte payload without truncation', () => {
    const data = 'x'.repeat(29);
    const { bytes, truncated } = encode(MemoKind.Plaintext, data);
    expect(truncated).toBe(false);
    expect(bytes[2]).toBe(29);
  });

  it('encodes with 30-byte payload triggers truncation', () => {
    const data = 'x'.repeat(30);
    const { bytes, truncated } = encode(MemoKind.Plaintext, data);
    expect(truncated).toBe(true);
    expect(bytes[2]).toBe(29);
  });
});

// ---------------------------------------------------------------------------
// decode
// ---------------------------------------------------------------------------

describe('decode', () => {
  it('decodes a v1 schema memo', () => {
    const { bytes } = encode(MemoKind.Plaintext, 'test');
    const d = decode(bytes);
    expect(d.version).toBe(SCHEMA_VERSION);
    expect(d.kind).toBe(MemoKind.Plaintext);
    expect(new TextDecoder().decode(d.data)).toBe('test');
  });

  it('returns Raw for non-schema bytes (backwards compat)', () => {
    const raw = new Uint8Array([0x00, 0x01, 0x02]);
    const d = decode(raw);
    expect(d.version).toBe(0);
    expect(d.kind).toBe(MemoKind.Raw);
    expect(d.data).toEqual(raw);
  });

  it('returns Raw for empty memo', () => {
    const d = decode(new Uint8Array(0));
    expect(d.version).toBe(0);
    expect(d.kind).toBe(MemoKind.Raw);
  });

  it('returns Raw for single-byte memo', () => {
    const d = decode(new Uint8Array([0x01]));
    expect(d.version).toBe(0);
    expect(d.kind).toBe(MemoKind.Raw);
  });

  it('returns Raw if version byte is unrecognized', () => {
    const buf = new Uint8Array(32);
    buf[0] = 0x02; // unknown version
    const d = decode(buf);
    expect(d.version).toBe(0);
    expect(d.kind).toBe(MemoKind.Raw);
  });

  it('decodes Reference kind correctly', () => {
    const ref = new Uint8Array([0xde, 0xad, 0xbe, 0xef]);
    const { bytes } = encode(MemoKind.Reference, ref);
    const d = decode(bytes);
    expect(d.kind).toBe(MemoKind.Reference);
    expect(d.data).toEqual(ref);
  });

  it('decodes payload with length less than 30', () => {
    const { bytes } = encode(MemoKind.Plaintext, 'short');
    const d = decode(bytes);
    expect(d.data.length).toBe(5);
  });
});

// ---------------------------------------------------------------------------
// round-trip
// ---------------------------------------------------------------------------

describe('round-trip', () => {
  it('Plaintext string round-trips correctly', () => {
    const original = 'invoice_abc_123';
    const { bytes } = encode(MemoKind.Plaintext, original);
    const decoded = decode(bytes);
    expect(decoded.kind).toBe(MemoKind.Plaintext);
    expect(new TextDecoder().decode(decoded.data)).toBe(original);
  });

  it('Reference binary round-trips correctly', () => {
    const original = new Uint8Array([0x01, 0x02, 0x03, 0x04, 0x05]);
    const { bytes } = encode(MemoKind.Reference, original);
    const decoded = decode(bytes);
    expect(decoded.kind).toBe(MemoKind.Reference);
    expect(decoded.data).toEqual(original);
  });

  it('Raw bytes round-trip correctly', () => {
    const original = new Uint8Array([0xa1, 0xb2, 0xc3]);
    const { bytes } = encode(MemoKind.Raw, original);
    const decoded = decode(bytes);
    expect(decoded.kind).toBe(MemoKind.Raw);
    expect(decoded.data).toEqual(original);
  });

  it('truncated Plaintext round-trips the truncated portion', () => {
    const long = 'z'.repeat(35);
    const { bytes, truncated } = encode(MemoKind.Plaintext, long);
    expect(truncated).toBe(true);
    const decoded = decode(bytes);
    expect(new TextDecoder().decode(decoded.data)).toBe('z'.repeat(29));
  });
});

// ---------------------------------------------------------------------------
// decodeText / decodeReference helpers
// ---------------------------------------------------------------------------

describe('decodeText', () => {
  it('returns text for Plaintext kind', () => {
    const { bytes } = encode(MemoKind.Plaintext, 'hello world');
    expect(decodeText(bytes)).toBe('hello world');
  });

  it('returns null for non-Plaintext kinds', () => {
    const { bytes } = encode(MemoKind.Reference, new Uint8Array([0x01]));
    expect(decodeText(bytes)).toBeNull();
  });

  it('returns null for Raw bytes', () => {
    expect(decodeText(new Uint8Array([0x00, 0x01]))).toBeNull();
  });
});

describe('decodeReference', () => {
  it('returns hex for Reference kind', () => {
    const { bytes } = encode(MemoKind.Reference, new Uint8Array([0xde, 0xad]));
    expect(decodeReference(bytes)).toBe('dead');
  });

  it('returns null for non-Reference kinds', () => {
    const { bytes } = encode(MemoKind.Plaintext, 'x');
    expect(decodeReference(bytes)).toBeNull();
  });

  it('returns null for Raw bytes', () => {
    expect(decodeReference(new Uint8Array([0xde, 0xad]))).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Extension mechanism
// ---------------------------------------------------------------------------

describe('extension mechanism', () => {
  it('custom kind (3) round-trips without data loss', () => {
    const kind = 3 as MemoKind;
    const data = new Uint8Array([0xca, 0xfe]);
    const { bytes } = encode(kind, data);
    const decoded = decode(bytes);
    expect(decoded.kind).toBe(kind);
    expect(decoded.data).toEqual(data);
  });

  it('custom kind (255) round-trips without data loss', () => {
    const kind = 255 as MemoKind;
    const data = new TextEncoder().encode('ext-data');
    const { bytes } = encode(kind, data);
    const decoded = decode(bytes);
    expect(decoded.kind).toBe(kind);
    expect(new TextDecoder().decode(decoded.data)).toBe('ext-data');
  });
});

// ---------------------------------------------------------------------------
// Encoding edge cases – hex output verification
// ---------------------------------------------------------------------------

describe('hex output', () => {
  it('Plaintext "hi" produces expected hex', () => {
    const { bytes } = encode(MemoKind.Plaintext, 'hi');
    // [01] [01] [02] [68 69] + 27 zero bytes
    expect(bytes[0]).toBe(0x01);
    expect(bytes[1]).toBe(0x01);
    expect(bytes[2]).toBe(0x02);
    expect(bytes[3]).toBe(0x68); // 'h'
    expect(bytes[4]).toBe(0x69); // 'i'
    expect(bytes.slice(5).every((b) => b === 0)).toBe(true);
  });

  it('Reference with single byte', () => {
    const { bytes } = encode(MemoKind.Reference, new Uint8Array([0xff]));
    expect(hex(bytes.slice(0, 4))).toBe('010201ff');
  });
});
