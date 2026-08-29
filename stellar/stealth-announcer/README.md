# Stealth Announcer Contract (`stealth-announcer`)

**⚠️ THIS CONTRACT IS DELIBERATELY FROZEN** — The v2 announcer deployment is a stable, stateless event emitter. No state changes are possible.

The `stealth-announcer` contract emits stealth address announcement events on Soroban. It is a pure event-emission function with no access control and no storage. Indexers watch for these events to let recipients detect incoming payments.

## Purpose

Emits v2 stealth address announcement events with bucketed view tags for efficient RPC filtering. The announcer is called by `stealth-sender` and `stealth-vault` after transfers to notify recipients.

## Entrypoints

| Function | Description | Authorization |
|----------|-------------|----------------|
| `announce(env, scheme_id, stealth_address, ephemeral_pub_key, metadata)` | Emits a v2 stealth address announcement event | None (permissionless) |

### `announce`

Emits a Stellar v2 stealth address announcement event.

**Parameters:**
- `scheme_id: u32` — Must be `2` for the v2 Stellar announcer deployment
- `stealth_address: Address` — The one-time stealth address that received funds
- `ephemeral_pub_key: BytesN<32>` — The ephemeral public key used to derive the stealth address
- `metadata: Bytes` — Non-empty metadata whose first byte is the view tag

**Returns:** None

**v2 event shape:**
- Topics: `("announce", scheme_id, view_tag_bucket, metadata_kind)`
- Data: `(stealth_address, ephemeral_pub_key, metadata)`

The stable `view_tag_bucket` derivation is `metadata[0] as u32`, where `metadata_kind = 1` (`METADATA_KIND_VIEW_TAG`) means the first metadata byte is the view tag and the remaining bytes are scheme-specific. This lets wallets and indexers filter Stellar RPC `getEvents` by scheme and bucket before doing client-side cryptographic validation.

**Migration note:** v1 announcements used the old Stellar layout `("announce", scheme_id, stealth_address)` with `(caller, ephemeral_pub_key, metadata)`. Do not reinterpret historical v1 events as v2. The compatibility path is a new announcer deployment using `scheme_id = 2`.

## Error Variants

The contract uses panics for validation (no custom error enum):

| Condition | Panic Reason |
|-----------|--------------|
| `scheme_id != 2` | Assertion failure (v1 scheme rejected) |
| Empty `metadata` | `metadata.get(0)` panic (view tag required) |

## Event Topics

| Topic | Data | Description |
|-------|------|-------------|
| `("announce", scheme_id, view_tag_bucket, metadata_kind)` | `(stealth_address, ephemeral_pub_key, metadata)` | V2 stealth payment announcement |

## Storage Layout

**None** — This contract is stateless and uses no persistent storage.

## Pause / Admin / Metrics Posture

| Feature | Status |
|---------|--------|
| Pausable | No — stateless event emitter, nothing to pause |
| Admin | No — no admin controls |
| Metrics | No — no metric events emitted |

## Related Docs

- [PAUSE.md](../PAUSE.md) — Pause posture documentation
- [MULTISIG.md](../MULTISIG.md) — Multisig setup documentation
- [METRICS.md](../METRICS.md) — Metrics standard documentation

## Constants

- `STELLAR_V2_SCHEME_ID: u32 = 2` — The v2 Stellar deployment scheme ID
- `METADATA_KIND_VIEW_TAG: u32 = 1` — Initial metadata kind for v2 announcements

## Testing

```bash
cargo test -p stealth-announcer
```

Tests cover:
- Event emission with correct topic and data structure
- View tag bucket derivation from first metadata byte
- Rejection of v1 scheme ID
- Rejection of missing view tag (empty metadata)

Error codes and panic-only coverage status are tracked in the [Stellar error catalog](../ERRORS.md#stealth-announcer).