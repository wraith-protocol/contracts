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

## Guarded Surface

### stealth-sender

Guarded by `require_not_paused`:
- `send` — token transfer + announcement
- `batch_send` — batch token transfers + announcements

NOT guarded (users must be able to exit during an incident):
- `withdraw_many` — batch asset exits

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
client.init(&admin);  // wraith-names only; stealth-sender admin set in init()

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