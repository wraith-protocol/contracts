# GOVERNANCE.md — Wraith Protocol Contract Upgrade Policy

## Overview

This document describes the upgrade authority model for Wraith Protocol's Stellar (Soroban) contracts. It specifies which contracts are upgradeable, who controls upgrades, and under what conditions upgrades can occur.

## Contract Upgrade Classification

| Contract | Upgradeable | Admin | Notes |
|---|---|---|---|
| **stealth-sender** | ✅ Yes | Configurable at init | Core transfer logic; needs upgrade path for security patches |
| **stealth-registry** | ❌ No | N/A | Stateless registry; redeployment preferred over upgrade |
| **stealth-announcer** | ❌ No | N/A | Pure event emitter; no state to preserve |
| **wraith-names** | ❌ No | N/A | Name registry; redeployment with migration |

## Upgrade Authority Model (stealth-sender)

### Admin Role

- Set during `init()` as the second parameter
- Can be transferred to a new address via `set_admin()`
- Only the current admin can authorize upgrades or transfer admin rights

### Upgrade Mechanism

The admin can call `upgrade(new_wasm_hash)` to update the contract's WASM bytecode via Soroban's `deployer().update_current_contract_wasm()`. This:

- Replaces the contract logic while preserving all storage
- Requires admin authorization (`require_auth`)
- Fails if upgrade authority has been renounced

### Renounce Mechanism

The admin can permanently renounce upgrade authority via `renounce_upgrade_authority()`. After renouncement:

- No further upgrades are possible
- Admin cannot be changed
- This action is irreversible

### Security Properties

1. **Non-admin cannot upgrade** — `require_auth()` enforces admin signature
2. **Renounced authority cannot be re-acquired** — `Renounced` flag is one-way
3. **State preservation** — Soroban upgrades preserve instance storage
4. **No backdoor** — No way to bypass admin check or un-renounce

## Multisig Considerations

For production deployments, the admin address should be a multisig contract (e.g., Stellar native multisig or a custom threshold signer). The upgrade authority test suite verifies that:

- Single-signer admin cannot call upgrade without auth
- If admin is a multisig, threshold must be met (tested at multisig level)

## Test Coverage

See `stealth-sender/src/lib.rs` `#[cfg(test)]` module for adversarial tests covering:

- `test_non_admin_cannot_upgrade` — Auth enforcement
- `test_admin_can_upgrade` — Happy path
- `test_admin_can_renounce` — Renounce flow
- `test_cannot_renounce_twice` — Double-renounce prevention
- `test_cannot_upgrade_after_renounce` — Post-renounce upgrade blocked
- `test_cannot_set_admin_after_renounce` — Post-renounce admin change blocked
- `test_admin_can_change_admin` — Admin transfer
- `test_non_admin_cannot_change_admin` — Admin transfer auth enforcement
- `test_admin_change_preserves_announcer` — State preservation

## Mainnet Readiness

Before mainnet deployment:

- [ ] Deploy with multisig admin address
- [ ] Run full upgrade authority test suite
- [ ] Audit upgrade path with independent security reviewer
- [ ] Document admin key custody procedure
- [ ] Consider timelock for upgrades (add to contract if needed)
