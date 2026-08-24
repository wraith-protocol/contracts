# Stealth Vault Contract (`stealth-vault`)

The `stealth-vault` contract implements a time-locked vault for stealth payments. Senders can deposit tokens for a recipient with a time-locked release window. The recipient can claim after the unlock ledger, and the sender can refund after a refund ledger if unclaimed.

## Purpose

Time-locked stealth payments with a refund safety net. Enables scenarios like:
- Scheduled payments (release at a specific future ledger)
- Escrow-style transactions (recipient has a window to claim, otherwise refund)
- Privacy-preserving time-locked transfers

## Entrypoints

| Function | Description | Authorization |
|----------|-------------|----------------|
| `init(env, announcer)` | Initialize the vault with an announcer address | None (one-time initialization) |
| `deposit(env, sender, recipient, amount, asset, unlock_ledger, refund_after, ephemeral_pub_key)` | Deposit tokens for a recipient with time-lock | `sender` must authorize |
| `claim(env, deposit_id, recipient)` | Claim a deposit after unlock time | `recipient` must authorize |
| `refund(env, deposit_id)` | Refund a deposit after refund window | `sender` must authorize |

### `init`

Initialize the vault with an announcer address.

**Parameters:**
- `announcer: Address` — The stealth announcer contract address

**Returns:** `Result<(), VaultError>`

**Errors:**
- `AlreadyInitialized` — Contract already initialized

### `deposit`

Deposit tokens for a recipient with a time-lock window.

**Parameters:**
- `sender: Address` — Sender address (must authorize)
- `recipient: Address` — Recipient stealth address
- `amount: i128` — Token amount to deposit
- `asset: Address` — Token contract address
- `unlock_ledger: u32` — Ledger number when recipient can claim
- `refund_after: u32` — Ledger number when sender can refund (must be > unlock_ledger + GRACE_PERIOD)
- `ephemeral_pub_key: BytesN<32>` — Ephemeral public key for recipient scanning

**Returns:** `Result<BytesN<32>, VaultError>` — The deposit ID (SHA-256 hash of deposit parameters)

**Validation:**
- `refund_after > unlock_ledger + GRACE_PERIOD` (1000 ledgers minimum grace period)
- Contract must be initialized

**Events emitted:**
- `("deposit", deposit_id)` with `(sender, amount, asset, unlock_ledger)`
- Announcement event via announcer contract (scheme_id=1, metadata=[view_tag])

**Errors:**
- `NotInitialized` — Contract not initialized
- `InvalidWindow` — `refund_after` is not > `unlock_ledger + GRACE_PERIOD`

### `claim`

Claim a deposit after the unlock ledger.

**Parameters:**
- `deposit_id: BytesN<32>` — The deposit ID returned by `deposit`
- `recipient: Address` — Recipient address (must authorize)

**Returns:** `Result<(), VaultError>`

**Validation:**
- Deposit must exist
- Current ledger >= `unlock_ledger`
- Caller must be the deposit recipient

**Events emitted:**
- `("claim", deposit_id)` with `(recipient, amount)`

**Errors:**
- `DepositNotFound` — Deposit ID does not exist
- `NotYetUnlocked` — Current ledger < `unlock_ledger`
- `WrongRecipient` — Caller is not the deposit recipient

### `refund`

Refund a deposit after the refund window.

**Parameters:**
- `deposit_id: BytesN<32>` — The deposit ID returned by `deposit`

**Returns:** `Result<(), VaultError>`

**Validation:**
- Deposit must exist
- Current ledger >= `refund_after`
- Caller must be the deposit sender

**Events emitted:**
- `("refund", deposit_id)` with `(sender, amount)`

**Errors:**
- `DepositNotFound` — Deposit ID does not exist
- `NotYetRefundable` — Current ledger < `refund_after`

## Error Variants

| Error Code | Description |
|------------|-------------|
| `AlreadyInitialized = 1` | Contract already initialized |
| `NotInitialized = 2` | Contract not initialized |
| `InvalidWindow = 3` | Refund window is not > unlock_ledger + GRACE_PERIOD |
| `DepositNotFound = 4` | Deposit ID does not exist |
| `NotYetUnlocked = 5` | Current ledger < unlock_ledger |
| `NotYetRefundable = 6` | Current ledger < refund_after |
| `WrongRecipient = 7` | Caller is not the deposit recipient |

## Event Topics

| Topic | Data | Description |
|-------|------|-------------|
| `("deposit", deposit_id)` | `(sender, amount, asset, unlock_ledger)` | Deposit created |
| `("claim", deposit_id)` | `(recipient, amount)` | Deposit claimed |
| `("refund", deposit_id)` | `(sender, amount)` | Deposit refunded |

## Storage Layout

### Instance Storage
- `DataKey::Announcer: Address` — The stealth announcer contract address

### Persistent Storage
- `DataKey::Deposit(deposit_id): DepositEntry` — Individual deposit entries

**DepositEntry struct:**
```rust
pub struct DepositEntry {
    pub sender: Address,
    pub recipient: Address,
    pub amount: i128,
    pub asset: Address,
    pub unlock_ledger: u32,
    pub refund_after: u32,
}
```

**TTL Strategy:**
- Instance storage: Extended to `TTL_EXTEND_TO` (518400 ledgers, ~30 days) on every write
- Deposit storage: Extended to `TTL_EXTEND_TO` on creation, removed on claim/refund

## Pause / Admin / Metrics Posture

| Feature | Status |
|---------|--------|
| Pausable | No — no pause mechanism implemented |
| Admin | No — no admin controls after initialization |
| Metrics | No — no metric events emitted |

## Related Docs

- [PAUSE.md](../PAUSE.md) — Pause posture documentation
- [MULTISIG.md](../MULTISIG.md) — Multisig setup documentation
- [METRICS.md](../METRICS.md) — Metrics standard documentation

## Constants

- `GRACE_PERIOD: u32 = 1000` — Minimum ledgers between unlock and refund window
- `TTL_THRESHOLD: u32 = 17280` — ~1 day, TTL extension threshold
- `TTL_EXTEND_TO: u32 = 518400` — ~30 days, TTL extension target

## Deposit ID Derivation

The deposit ID is a SHA-256 hash of:
- `amount` (big-endian bytes)
- `unlock_ledger` (big-endian bytes)
- `refund_after` (big-endian bytes)
- `ephemeral_pub_key` (32 bytes)
- Current ledger sequence (big-endian bytes)

This ensures deterministic, unique IDs for each deposit.

## Testing

```bash
cargo test -p stealth-vault
```

Tests cover:
- Deposit and claim flow
- Claim before unlock rejection
- Refund after window success
- Refund before window rejection
- Double claim rejection
- Wrong recipient claim rejection
- Refund window validation
- Sender early refund rejection
