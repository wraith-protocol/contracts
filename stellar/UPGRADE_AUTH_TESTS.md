# Upgrade Authority Enforcement Test Suite

## Overview

This document describes the comprehensive test suite that validates the upgrade authorization model for Wraith Protocol's Stellar smart contracts. These tests serve as both:

1. **Security Guarantees**: Proving that upgrade authority is correctly enforced
2. **Living Documentation**: Demonstrating the governance model through executable code

## Context

Per [GOVERNANCE.md](./GOVERNANCE.md), Wraith Protocol uses different upgrade strategies for different contracts:

- **Frozen** (No upgrade path): `stealth-announcer`, `stealth-registry`
- **Upgradeable** (planned/stub coverage): `stealth-sender`
- **Pause-only** (no contract upgrade, renunciation, guardian, recovery, or
  upgrade-timelock support): `wraith-names`

These tests verify that the governance model is correctly implemented and cannot be bypassed.

## Test Organization

Each contract has a dedicated test file in its `tests/` directory:

```
stellar/
├── stealth-announcer/tests/upgrade_auth.rs   # Frozen contract tests
├── stealth-registry/tests/upgrade_auth.rs    # Frozen contract tests
├── stealth-sender/tests/upgrade_auth.rs      # Upgradeable contract tests (stubs)
└── wraith-names/tests/admin_auth.rs          # Pause-admin tests (renamed from upgrade_auth.rs in #239)
```

## Test Categories

### 1. Frozen Contracts (`stealth-announcer`, `stealth-registry`)

These tests prove that frozen contracts have **no upgrade path** and are immutable by design.

#### Key Tests:

- **`test_no_admin_exists`**: Verifies no admin role exists in storage
- **`test_no_upgrade_function_exists`**: Proves no upgrade capability in contract interface
- **`test_deployer_cannot_upgrade_frozen_contract`**: Even deployer cannot upgrade (should panic)
- **`test_user_keys_cannot_be_censored`** (registry only): User sovereignty guaranteed
- **`test_immutability_documented`**: Living documentation of trust-minimizing design
- **`test_no_governance_infrastructure`**: No admin, timelock, multisig, or pause mechanisms
- **`test_behavior_deterministic_and_unchanging`**: Same inputs always produce same outputs
- **`test_perpetual_operation_without_admin`**: Contract works forever without admin

#### Rationale:

Per GOVERNANCE.md:
- `stealth-announcer`: "Simple, trust-minimizing contract that merely emits events. To build trust, the most-watched and foundational contract should be immutable."
- `stealth-registry`: "Holds the user's meta-address mapping and scheme keys. Keeping this frozen ensures users that their keys cannot be arbitrarily altered or censored."

### 2. `stealth-sender` (upgrade fn not yet implemented — stub tests)

These tests document the intended upgrade model for `stealth-sender`. Note that
the `upgrade` function does not exist yet; tests that reference it are stubs and
will be replaced with real coverage when the upgrade infrastructure lands.

#### Key Tests:

- **`test_non_admin_cannot_upgrade`**: Non-admin upgrade attempt panics (stub — manually panics)
- **`test_admin_can_upgrade`**: Admin upgrade stub (no-op; verified only by manual panic suppression)
- **`test_post_upgrade_state_preserved`**: Announcer address survives a no-op "upgrade"
- **`test_multisig_threshold_honored`**: 3-of-5 arithmetic verified on a local Vec (no contract call)
- **`test_renounced_authority_cannot_be_reacquired`**: Stub (no renunciation fn exists)
- **`test_timelock_delay_enforced`**: Stub (no timelock fn exists)

### 3. `wraith-names` (pause admin implemented; upgrade fn not yet implemented)

These tests prove that the **pause admin** is correctly enforced in `wraith-names` and
that the contract's core name CRUD is reliable. They replace the vacuous upgrade stubs
that previously gave false assurance about unimplemented features.

#### Key Tests:

- **`test_admin_can_pause`**: Pause admin can pause the contract
- **`test_non_admin_cannot_pause`**: Non-admin pause attempt panics
- **`test_admin_can_unpause`**: Pause admin can unpause
- **`test_non_admin_cannot_unpause`**: Non-admin unpause attempt panics
- **`test_init_is_idempotent`**: `init` is a no-op if admin already set
- **`test_paused_contract_rejects_mutations`**: All write ops fail with `Paused`
- **`test_paused_contract_allows_reads`**: `resolve` / `name_of` work while paused
- **`test_operations_resume_after_unpause`**: CRUD works normally after unpause
- **`test_name_registrations_survive_ledger_advances`**: Persistent storage outlives ledger advances
- **`test_core_operations_work_normally`**: Register / update / release / resolve baseline
- **`test_ownership_preserved_through_pause_cycle`**: Ownership enforced through pause/unpause

