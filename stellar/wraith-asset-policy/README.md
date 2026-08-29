# Wraith Asset Policy Contract (`wraith-asset-policy`)

The `wraith-asset-policy` contract provides an admin-controlled asset allowlist for stealth payments. It implements the standard asset policy interface used by `stealth-sender` to filter which assets are allowed for stealth transfers.

## Purpose

Asset allowlist policy for stealth payments. This contract protects the unlinkability and user experience of stealth transfers by preventing clawback-enabled or freeze-enabled assets from being sent (as identified in audit #43). When configured in `stealth-sender`, this contract is called before every transfer to ensure the asset is allowed.

## Entrypoints

| Function | Description | Authorization |
|----------|-------------|----------------|
| `init(env, admin, default_assets)` | Initialize the policy with an admin and default allowlist | None (one-time initialization) |
| `add_asset(env, asset)` | Add an asset to the allowlist | Admin must authorize |
| `remove_asset(env, asset)` | Remove an asset from the allowlist | Admin must authorize |
| `check_asset(env, asset)` | Check if an asset is allowed | None (read-only) |

### `init`

Initialize the policy with an admin address and optional default assets.

**Parameters:**
- `admin: Address` — Admin address that can add/remove assets
- `default_assets: Vec<Address>` — Initial list of allowed assets

**Returns:** None

**Validation:**
- Contract must not already be initialized

### `add_asset`

Add an asset to the allowlist.

**Parameters:**
- `asset: Address` — Token contract address to allow

**Returns:** None

**Authorization:** Admin must authorize

**Validation:**
- Contract must be initialized

### `remove_asset`

Remove an asset from the allowlist.

**Parameters:**
- `asset: Address` — Token contract address to disallow

**Returns:** None

**Authorization:** Admin must authorize

**Validation:**
- Contract must be initialized

### `check_asset`

Check if an asset is allowed for stealth payments.

**Parameters:**
- `asset: Address` — Token contract address to check

**Returns:** `bool` — `true` if asset is allowed, `false` otherwise

**Authorization:** None (read-only)

This is the standard interface called by `stealth-sender` before transfers.

## Error Variants

The contract uses panics for validation (no custom error enum):

| Condition | Panic Reason |
|-----------|--------------|
| Already initialized | "already initialized" |
| Not initialized | "not initialized" (on admin operations) |

## Event Topics

**None** — This contract emits no events.

## Storage Layout

### Instance Storage
- `DataKey::Admin: Address` — Admin address that can add/remove assets

### Persistent Storage
- `DataKey::Asset(asset): bool` — Asset allowlist entries (true = allowed)

**TTL Strategy:** Not explicitly managed in this contract (relies on default Soroban TTL behavior).

## Pause / Admin / Metrics Posture

| Feature | Status |
|---------|--------|
| Pausable | No — no pause mechanism implemented |
| Admin | Yes — admin can add/remove assets (set at init) |
| Metrics | No — no metric events emitted |

## Related Docs

- [PAUSE.md](../PAUSE.md) — Pause posture documentation
- [MULTISIG.md](../MULTISIG.md) — Multisig setup documentation (for admin key)
- [METRICS.md](../METRICS.md) — Metrics standard documentation

## Asset Policy Interface

This contract implements the standard asset policy interface expected by `stealth-sender`:

```rust
pub fn check_asset(env: Env, asset: Address) -> bool;
```

- **asset**: The contract address of the Stellar Asset Contract (SAC) being checked
- **Returns**: `true` if the asset is allowed for stealth payments, or `false` otherwise

If `false` is returned, `stealth-sender` rejects the transaction with `SenderError::TokenNotAllowed`.

## Custom Policy Contracts

Any contract can act as an asset policy as long as it implements the `check_asset` interface above. Callers who want custom rules (such as check-free transfers, or automated query-based enforcement) can deploy their own contract matching the interface and configure it in `stealth-sender` during initialization.

## Testing

```bash
cargo test -p wraith-asset-policy
```

Tests cover:
- Policy allowlist flow (add, check, remove)
- Initialize with default assets
- Double initialization rejection

Error codes and panic-only coverage status are tracked in the [Stellar error catalog](../ERRORS.md#wraith-asset-policy).