//! Differential test harness: stealth-registry on-chain vs reference impl.
//!
//! Property-based tests generate sequences of operations, execute them
//! against both the on-chain Soroban contract and a pure-Rust reference
//! implementation, then assert identical per-operation results and
//! identical final state.
//!
//! Catches regressions from:
//! - Soroban SDK upgrades that change storage or event semantics
//! - Compiler optimisations that alter contract behaviour
//! - Refactoring that introduces subtle semantic mismatches
//!
//! ## Operation coverage
//!
//! | Operation       | Maps to                    |
//! |-----------------|----------------------------|
//! | Register        | `register_keys`            |
//! | Revoke (Remove) | `remove_keys`              |
//! | Expire          | `AdvanceLedger` (TTL)¹     |
//! | Re-register     | `register_keys` (overwrite)|
//!
//! ¹ TTL expiration is modeled in the reference impl and tested via its
//! unit tests. The on-chain AdvanceLedger is included in the Op enum for
//! completeness but excluded from proptest strategies due to Soroban's
//! TTL invariants in the test environment.
//!
//! ## Case counts
//!
//! The `WRAITH_PROPTEST_CASES` environment variable controls the number of
//! generated test cases. Default: 1024. Nightly CI: 16384.

mod reference;

use proptest::prelude::*;
use reference::{Op, OpResult, ReferenceRegistry, NUM_REGISTRANTS};
use soroban_sdk::{
    testutils::{Address as _, EnvTestConfig, Ledger},
    Address, Bytes, Env,
};
use stealth_registry::{RegistryError, StealthRegistryContract, StealthRegistryContractClient};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Number of proptest cases to run. Controlled by `WRAITH_PROPTEST_CASES`.
fn cases() -> u32 {
    std::env::var("WRAITH_PROPTEST_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1024)
}

/// Create an Env configured like the existing property-based tests.
///
/// Uses `capture_snapshot_at_drop: false` to avoid generating snapshot
/// files during proptest runs (matches `properties.rs`).
fn env() -> Env {
    Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    })
}

// ---------------------------------------------------------------------------
// Core differential test
// ---------------------------------------------------------------------------

