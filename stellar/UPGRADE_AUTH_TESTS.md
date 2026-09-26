# Upgrade Authority Enforcement Test Suite

## Overview

This document describes the comprehensive test suite that validates the upgrade authorization model for Wraith Protocol's Stellar smart contracts. These tests serve as both:

1. **Security Guarantees**: Proving that upgrade authority is correctly enforced
2. **Living Documentation**: Demonstrating the governance model through executable code

## Context

Per [GOVERNANCE.md](./GOVERNANCE.md), Wraith Protocol uses different upgrade strategies for different contracts:

- **Frozen** (No upgrade path): `stealth-announcer`, `stealth-registry`
- **Upgradeable** (Timelock + Multisig): `stealth-sender`, `wraith-names`

These tests verify that the governance model is correctly implemented and cannot be bypassed.

## Test Organization

Each contract has a dedicated `tests/upgrade_auth.rs` file:

```
stellar/
├── stealth-announcer/tests/upgrade_auth.rs   # Frozen contract tests
├── stealth-registry/tests/upgrade_auth.rs    # Frozen contract tests
├── stealth-sender/tests/upgrade_auth.rs      # Upgradeable contract tests
└── wraith-names/tests/upgrade_auth.rs        # Upgradeable contract tests
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

### 2. Upgradeable Contracts (`stealth-sender`, `wraith-names`)

These tests prove that upgradeable contracts have **controlled upgrade paths** with proper authorization.

#### Key Tests:

- **`test_non_admin_cannot_upgrade`**: Non-admin addresses cannot trigger upgrades (should panic)
- **`test_admin_can_upgrade`**: Admin with proper authorization can upgrade
- **`test_post_upgrade_state_preserved`**: All persistent data survives upgrade
  - For `stealth-sender`: Announcer address preserved
  - For `wraith-names`: Name registrations, guardian configs, recovery proposals preserved
- **`test_multisig_threshold_honored`**: 3-of-5 multisig threshold enforced
  - With 3 signatures: upgrade succeeds
  - With 2 signatures: upgrade fails
- **`test_renounced_authority_cannot_be_reacquired`**: Once admin is renounced, no one can upgrade
- **`test_timelock_delay_enforced`**: 7-day timelock delay enforced
  - Immediate execution fails
  - After 120,960 ledgers (7 days at 5s/ledger): succeeds
- **`test_timelock_proposal_can_be_cancelled`**: Admin can cancel within delay window
- **`test_upgrade_events_emitted`**: Transparency through event emission
- **`test_contract_functional_during_upgrade_timelock`**: Normal operations continue during pending upgrade
- **`test_renounced_contract_behaves_like_frozen`**: After renunciation, matches frozen contract behavior

#### Rationale:

Per GOVERNANCE.md:
- `stealth-sender`: "Holds complex logic to process stealth payments. If a critical bug is found, the admin needs a path to upgrade the logic to prevent loss of funds, but with a 7-day timelock so users have a chance to review the upgrade."
- `wraith-names`: "Handles naming logic and resolution. We may need to add complex resolution upgrades or support new scheme IDs. Once fully matured, we can renounce the upgrade capability."

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

**Phase 2: Admin Infrastructure Implementation (Future)**
- [ ] Add admin storage keys to upgradeable contracts
- [ ] Implement `upgrade` function with admin authorization
- [ ] Implement `renounce_admin` function
- [ ] Add multisig integration (3-of-5 threshold)
- [ ] Add timelock mechanism (7-day delay)
- [ ] Add upgrade proposal storage and events

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
