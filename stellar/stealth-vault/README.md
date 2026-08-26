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
| `init(env, admin, announcer)` | Initialize the vault with a pause admin and an announcer address | None (one-time initialization) |
| `deposit(env, sender, recipient, amount, asset, unlock_ledger, refund_after, ephemeral_pub_key)` | Deposit tokens for a recipient with time-lock | `sender` must authorize |
| `claim(env, deposit_id, recipient)` | Claim a deposit after unlock time | `recipient` must authorize |
| `refund(env, deposit_id)` | Refund a deposit after refund window | `sender` must authorize |
| `refund_permissionless(env, caller, deposit_id)` | Return a deposit to its depositor one grace period after `refund_after` | `caller` must authorize (any address) |
| `get_deposit(env, deposit_id)` | Read a stored deposit | None |
| `pause(env, caller)` / `unpause(env, caller)` | Block / unblock new deposits | `caller` must be the admin |
| `is_paused(env)` | Whether new deposits are blocked | None |
| `admin(env)` | The pause admin | None |
| `grace_period(env)` | The configured grace period, in ledgers | None |
| `set_grace_period(env, caller, grace_period)` | Retune the grace period | `caller` must be the admin |

### `init`

Initialize the vault with a pause admin and an announcer address. Seeds the
grace period to `DEFAULT_GRACE_PERIOD` (1000 ledgers).

**Parameters:**
- `admin: Address` — The pause admin (should be a multisig; see [MULTISIG.md](../MULTISIG.md))
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
- `refund_after: u32` — Ledger number when sender can refund (must be > unlock_ledger + grace_period)
- `ephemeral_pub_key: BytesN<32>` — Ephemeral public key for recipient scanning

**Returns:** `Result<BytesN<32>, VaultError>` — The deposit ID (SHA-256 hash of deposit parameters)

**Validation:**
- Contract must not be paused
- `refund_after > unlock_ledger + grace_period` (saturating; 1000 ledgers by default)
- Contract must be initialized

**Events emitted:**
- `("deposit", deposit_id)` with `(sender, amount, asset, unlock_ledger)`
- Announcement event via announcer contract (scheme_id=2, metadata=[view_tag])

**Errors:**
- `Paused` — New deposits are blocked
- `NotInitialized` — Contract not initialized
- `InvalidWindow` — `refund_after` is not > `unlock_ledger + grace_period`

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

### `refund_permissionless`

Return an unclaimed deposit to its depositor without the depositor's signature,
one grace period after `refund_after`. Funds always go to the recorded
depositor — `caller` only pays the fee and is authorized so the invocation is
attributable. This keeps a depositor who has lost their key from stranding funds
in the vault forever.

**Parameters:**
- `caller: Address` — Whoever triggers the refund (must authorize)
- `deposit_id: BytesN<32>` — The deposit ID returned by `deposit`

**Returns:** `Result<(), VaultError>`

**Validation:**
- Deposit must exist
- Current ledger >= `refund_after + grace_period` (saturating)

**Events emitted:**
- `("refund", deposit_id)` with `(sender, amount)` — the same event the
  depositor path emits, so indexers need no change

**Errors:**
- `DepositNotFound` — Deposit ID does not exist
- `NotYetPermissionless` — Current ledger < `refund_after + grace_period`

## Error Variants

| Error Code | Description |
|------------|-------------|
| `AlreadyInitialized = 1` | Contract already initialized |
| `NotInitialized = 2` | Contract not initialized |
| `InvalidWindow = 3` | Refund window is not > unlock_ledger + grace_period |
| `DepositNotFound = 4` | Deposit ID does not exist |
| `NotYetUnlocked = 5` | Current ledger < unlock_ledger |
| `NotYetRefundable = 6` | Current ledger < refund_after |
| `WrongRecipient = 7` | Caller is not the deposit recipient |
| `Paused = 8` | New deposits are blocked; exits remain callable |
| `NotYetPermissionless = 9` | Permissionless refund window has not opened |
| `InvalidGracePeriod = 10` | `set_grace_period` was called with zero |

## Event Topics

| Topic | Data | Description |
|-------|------|-------------|
| `("deposit", deposit_id)` | `(sender, amount, asset, unlock_ledger)` | Deposit created |
| `("claim", deposit_id)` | `(recipient, amount)` | Deposit claimed |
| `("refund", deposit_id)` | `(sender, amount)` | Deposit refunded (either refund path) |
| `("paused",)` | `(caller,)` | Deposits blocked by the admin |
| `("unpaused",)` | `(caller,)` | Deposits re-enabled by the admin |
| `("grace",)` | `(caller, grace_period)` | Grace period retuned by the admin |

## Storage Layout

### Instance Storage
- `DataKey::Announcer: Address` — The stealth announcer contract address
- `DataKey::Admin: Address` — The pause admin
- `DataKey::Paused: bool` — Whether new deposits are blocked
- `DataKey::GracePeriod: u32` — Configurable grace period, in ledgers

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
| Pausable | Yes — `deposit` only; `claim`, `refund`, and `refund_permissionless` stay callable so users can always exit |
| Admin | Yes — set at `init`; can pause, unpause, and retune the grace period |
| Metrics | Yes — `deposit_count`, `deposit_volume`, `claim_count`, `refund_count` |

## Related Docs

- [AUDIT_SUMMARY.md](./AUDIT_SUMMARY.md) — Security audit, deposit-id derivation,
  the single-invocation model and why no reentrancy guard is required
- [PAUSE.md](../PAUSE.md) — Pause posture documentation
- [MULTISIG.md](../MULTISIG.md) — Multisig setup documentation
- [METRICS.md](../METRICS.md) — Metrics standard documentation
- [PERF.md](../PERF.md) — Gas bench numbers

## Constants

- `DEFAULT_GRACE_PERIOD: u32 = 1000` — Grace period seeded at `init`; also the
  fallback for vaults deployed before the key existed
- `ANNOUNCE_SCHEME_ID: u32 = 2` — Must match `stealth_announcer::STELLAR_V2_SCHEME_ID`
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
- Admin recorded at `init`, one-shot `init`, grace-period retuning and rejection of zero
- Pause / unpause by the admin, rejection of non-admins, `deposit` blocked while
  paused, `claim` / `refund` / `refund_permissionless` still callable while paused
- Permissionless refund after the grace window, before it, and after a claim
- Metric event shape for all four vault metrics
- Integration against the real `stealth-announcer` (`tests/announcer.rs`)

## Formal Verification

Time-lock invariants are machine-checked with [Kani](https://model-checking.github.io/kani/):

```bash
cargo kani --package stealth-vault
```

The proofs live in `src/proofs/mod.rs` and run against the real `claim` /
`refund` / `refund_permissionless` bodies, compiled against `src/mock_sdk.rs` in
place of `soroban-sdk`. See [AUDIT_SUMMARY.md](./AUDIT_SUMMARY.md) for what the
model assumes.
