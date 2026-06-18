# PAUSE.md — Wraith Protocol Contract Pause Status

## Overview

This document describes the pause (circuit-breaker) posture for each Wraith Protocol Stellar contract. Pause functionality allows the admin to temporarily halt state-mutating operations in case of security incidents, while preserving read-only access.

## Contract Pause Status

| Contract | Pausable | Admin | Read-Only When Paused |
|---|---|---|---|
| **stealth-sender** | ✅ Yes | Configurable at init | ❌ Send/batch_send blocked |
| **wraith-names** | ✅ Yes | Configurable at init | ✅ resolve/name_of still work |
| **stealth-registry** | ❌ No | N/A | N/A (stateless, redeploy) |
| **stealth-announcer** | ❌ No | N/A | N/A (pure events) |

## Pause Behavior

### stealth-sender

**When paused:**
- `send()` → reverts with `ContractPaused`
- `batch_send()` → reverts with `ContractPaused`
- `upgrade()` → still works (admin may need to upgrade during incident)
- `set_admin()` → still works
- `pause()` / `unpause()` → still works (admin only)

**When unpaused:**
- All operations work normally

### wraith-names

**When paused:**
- `register()` → reverts with `ContractPaused`
- `update()` → reverts with `ContractPaused`
- `release()` → reverts with `ContractPaused`
- `resolve()` → ✅ still works (read-only)
- `name_of()` → ✅ still works (read-only)

**When unpaused:**
- All operations work normally

## Admin Control

Both pausable contracts require admin authorization for pause/unpause:
- `admin.require_auth()` enforced
- Admin set during `init()`
- No way to pause without admin signature

## Incident Response Playbook

1. **Detect** — Monitor for anomalous activity (unusual send volumes, unexpected name registrations)
2. **Pause** — Admin calls `pause()` on affected contract(s)
3. **Investigate** — Assess scope and impact of incident
4. **Fix** — If needed, upgrade contract (stealth-sender) or deploy fix
5. **Unpause** — Admin calls `unpause()` when safe to resume

## Testing

Pause tests are included in each contract's `#[cfg(test)]` module:

- `test_admin_can_pause` / `test_admin_can_unpause`
- `test_register_blocked_when_paused` (wraith-names)
- `test_update_blocked_when_paused` (wraith-names)
- `test_release_blocked_when_paused` (wraith-names)
- `test_resolve_works_when_paused` (wraith-names)
- `test_name_of_works_when_paused` (wraith-names)
- `test_register_works_after_unpause` (wraith-names)