/// Run a sequence of operations against both the on-chain contract and the
/// reference implementation, asserting identical behaviour and final state.
fn differential_test(ops: &[Op]) {
    let env = env();
    env.mock_all_auths();

    // Deploy on-chain contract.
    let contract_id = env.register(StealthRegistryContract, ());
    let client = StealthRegistryContractClient::new(&env, &contract_id);

    // Pre-generate registrant addresses (one per simulated index).
    let registrants: Vec<Address> = (0..NUM_REGISTRANTS as u8)
        .map(|_| Address::generate(&env))
        .collect();

    let mut reference = ReferenceRegistry::new();

    for (i, op) in ops.iter().enumerate() {
        let ref_result = reference.apply(op);

        match op {
            // ------ Register -------------------------------------------------
            Op::Register {
                registrant,
                scheme_id,
                meta,
            } => {
                let registrant_addr = &registrants[*registrant as usize];
                let meta_bytes = Bytes::from_slice(&env, meta);

                // On-chain register always succeeds for valid 64-byte values.
                let _ = client.register_keys(registrant_addr, scheme_id, &meta_bytes);

                // Verify reference result.
                match &ref_result {
                    OpResult::Registered(_) => {}
                    other => panic!(
                        "op[{}] {:?}: reference returned {:?}, expected Registered",
                        i, op, other
                    ),
                }
            }

            // ------ Remove ---------------------------------------------------
            Op::Remove {
                registrant,
                scheme_id,
            } => {
                let registrant_addr = &registrants[*registrant as usize];
                let contract_result = client.try_remove_keys(registrant_addr, scheme_id);

                // try_remove_keys returns Result<Result<(), ConversionError>,
                //                            Result<RegistryError, InvokeError>>
                match (&ref_result, &contract_result) {
                    (OpResult::Removed, Ok(Ok(()))) => {
                        // Both succeeded.
                    }
                    (OpResult::Removed, _) => {
                        panic!(
                            "op[{}] {:?}: contract remove failed but reference succeeded\n  result: {:?}",
                            i, op, contract_result
                        );
                    }
                    (OpResult::NotRegistered, Err(Ok(RegistryError::NotRegistered))) => {
                        // Both agree — not registered.
                    }
                    (OpResult::NotRegistered, _) => {
                        panic!(
                            "op[{}] {:?}: contract remove {:?} but reference returned NotRegistered",
                            i, op, contract_result
                        );
                    }
                    (other, _) => panic!(
                        "op[{}] {:?}: reference returned unexpected {:?}",
                        i, op, other
                    ),
                }
            }

            // ------ Lookup ---------------------------------------------------
            Op::Lookup {
                registrant,
                scheme_id,
            } => {
                let registrant_addr = &registrants[*registrant as usize];
                let contract_result =
                    client.try_stealth_meta_address_of(registrant_addr, scheme_id);

                // try_stealth_meta_address_of returns
                //   Result<Result<Bytes, ConversionError>,
                //          Result<RegistryError, InvokeError>>
                match (&ref_result, &contract_result) {
                    (OpResult::LookupValue(expected_meta), Ok(Ok(actual_meta))) => {
                        let expected = Bytes::from_slice(&env, expected_meta);
                        assert_eq!(
                            *actual_meta, expected,
                            "op[{}] {:?}: lookup returned wrong meta",
                            i, op,
                        );
                    }
                    (OpResult::LookupValue(_), Ok(Err(_))) => {
                        panic!(
                            "op[{}] {:?}: contract lookup had conversion error but reference found entry",
                            i, op
                        );
                    }
                    (OpResult::LookupValue(_), Err(_)) => {
                        panic!(
                            "op[{}] {:?}: contract lookup failed but reference found entry\n  result: {:?}",
                            i, op, contract_result
                        );
                    }
                    (OpResult::NotRegistered, Err(Ok(RegistryError::NotRegistered))) => {
                        // Both agree — not registered.
                    }
                    (OpResult::NotRegistered, _) => {
                        panic!(
                            "op[{}] {:?}: contract lookup succeeded ({:?}) but reference returned NotRegistered",
                            i, op, contract_result
                        );
                    }
                    (other, _) => panic!(
                        "op[{}] {:?}: reference returned unexpected {:?}",
                        i, op, other
                    ),
                }

            }

            // ------ AdvanceLedger --------------------------------------------
            Op::AdvanceLedger { count } => {
                let new_seq = env.ledger().sequence().saturating_add(*count);
                env.ledger().with_mut(|li| {
                    li.sequence_number = new_seq;
                });

                match &ref_result {
                    OpResult::LedgerAdvanced => {}
                    other => panic!(
                        "op[{}] {:?}: reference returned unexpected {:?}",
                        i, op, other
                    ),
                }
            }
        }
    }

    // -------------------------------------------------------------------
    // Final state comparison
    // -------------------------------------------------------------------

    // Collect all (registrant, scheme_id) pairs that were touched.
    let mut touched_pairs: Vec<(u8, u32)> = ops
        .iter()
        .filter_map(|op| match op {
            Op::Register {
                registrant,
                scheme_id,
                ..
            }
            | Op::Remove {
                registrant,
                scheme_id,
            }
            | Op::Lookup {
                registrant,
                scheme_id,
            } => Some((*registrant, *scheme_id)),
            Op::AdvanceLedger { .. } => None,
        })
        .collect();

    touched_pairs.sort();
    touched_pairs.dedup();

    for (registrant_idx, scheme_id) in &touched_pairs {
        let registrant_addr = &registrants[*registrant_idx as usize];

        // Use `stealth_meta_address_of` (with TTL extension) for both sides.
        let ref_result = reference.stealth_meta_address_of(*registrant_idx, *scheme_id);
        let contract_result =
            client.try_stealth_meta_address_of(registrant_addr, scheme_id);

        match (contract_result, ref_result) {
            (Ok(Ok(meta)), Some(expected)) => {
                let expected_bytes = Bytes::from_slice(&env, &expected);
                assert_eq!(
                    meta, expected_bytes,
                    "final state mismatch for (reg={}, scheme={})",
                    registrant_idx,
                    scheme_id,
                );
            }
            (Err(Ok(RegistryError::NotRegistered)), None) => {
                // Both agree the entry is absent — OK.
            }
            (Ok(Ok(_)), None) => {
                panic!(
                    "final state: contract returned meta but reference says not registered \
                     for (reg={}, scheme={})",
                    registrant_idx, scheme_id,
                );
            }
            (Err(Ok(RegistryError::NotRegistered)), Some(_)) => {
                panic!(
                    "final state: reference has entry but contract says not registered \
                     for (reg={}, scheme={})",
                    registrant_idx, scheme_id,
                );
            }
            (other_contract, _other_ref) => {
                panic!(
                    "final state: unexpected contract result {:?} vs reference {:?} \
                     for (reg={}, scheme={})",
                    other_contract, ref_result,
                    registrant_idx, scheme_id,
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Proptest strategies
// ---------------------------------------------------------------------------

/// Strategy for generating a single operation.
fn op_strategy() -> impl Strategy<Value = Op> {
    let register = (0u8..NUM_REGISTRANTS as u8, any::<u32>(), any::<[u8; 64]>()).prop_map(
        |(registrant, scheme_id, meta)| Op::Register {
            registrant,
            scheme_id,
            meta,
        },
    );

    let remove =
        (0u8..NUM_REGISTRANTS as u8, any::<u32>()).prop_map(|(registrant, scheme_id)| Op::Remove {
            registrant,
            scheme_id,
        });

    let lookup =
        (0u8..NUM_REGISTRANTS as u8, any::<u32>()).prop_map(|(registrant, scheme_id)| Op::Lookup {
            registrant,
            scheme_id,
        });

    // AdvanceLedger is excluded from the differential proptest because
    // Soroban's test TTL invariants make it impractical to advance the ledger
    // past TTL_EXTEND_TO without also archiving the contract instance.
    // TTL expiration is modelled in the reference impl and tested via its
    // unit tests. See reference::tests::test_expiry_after_advance.
    //
    // When re-enabled, the handler below must be un-commented:
    //
    // Op::AdvanceLedger { count } => {
    //     let new_seq = env.ledger().sequence().saturating_add(*count);
    //     env.ledger().with_mut(|li| { li.sequence_number = new_seq; });
    //     ... verify ref_result ...
    // }

    prop_oneof![
        5 => register,
        2 => remove,
        4 => lookup,
    ]
}

// ---------------------------------------------------------------------------
// Proptest entry points
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: cases(),
        ..ProptestConfig::default()
    })]

    /// Main differential test: random sequence of operations (1–60 ops).
    #[test]
    fn differential_register_revoke_expire_reregister(
        ops in prop::collection::vec(op_strategy(), 1..60)
    ) {
        differential_test(&ops);
    }
}

// ---------------------------------------------------------------------------
// Sanity checks
// ---------------------------------------------------------------------------

#[test]
fn default_differential_case_count_is_at_least_1024() {
    assert!(cases() >= 1024);
}
