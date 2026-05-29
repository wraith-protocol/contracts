# Security Audit: Stealth Registry Soroban Contract

**Date**: 2026-05-29
**Auditor**: thebabalola
**Status**: Completed (with implemented fixes)

## 1. Executive Summary
The `stealth-registry` contract is a core component of the Wraith Protocol, mapping user addresses to their 64-byte stealth meta-addresses. The initial implementation was functional but had opportunities for improvement in storage efficiency, privacy, and robustness.

## 2. Findings & Improvements

### 2.1 Storage Type Efficiency (Medium)
- **Original**: Used `instance()` storage for meta-addresses.
- **Risk**: `instance()` storage has a limited footprint size. As more users register, the contract would eventually hit the size limit and fail.
- **Fix**: Migrated to `persistent()` storage. This allows the registry to scale to a virtually unlimited number of users without affecting the contract's instance footprint.

### 2.2 User Privacy: Missing Deletion (Low)
- **Original**: No way for a user to remove their stealth keys once registered.
- **Risk**: Users might want to opt-out or rotate keys completely without leaving old data on-chain.
- **Fix**: Added `remove_keys(registrant, scheme_id)` function with `require_auth()` verification.

### 2.3 Event Consistency (Low)
- **Improvement**: Added a `remove` event to complement the `register` event, ensuring indexers can stay in sync with the registry state.

## 3. Implementation Details

### 3.1 Changes to `lib.rs`
- Switched `env.storage().instance()` to `env.storage().persistent()` in `register_keys` and `stealth_meta_address_of`.
- Implemented `remove_keys` function.
- Updated tests to cover `remove_keys` and verify storage behavior.

## 4. Conclusion
The contract is now more robust and scalable. The use of `persistent` storage is a critical fix for long-term viability.
