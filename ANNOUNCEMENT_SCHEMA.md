# Cross-Chain Announcement Schema Conformance Audit

This document audits announcement schema compatibility across Wraith-supported chains and recommends a canonical normalized SDK model.

## Canonical normalized `Announcement` form (SDK-facing)

```ts
interface Announcement {
  chain: "evm" | "stellar" | "solana" | "ckb";
  schemeId: number | null;            // null for CKB lock-script style announcements
  stealthAddress: Uint8Array;         // canonical raw bytes for recipient identity
  caller: Uint8Array | null;          // null when not emitted by chain primitive
  ephemeralPubKey: Uint8Array;        // compressed key bytes (32 or 33 depending on chain)
  metadata: Uint8Array;               // opaque bytes; may be empty
  txHash: Uint8Array;
  logIndex: number | null;
}
```

## Chain-by-chain field matrix

| Field | EVM | Stellar | Solana | CKB | Diverges? |
|---|---|---|---|---|---|
| `schemeId` | `uint256` event field | topic field `u32` | event field `u32` | not present in script args | **Yes** |
| `stealthAddress` | `address` (20 bytes) | `Address` (Soroban address type) | `Pubkey` (32 bytes) | `blake160(stealth_pubkey)` in args `[33..53]` (20 bytes) | **Yes** |
| `caller` | `address` (20 bytes, `msg.sender`) | contract address in event data (`env.current_contract_address()`) | signer `Pubkey` (32 bytes) | not present | **Yes** |
| `ephemeralPubKey` | `bytes` (variable; expected compressed secp256k1, typically 33) | `BytesN<32>` | `[u8; 32]` | script args `[0..33]` (33 bytes compressed key) | **Yes** |
| `metadata` | `bytes` | `Bytes` | `Vec<u8>` | not present as independent field | **Yes** |
| Announcement carrier | EVM log `Announcement` | Soroban event topics+data tuple | Anchor `AnnouncementEvent` | lock args + witness model (no native announcer event) | **Yes** |

## Source-of-truth references

- EVM: `evm/contracts/interfaces/IERC5564Announcer.sol`, `evm/contracts/ERC5564Announcer.sol`.
- Stellar: `stellar/stealth-announcer/src/lib.rs`.
- Solana: `solana/programs/wraith-announcer/src/lib.rs`.
- CKB: `ckb/contracts/wraith-stealth-lock/src/main.rs`.

## Detailed per-field mapping

### `schemeId`
- **EVM encoded type:** `uint256`.
- **Stellar encoded type:** `u32` topic component.
- **Solana encoded type:** `u32`.
- **CKB encoded type:** absent.
- **Logical SDK type:** `number | null` (null on CKB).

**Finding (Medium):** Width mismatch (`uint256` vs `u32`) and CKB omission force SDK specialization.

### `stealthAddress`
- **EVM encoded type:** `address` (20-byte EVM account).
- **Stellar encoded type:** `Address` (Soroban abstraction).
- **Solana encoded type:** `Pubkey` (32-byte ed25519 address).
- **CKB encoded type:** `blake160(stealth_pubkey)` extracted from lock args.
- **Logical SDK type:** `Uint8Array` normalized identity bytes + chain discriminator.

**Finding (Intentional, Medium):** Different chain account models; divergence is fundamental.

### `caller`
- **EVM encoded type:** indexed `address`.
- **Stellar encoded type:** contract address emitted in event data.
- **Solana encoded type:** signer `Pubkey`.
- **CKB encoded type:** absent.
- **Logical SDK type:** `Uint8Array | null`.

**Finding (Low):** Caller semantics differ but are mostly auxiliary for scan pipelines.

### `ephemeralPubKey`
- **EVM encoded type:** `bytes` (not length-constrained on-chain).
- **Stellar encoded type:** `BytesN<32>`.
- **Solana encoded type:** `[u8; 32]`.
- **CKB encoded type:** 33-byte compressed secp256k1 key in args.
- **Logical SDK type:** `Uint8Array`.

**Finding (High):** EVM/CKB are compatible with compressed secp256k1 (33), but Stellar/Solana currently enforce 32 bytes. This is a hard divergence for byte-for-byte parity.

### `metadata`
- **EVM encoded type:** `bytes`.
- **Stellar encoded type:** `Bytes`.
- **Solana encoded type:** `Vec<u8>`.
- **CKB encoded type:** not independently encoded.
- **Logical SDK type:** `Uint8Array`.

**Finding (Low):** Dynamic-byte representations diverge syntactically but normalize naturally in SDK.

## SDK normalization status

The task requested linking SDK decoder code (`sdk/src/chains/*/announcements.ts`), but that directory is not present in this repository snapshot. As a result, SDK conformance can only be inferred from on-chain schema and contract test-level decoding patterns in this repo.

## Harmless-to-align divergences and proposed contract PRs

1. **Align `schemeId` width to `u64`/`u128`-friendly portable range docs** (no runtime breaking change needed if SDK clamps to JS-safe integer).
2. **Constrain EVM `ephemeralPubKey` to fixed length via emit-time `require` (33 bytes)** to match CKB assumptions and reduce malformed announcements.
3. **Add optional `scheme_id` surrogate in CKB args prefix** for future-proof schema uniformity (new script version only).

## Intentional divergences

1. **CKB lock-script announcement model differs fundamentally** (cell/witness/script-args model, not event logs).
2. **Address encodings are chain-native by design** (`address` vs `Address` vs `Pubkey` vs `blake160`).
3. **Caller availability differs due to execution semantics** (CKB lock script has no direct event caller field).

## Round-trip correctness tests in this repo

- **EVM:** `evm/test/ERC5564Announcer.test.ts` decodes emitted `Announcement` log with ABI parser and asserts field equality.
- **Stellar:** `stellar/stealth-announcer/src/lib.rs` tests verify emitted topics and contract identity.
- **Solana:** `solana/tests/wraith-announcer.ts` listens for and validates `announcementEvent` payload.
- **CKB:** Added `parse_lock_args_roundtrip` unit test below to validate extraction of ephemeral key and stealth hash segments from lock args.
