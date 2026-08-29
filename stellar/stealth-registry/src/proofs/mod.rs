use alloc::vec::Vec;

use crate::mock_sdk::{Address, Bytes, DataKey, Env, StorageEntry};
use crate::StealthRegistryContract;

macro_rules! assert_bytes_eq_64 {
    ($left:expr, $right:expr) => {{
        assert_eq!($left[0], $right[0]);
        assert_eq!($left[1], $right[1]);
        assert_eq!($left[2], $right[2]);
        assert_eq!($left[3], $right[3]);
        assert_eq!($left[4], $right[4]);
        assert_eq!($left[5], $right[5]);
        assert_eq!($left[6], $right[6]);
        assert_eq!($left[7], $right[7]);
        assert_eq!($left[8], $right[8]);
        assert_eq!($left[9], $right[9]);
        assert_eq!($left[10], $right[10]);
        assert_eq!($left[11], $right[11]);
        assert_eq!($left[12], $right[12]);
        assert_eq!($left[13], $right[13]);
        assert_eq!($left[14], $right[14]);
        assert_eq!($left[15], $right[15]);
        assert_eq!($left[16], $right[16]);
        assert_eq!($left[17], $right[17]);
        assert_eq!($left[18], $right[18]);
        assert_eq!($left[19], $right[19]);
        assert_eq!($left[20], $right[20]);
        assert_eq!($left[21], $right[21]);
        assert_eq!($left[22], $right[22]);
        assert_eq!($left[23], $right[23]);
        assert_eq!($left[24], $right[24]);
        assert_eq!($left[25], $right[25]);
        assert_eq!($left[26], $right[26]);
        assert_eq!($left[27], $right[27]);
        assert_eq!($left[28], $right[28]);
        assert_eq!($left[29], $right[29]);
        assert_eq!($left[30], $right[30]);
        assert_eq!($left[31], $right[31]);
        assert_eq!($left[32], $right[32]);
        assert_eq!($left[33], $right[33]);
        assert_eq!($left[34], $right[34]);
        assert_eq!($left[35], $right[35]);
        assert_eq!($left[36], $right[36]);
        assert_eq!($left[37], $right[37]);
        assert_eq!($left[38], $right[38]);
        assert_eq!($left[39], $right[39]);
        assert_eq!($left[40], $right[40]);
        assert_eq!($left[41], $right[41]);
        assert_eq!($left[42], $right[42]);
        assert_eq!($left[43], $right[43]);
        assert_eq!($left[44], $right[44]);
        assert_eq!($left[45], $right[45]);
        assert_eq!($left[46], $right[46]);
        assert_eq!($left[47], $right[47]);
        assert_eq!($left[48], $right[48]);
        assert_eq!($left[49], $right[49]);
        assert_eq!($left[50], $right[50]);
        assert_eq!($left[51], $right[51]);
        assert_eq!($left[52], $right[52]);
        assert_eq!($left[53], $right[53]);
        assert_eq!($left[54], $right[54]);
        assert_eq!($left[55], $right[55]);
        assert_eq!($left[56], $right[56]);
        assert_eq!($left[57], $right[57]);
        assert_eq!($left[58], $right[58]);
        assert_eq!($left[59], $right[59]);
        assert_eq!($left[60], $right[60]);
        assert_eq!($left[61], $right[61]);
        assert_eq!($left[62], $right[62]);
        assert_eq!($left[63], $right[63]);
    }};
}

/// Proof (a): register-then-resolve returns the exact registered payload.
///
/// Claim: For any valid 64-byte payload registered under a key, resolving that key
/// immediately returns the exact registered payload.
#[kani::proof]
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

    // Assert lookup returns Ok and matches meta without invoking memcmp.
    let resolved = resolved.unwrap();
    assert_eq!(resolved.len(), 64);
    assert_bytes_eq_64!(resolved.data, meta.data);
}

/// Proof (b): no two active registrations share the same key.
///
/// Claim: The registry storage map maintains a uniqueness invariant such that
/// no two distinct entries in the active registration list share the same storage key.
#[kani::proof]
#[kani::unwind(5)]
pub fn proof_no_duplicate_keys() {
    let env = Env::new(1);

    // Construct an arbitrary initial state of 2 distinct entries without dynamic symbolic loops
    let key1 = DataKey::MetaAddress(Address { id: kani::any() }, kani::any());
    let key2 = DataKey::MetaAddress(Address { id: kani::any() }, kani::any());
    kani::assume(key1 != key2);

    let storage: Vec<StorageEntry> = alloc::vec![
        StorageEntry {
            key: key1,
            value: Bytes {
                data: kani::any(),
                len: 64,
            },
            expiry_ledger: kani::any(),
        },
        StorageEntry {
            key: key2,
            value: Bytes {
                data: kani::any(),
                len: 64,
            },
            expiry_ledger: kani::any(),
        },
    ];

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
    if len == 3 {
        assert!(final_storage[0].key != final_storage[1].key);
        assert!(final_storage[1].key != final_storage[2].key);
        assert!(final_storage[0].key != final_storage[2].key);
    } else if len == 2 {
        assert!(final_storage[0].key != final_storage[1].key);
    }
}

/// Proof (c): expiry strictly monotonic per key.
///
/// Claim: Any state-mutating operation (registration) or read operation (lookup)
/// that extends the entry's Time-To-Live (TTL) results in an expiry ledger that is
/// greater than or equal to the previous expiry ledger.
#[kani::proof]
#[kani::unwind(5)]
pub fn proof_expiry_monotonicity() {
    let initial_ledger: u32 = kani::any();
    kani::assume(initial_ledger <= u32::MAX - 518400);
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
