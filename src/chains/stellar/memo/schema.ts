/**
 * # Memo Schema v1
 *
 * Structured metadata encoding for Stellar memos.
 *
 * ## Wire Format (32 bytes)
 *
 * ```
 * [0]     = version (0x01)
 * [1]     = kind    (0=raw, 1=plaintext, 2=reference)
 * [2]     = data length in bytes (0-29)
 * [3..31] = data payload (max 29 bytes)
 * ```
 *
 * ## Extension Mechanism
 *
 * Add a new enum variant to `MemoKind` (values 3-255 are available).
 * Both `encode` and `decode` are pure data transformations and will
 * pass through any `kind` value unchanged. Downstream consumers match
 * on `MemoKind` and can handle new kinds gracefully via the `default`
 * branch.
 */

export const SCHEMA_VERSION = 0x01;

export enum MemoKind {
  /** Raw bytes – no schema applied, returned as-is. */
  Raw = 0,
  /** UTF-8 plaintext (e.g. a reason string). */
  Plaintext = 1,
  /** Binary reference identifier (e.g. invoice hash / order ID). */
  Reference = 2,
}

const HEADER_SIZE = 3;
const MEMO_BYTE_LEN = 32;
const MAX_PAYLOAD = MEMO_BYTE_LEN - HEADER_SIZE; // 29 bytes

export interface EncodedMemo {
  bytes: Uint8Array;
  truncated: boolean;
}

export interface DecodedMemo {
  version: typeof SCHEMA_VERSION | 0;
  kind: MemoKind;
  data: Uint8Array;
  truncated: boolean;
}

function toBytes(data: string | Uint8Array): Uint8Array {
  return typeof data === 'string' ? new TextEncoder().encode(data) : data;
}

function header(version: number, kind: number, len: number): Uint8Array {
  const h = new Uint8Array(HEADER_SIZE);
  h[0] = version;
  h[1] = kind;
  h[2] = len;
  return h;
}

/**
 * Encode structured data into a 32-byte Stellar memo.
 *
 * If the payload exceeds 29 bytes the data is **silently truncated**
 * to 29 bytes and `truncated` is set to `true` on the return value.
 * Callers that cannot afford data loss should hash the payload
 * off-chain and encode the hash as a `Reference`-kind memo instead.
 *
 * @example
 * ```ts
 * const { bytes } = encode(MemoKind.Plaintext, "order_12345");
 * // bytes is a 32-byte Uint8Array ready to use as MEMO_HASH / MEMO_RETURN
 * ```
 */
export function encode(kind: MemoKind, data: string | Uint8Array): EncodedMemo {
  if (kind === MemoKind.Raw) {
    const bytes = toBytes(data);
    const truncated = bytes.length > MEMO_BYTE_LEN;
    return { bytes: truncated ? bytes.slice(0, MEMO_BYTE_LEN) : bytes, truncated };
  }

  const payload = toBytes(data);
  const truncated = payload.length > MAX_PAYLOAD;
  const dataBytes = truncated ? payload.slice(0, MAX_PAYLOAD) : payload;
  const memo = new Uint8Array(MEMO_BYTE_LEN);

  memo.set(header(SCHEMA_VERSION, kind, dataBytes.length), 0);
  memo.set(dataBytes, HEADER_SIZE);

  return { bytes: memo, truncated };
}

/**
 * Decode a 32-byte Stellar memo into structured fields.
 *
 * **Backwards compatibility** – if the memo does not start with
 * `SCHEMA_VERSION` as its first byte, or is shorter than 3 bytes,
 * the entire memo is returned as `kind: MemoKind.Raw` with
 * `version: 0`. This ensures that memos created before this schema
 * (or by other applications) are still readable.
 *
 * @example
 * ```ts
 * const memo = decode(bytes);
 * if (memo.kind === MemoKind.Plaintext) {
 *   const text = new TextDecoder().decode(memo.data);
 * }
 * ```
 */
export function decode(memo: Uint8Array): DecodedMemo {
  if (memo.length < HEADER_SIZE || memo[0] !== SCHEMA_VERSION || memo[1] === MemoKind.Raw) {
    return {
      version: 0,
      kind: MemoKind.Raw,
      data: memo.slice(),
      truncated: false,
    };
  }

  const kind = memo[1] as MemoKind;
  const dataLen = Math.min(memo[2], MAX_PAYLOAD);
  const data = memo.slice(HEADER_SIZE, HEADER_SIZE + dataLen);

  return {
    version: SCHEMA_VERSION,
    kind,
    data,
    truncated: false,
  };
}

/**
 * Convenience: decode and, if the kind is `Plaintext`, return the
 * data as a decoded UTF-8 string.
 */
export function decodeText(memo: Uint8Array): string | null {
  const d = decode(memo);
  return d.kind === MemoKind.Plaintext
    ? new TextDecoder().decode(d.data)
    : null;
}

/**
 * Convenience: decode and, if the kind is `Reference`, return the
 * data as a hex string.
 */
export function decodeReference(memo: Uint8Array): string | null {
  const d = decode(memo);
  if (d.kind !== MemoKind.Reference) return null;
  return Array.from(d.data)
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}