#### Rationale:

Per GOVERNANCE.md:
- `stealth-sender`: Upgrade infrastructure is a future work item; stubs document the intent.
- `wraith-names`: Pause-only admin is live. Upgrade / renunciation infrastructure is tracked
  as a future work item. Tests now cover what is implemented.

## Running the Tests

### Run all upgrade auth tests:

```bash
cd stellar
cargo test upgrade_auth --workspace
```

### Run tests for specific contract:

```bash
cd stellar/stealth-announcer
cargo test upgrade_auth

cd stellar/stealth-registry
cargo test upgrade_auth

cd stellar/stealth-sender
cargo test upgrade_auth

cd stellar/wraith-names
cargo test upgrade_auth
```

### Run with verbose output:

```bash
cargo test upgrade_auth --workspace -- --nocapture
```

## CI Integration

These tests run automatically on every PR via GitHub Actions:

```yaml
- name: Run Upgrade Authority Tests
  run: cd stellar && cargo test upgrade_auth --workspace
```

Any failure blocks the PR from merging, ensuring upgrade authority enforcement is always verified.

## Implementation Status

**Phase 1: Test Suite Creation (Issue #57)** ✅ COMPLETED
- [x] Create test files for all contracts
- [x] Document test rationale and coverage
- [x] Integrate with CI pipeline
- [x] Update GOVERNANCE.md with test references
- [x] Update MAINNET_READINESS.md with test checklist

**Reconciliation (Issue #239)** ✅ COMPLETED
- [x] Replace vacuous upgrade stubs in `wraith-names/tests/upgrade_auth.rs` with honest tests of what is actually implemented (pause admin, name persistence, CRUD)
- [x] Remove doc checkmarks that claimed unimplemented guarantees were proven
- [x] Add reason strings to all bare `#[ignore]` attributes in `stealth-registry/tests/upgrade_auth.rs`
- [x] Update GOVERNANCE.md to separate "implemented" from "aspirational"

**Phase 2: Admin Infrastructure Implementation (Future)**
- [ ] Add `upgrade` function to `wraith-names` with admin authorization
- [ ] Implement `renounce_admin` function
- [ ] Add multisig integration (3-of-5 threshold) gating upgrades
- [ ] Add timelock mechanism (7-day delay) for upgrades
- [ ] Add upgrade proposal storage and events
- [ ] When implemented: add full upgrade-lifecycle tests (propose → cancel,
  propose → execute past timelock, non-admin rejection, post-upgrade state
  preservation, renunciation success/failure paths)

**Phase 3: Production Deployment**
- [ ] Deploy multisig wallet with 5 guardians
- [ ] Configure admin addresses in contract initialization
- [ ] Test end-to-end upgrade flow on testnet
- [ ] Document upgrade procedures in runbooks

## Security Considerations

### Frozen Contracts

**Trust Properties:**
- Immutable forever
- No admin can censor or alter user data
- Behavior is predictable and unchanging
- Maximum trust for users

**Trade-offs:**
- Cannot fix bugs (choose simple, battle-tested logic)
- Cannot add features (accept limited functionality)
- Cannot optimize (accept current gas costs)

### Upgradeable Contracts

**Trust Properties:**
- Admin can fix critical bugs
- Admin can add new features
- 7-day timelock gives users exit window
- 3-of-5 multisig prevents single point of failure

**Trade-offs:**
- Users must trust admin during upgrade window
- Governance attack surface exists
- More complex than frozen contracts

**Mitigation:**
- Transparent upgrade proposals (emitted events)
- 7-day review period before execution
- Ability to cancel during review period
- Eventual renunciation pathway

## Governance Evolution

The upgrade authority model can evolve over time:

1. **Launch**: Multisig admin (3-of-5) with 7-day timelock
2. **Maturity**: Monitor for required upgrades, build confidence
3. **Stability**: If no upgrades needed for extended period, consider renunciation
4. **Renunciation**: Remove admin permanently, achieve frozen contract trust properties

The tests support all phases of this evolution.

## Related Documentation

- [GOVERNANCE.md](./GOVERNANCE.md) - Overall governance strategy
- [MAINNET_READINESS.md](./MAINNET_READINESS.md) - Launch checklist
- [PAUSE.md](./PAUSE.md) - Emergency pause mechanism (separate from upgrades)

## Contact

For questions about upgrade authority or governance:
- GitHub Issues: [wraith-protocol/contracts](https://github.com/wraith-protocol/contracts)
- Security Contact: `security@usewraith.xyz`

---

**Last Updated**: 2026-06-26  
**Test Suite Version**: 1.0.0  
**Status**: Test suite complete, implementation pending
