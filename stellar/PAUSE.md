# Stellar Contract Pause Posture

## Pattern
Admin-only pause via `DataKey::Paused` in contract storage.
Upgrade authority (set at init) is the only address that can pause/unpause.
All state-mutating functions guard with `require_not_paused!`.

## Per-Contract Decision

| Contract           | Pausable? | Reason |
|--------------------|-----------|--------|
| stealth-announcer  | No        | Stateless event emitter — no storage, nothing to pause |
| stealth-registry   | Yes       | Stores stealth meta-addresses; pause prevents new registrations during incident |
| stealth-sender     | Yes       | Moves tokens; pause prevents sends during incident |
| wraith-names       | Yes       | Name registry with ownership; pause prevents registrations/releases |

## Usage
```rust
// Pause
client.pause();

// Unpause  
client.unpause();

// Check
client.is_paused(); // returns bool
```