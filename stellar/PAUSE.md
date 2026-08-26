# Stellar Contract Pause Posture

## Pattern
Admin-only pause via `DataKey::Paused` in contract instance storage.
Admin address is set at `init` time and is the only address that can pause/unpause.
All state-mutating functions guard with `require_not_paused()` which returns
the contract-specific `Paused` error.

`Paused` / `Unpaused` events are emitted (topic: `"paused"` / `"unpaused"`,
data: `(caller,)`).

## Per-Contract Decision

| Contract           | Pausable? | Reason |
|--------------------|-----------|--------|
| stealth-announcer  | No        | Stateless event emitter — no storage, nothing to pause |
| stealth-registry   | No        | Not implemented; non-custodial metadata writes, registrations are not guarded |
| stealth-sender     | Yes       | Moves tokens; pause prevents sends during incident |
| wraith-names       | Yes       | Name registry with ownership; pause prevents registrations, updates, releases, and TTL extensions |
| stealth-vault      | Yes       | Custodies time-locked deposits; pause prevents new deposits during an incident |

## Guarded Surface

### stealth-sender

Guarded by `require_not_paused`:
- `send` — token transfer + announcement
- `batch_send` — batch token transfers + announcements

NOT guarded (users must be able to exit during an incident):
- `withdraw_many` — batch asset exits

### stealth-vault

Guarded by `require_not_paused`:
- `deposit` — token transfer into the vault + announcement

NOT guarded (users must be able to exit during an incident):
- `claim` — recipient takes an unlocked deposit
- `refund` — depositor reclaims an unclaimed deposit after `refund_after`
- `refund_permissionless` — anyone returns a deposit to its depositor one grace
  period after `refund_after`
- `get_deposit`, `is_paused`, `admin`, `grace_period` — read-only accessors

### wraith-names

Guarded by `require_not_paused`:
- `register` / `register_on_behalf`
- `update` / `update_on_behalf`
- `release` / `release_on_behalf`
- `extend_name_ttl`

NOT guarded (read-only lookups remain available):
- `resolve` — name → meta-address lookup
- `name_of` — reverse lookup (meta-address → name)

## Usage
```rust
// Admin initialises the pause capability
client.init(&admin);  // wraith-names only; stealth-sender and stealth-vault
                      // take the admin as an init() argument

// Pause
client.pause(&admin);

// Unpause
client.unpause(&admin);

// Check
client.is_paused(); // returns bool
```

## Events

```rust
// Pause
env.events().publish(("paused",), (caller,));

// Unpause
env.events().publish(("unpaused",), (caller,));
```