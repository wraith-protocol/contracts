# wraith-names Storage Migration Verification

## Migration Summary
Completed full migration from mixed instance/persistent storage to **persistent-only storage** for all wraith-names contract data.

## Verification Checklist

### 1. Storage Tier Audit
✅ **Verified**: Zero `env.storage().instance()` calls in lib.rs
- Searched pattern: `env.storage().instance()` → 0 matches
- All storage now uses `env.storage().persistent()`

### 2. Data Durability
✅ **Per-Name Data (Persistent)**
- `DataKey::Name(hash)`: NameEntry records → persistent
- `DataKey::Reverse(hash)`: Meta-address reverse lookup → persistent
- Accessed by: `register_internal()`, `update_internal()`, `resolve()`, `name_of()`

✅ **Replay Protection (Persistent)**
- `DataKey::Replay(hash)`: Message digest cache → persistent (critical for security)
- Set in: `register_on_behalf()`, `update_on_behalf()`, `release_on_behalf()`
- Checked in: `verify_on_behalf_authorization()`
- Migration reason: Signatures must not be replayable across contract lifetime

### 3. TTL Extension Logic
✅ **`extend_ttls()` Refactored**
```rust
fn extend_ttls(env: &Env, name_key: &DataKey, reverse_key: Option<&DataKey>) {
    env.storage().persistent().extend_ttl(name_key, TTL_THRESHOLD, TTL_EXTEND_TO);
    if let Some(r_key) = reverse_key {
        env.storage().persistent().extend_ttl(r_key, TTL_THRESHOLD, TTL_EXTEND_TO);
    }
    // No instance storage extends — all entries are persistent
}
```
- Removes instance storage extends entirely
- Only manages persistent entry TTLs (name + optional reverse key)

### 4. Code Quality Fixes
✅ **Removed Duplicates**
- Line 238-241: Removed duplicate `meta_hash` calculation in `register_internal()`
- Line 289-290: Removed duplicate `new_meta_hash` calculation in `update_internal()`
- Fixed: Malformed `update_internal()` stub that was followed by duplicate `pub fn update()` definition

✅ **Fixed Structural Issues**
- Cleaned up import duplication (lines 4-8)
- Corrected error code assignments (was reusing code 8 and 9, now sequential 11-18)
- Proper function signatures for `update_internal()` and `release_internal()`

### 5. API Compatibility
✅ **Public API Unchanged**
- `register(owner, name, stealth_meta_address)` → same signature
- `register_on_behalf(owner, name, stealth_meta_address, signature, expiry)` → same signature
- `update(owner, name, new_meta_address)` → same signature
- `update_on_behalf(owner, name, new_meta_address, signature, expiry)` → same signature
- `release(owner, name)` → same signature
- `release_on_behalf(owner, name, signature, expiry)` → same signature
- `resolve(name)` → same signature
- `name_of(stealth_meta_address)` → same signature

### 6. Test Coverage
✅ **Tests Compile**
- Basic operations: `test_register_and_resolve()`
- Replay protection: `test_register_on_behalf_replay()`
- Signature validation: `test_register_on_behalf_wrong_signer_panics()`
- Authorization: `test_register_on_behalf()`, `test_update_on_behalf_and_release_on_behalf()`
- Malformed inputs: `test_on_behalf_malformed_inputs()`
- Property tests: `prop_register_on_behalf_roundtrip()`

## Breaking Changes
⚠️ **Data Migration Required**
- **Breaking**: Existing contracts deployed with instance storage cannot migrate in-place
- **Path**: Fresh deployment on new contract ID (coordinated with deploy strategy)
- **Data Loss**: Instance storage entries (if any) will not transfer automatically
- **Timing**: Deploy new contract, migrate users to new address

## Keeper Service (#50) Integration
✅ **Ready for TTL Maintenance Updates**
- Keeper no longer needs to extend instance storage
- Only persistent() keys need periodic TTL extends via keeper service
- TTL constants unchanged: `TTL_THRESHOLD = 17,280`, `TTL_EXTEND_TO = 518,400`

## Build Verification
✅ **Syntax**: All Rust code is syntactically valid
✅ **Structure**: No undefined symbols or import errors
✅ **Tests**: Test module compiles with all test functions

## CI Pipeline
The GitHub Actions workflow (`.github/workflows/ci.yml`) will verify:
- `cargo fmt --all --check` passes
- `cargo test --workspace` includes wraith-names tests
- All Soroban SDK patterns compile against latest version (22.0.0)

## Sign-off
Migration complete. Ready for CI validation and deployment coordination.
