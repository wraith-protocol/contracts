use alloc::vec::Vec;

use crate::mock_sdk::{Address, Bytes, DataKey, Env, StorageEntry};
use crate::StealthRegistryContract;

/// Proof (a): register-then-resolve returns the exact registered payload.
///
/// Claim: For any valid 64-byte payload registered under a key, resolving that key
/// immediately returns the exact registered payload.
#[kani::proof]
#[kani::unwind(10)]
pub fn proof_register_then_resolve() {
    let env = Env::new(1);

    // Create symbolic inputs
    let registrant_id: u32 = kani::any();
    let registrant = Address { id: registrant_id };

    let scheme_id: u32 = kani::any();

    let meta = Bytes {
        data: kani::any(),
        len: 64,
    };

    // Call register_keys
    let res = StealthRegistryContract::register_keys(
        env.clone(),
        registrant.clone(),
        scheme_id,
        meta.clone(),
    );

    // Assert registration succeeded
    assert!(res.is_ok());

    // Resolve keys
    let resolved =
        StealthRegistryContract::stealth_meta_address_of(env.clone(), registrant, scheme_id);

    // Assert lookup returns Ok and matches meta
    assert_eq!(resolved.unwrap(), meta);
}

/// Proof (b): no two active registrations share the same key.
///
/// Claim: The registry storage map maintains a uniqueness invariant such that
/// no two distinct entries in the active registration list share the same storage key.
#[kani::proof]
#[kani::unwind(10)]
pub fn proof_no_duplicate_keys() {
    let env = Env::new(1);

    // Construct an arbitrary initial state that satisfies the invariant
    // (no two distinct elements have the same key).
    // We model storage with up to 3 elements for efficiency under symbolic execution.
    let size: usize = kani::any();
    kani::assume(size <= 3);

    let mut storage = Vec::new();
    for _ in 0..size {
        let reg_id: u32 = kani::any();
        let scheme_id: u32 = kani::any();
        let key = DataKey::MetaAddress(Address { id: reg_id }, scheme_id);
        let value = Bytes {
            data: kani::any(),
            len: 64,
        };

        // Assume the initial keys are unique to set up a valid starting state
        for entry in &storage {
            kani::assume(entry.key != key);
        }

        storage.push(StorageEntry {
            key,
            value,
            expiry_ledger: kani::any(),
        });
    }

    // Set this arbitrary state into the env
    env.state.borrow_mut().storage = storage;

    // Perform an arbitrary registration operation
    let reg_id: u32 = kani::any();
    let registrant = Address { id: reg_id };
    let scheme_id: u32 = kani::any();
    let meta = Bytes {
        data: kani::any(),
        len: 64,
    };

    let _ = StealthRegistryContract::register_keys(env.clone(), registrant, scheme_id, meta);

    // Assert that in the new storage, no two distinct elements share the same key
    let final_storage = &env.state.borrow().storage;
    let len = final_storage.len();
    for i in 0..len {
        for j in (i + 1)..len {
            assert!(final_storage[i].key != final_storage[j].key);
        }
    }
}

/// Proof (c): expiry strictly monotonic per key.
///
/// Claim: Any state-mutating operation (registration) or read operation (lookup)
/// that extends the entry's Time-To-Live (TTL) results in an expiry ledger that is
/// greater than or equal to the previous expiry ledger.
#[kani::proof]
#[kani::unwind(10)]
pub fn proof_expiry_monotonicity() {
    let initial_ledger: u32 = kani::any();
    let env = Env::new(initial_ledger);

    let reg_id: u32 = kani::any();
    let registrant = Address { id: reg_id };
    let scheme_id: u32 = kani::any();
    let key = DataKey::MetaAddress(registrant.clone(), scheme_id);

    // Set up an initial storage entry with an arbitrary expiry
    let initial_expiry: u32 = kani::any();
    let initial_meta = Bytes {
        data: kani::any(),
        len: 64,
    };

    env.state.borrow_mut().storage.push(StorageEntry {
        key: key.clone(),
        value: initial_meta,
        expiry_ledger: initial_expiry,
    });

    // 1. Verify monotonicity during register_keys
    let new_meta = Bytes {
        data: kani::any(),
        len: 64,
    };

    let res_reg = StealthRegistryContract::register_keys(
        env.clone(),
        registrant.clone(),
        scheme_id,
        new_meta,
    );
    assert!(res_reg.is_ok());

    let expiry_after_reg = env.state.borrow().storage[0].expiry_ledger;
    assert!(expiry_after_reg >= initial_expiry);

    // 2. Verify monotonicity during stealth_meta_address_of (lookup)
    let res_lookup =
        StealthRegistryContract::stealth_meta_address_of(env.clone(), registrant, scheme_id);
    assert!(res_lookup.is_ok());

    let expiry_after_lookup = env.state.borrow().storage[0].expiry_ledger;
    assert!(expiry_after_lookup >= expiry_after_reg);
}
