# Security Audit: Stealth Registry Soroban Contract

**Date**: 2026-06-02
**Auditor**: thebabalola
**Status**: Completed (with implemented fixes)

## 1. Executive Summary
The `stealth-registry` contract is a core component of the Wraith Protocol, mapping user addresses to their 64-byte stealth meta-addresses. The initial implementation was functional but had opportunities for improvement in storage efficiency, privacy, and robustness. This audit addresses specific attack vectors, storage mechanics, and future compatibility concerns.

## 2. Findings & Improvements

### 2.1 Storage Type Efficiency & Rent Strategy (Medium)
- **Original**: Used `instance()` storage for meta-addresses.
- **Risk**: `instance()` storage has a limited footprint size. As more users register, the contract would eventually hit the size limit and fail.
- **Fix**: Migrated to `persistent()` storage.
- **Storage Rent Strategy**: With `persistent()` storage, registrations no longer expire by archival natively like `temporary` storage, but Soroban still charges rent on TTL extensions. For the registry, we rely on users extending their own registration TTLs periodically or during active transactions on the network (e.g., wallet tools extending TTL automatically). In future versions, a public `extend_ttl` endpoint can be introduced if deemed necessary.

### 2.2 Storage-Key Collision Risk (Low)
- **Finding**: Assessed if two `(registrant, scheme_id)` pairs can collide via the `DataKey::MetaAddress(Address, u32)` packing.
- **Analysis**: Soroban's native XDR serialization prevents packing collisions. A tuple of `(Address, u32)` produces a distinct and deterministic XDR structure, meaning `(AddressA, 1)` cannot accidentally resolve to the same storage key as `(AddressB, 1)` or `(AddressA, 2)`.
- **Reproducer Test**: `test_finding_storage_key_collision_risk` in `tests/audit.rs`.

### 2.3 Replacement Squatting (High)
- **Finding**: Can an attacker pre-register or hijack a victim's stealth slot?
- **Analysis**: No. The registration function enforces `registrant.require_auth()`. Soroban's auth framework guarantees that the transaction was signed by the `registrant` account before any state is written. Thus, an attacker cannot front-run or squat an address they do not control.
- **Reproducer Test**: `test_finding_replacement_squatting` in `tests/audit.rs`.

### 2.4 Scheme-ID Forward Compatibility (Low)
- **Finding**: What happens with unknown `scheme_id` values?
- **Analysis**: The `scheme_id` is defined as a `u32`. The registry is intentionally agnostic to the specific schemes. This allows forward compatibility for future stealth implementations (e.g., `scheme_id=2` for the new event-topic redesign) without needing to upgrade the registry contract.
- **Reproducer Test**: `test_finding_scheme_id_forward_compatibility` in `tests/audit.rs`.

### 2.5 State Exposure & Privileged Side Channels (Low)
- **Finding**: The registry only maintains public keys (`spending_pubkey || viewing_pubkey`). There are no secret keys, administrative privileges, or side channels inside the registry. The entire contract is trust-minimizing and entirely public.

### 2.6 Replay Protection Across the Write Boundary (Info)
- **Finding**: The contract currently allows overwriting (updating) an existing registration for a given `(registrant, scheme_id)` without enforcing a sequence check.
- **Analysis**: This is intentional. Users must be able to rotate their stealth keys if they believe their viewing key is compromised or if they wish to migrate to a new setup. Since it requires `require_auth()`, unauthorized replay attacks with old keys are prevented by the Stellar network's native sequence number checks on the transaction level.
- **Reproducer Test**: `test_finding_replay_protection_across_write_boundary` in `tests/audit.rs`.

### 2.7 User Privacy: Missing Deletion (Low)
- **Original**: No way for a user to remove their stealth keys once registered.
- **Risk**: Users might want to opt-out or rotate keys completely without leaving old data on-chain.
- **Fix**: Added `remove_keys(registrant, scheme_id)` function with `require_auth()` verification, complementing the event with a `remove` event.

## 3. Conclusion
The contract is now more robust and scalable. The use of `persistent` storage is a critical fix for long-term viability, and the audit confirms that the auth model and storage keys are resilient against squatting and collision attacks.
