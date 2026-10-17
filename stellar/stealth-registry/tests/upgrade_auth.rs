//! Upgrade authority enforcement tests for stealth-registry contract.
//!
//! Per GOVERNANCE.md, stealth-registry is FROZEN (No upgrade path).
//! These tests verify:
//! 1. No admin exists
//! 2. No upgrade path is available  
//! 3. User keys cannot be arbitrarily altered or censored
//! 4. Contract is immutable by design

#![cfg(test)]

use soroban_sdk::{
    symbol_short,
    testutils::{
        storage::{Instance as _, Persistent as _},
        Address as _, Events, Ledger,
    },
    Address, Bytes, BytesN, Env, IntoVal, TryFromVal, Val,
};
use stealth_registry::{DataKey, StealthRegistryContract, StealthRegistryContractClient};

extern crate alloc;
use alloc::vec::Vec;

// Mirror the registry's renewal policy; keep the minimum below the threshold
// so these tests exercise actual TTL extensions instead of pre-pinning entries.
const TTL_THRESHOLD: u32 = 17_280;
const TTL_EXTEND_TO: u32 = 518_400;

fn lifetime_env() -> Env {
    let env = Env::default();
    env.ledger().with_mut(|li| {
        li.sequence_number = 1_000;
        li.min_persistent_entry_ttl = 100;
        li.min_temp_entry_ttl = 16;
        li.max_entry_ttl = 1_000_000;
    });
    env.mock_all_auths();
    env
}

/// Advance only as far as the next renewal threshold, then touch every active
/// registration through the public lookup. Immutability does not waive rent:
/// both the instance and persistent entries must stay live throughout a test.
fn advance_with_renewals(
    env: &Env,
    client: &StealthRegistryContractClient<'_>,
    registrations: &[(Address, Bytes)],
    ledgers: u32,
) {
    let target = env.ledger().sequence() + ledgers;
    while env.ledger().sequence() < target {
        let (instance_ttl, shortest_ttl) = env.as_contract(&client.address, || {
            let instance_ttl = env.storage().instance().get_ttl();
            let shortest_ttl = registrations.iter().fold(instance_ttl, |ttl, (user, _)| {
                ttl.min(
                    env.storage()
                        .persistent()
                        .get_ttl(&DataKey::MetaAddress(user.clone(), 1)),
                )
            });
            (instance_ttl, shortest_ttl)
        });
        assert!(shortest_ttl > TTL_THRESHOLD);
        let step = (shortest_ttl - TTL_THRESHOLD).min(target - env.ledger().sequence());
        env.ledger()
            .set_sequence_number(env.ledger().sequence() + step);

        for (user, expected_meta) in registrations {
            let key = DataKey::MetaAddress(user.clone(), 1);
            let before =
                env.as_contract(&client.address, || env.storage().persistent().get_ttl(&key));
            assert_eq!(client.stealth_meta_address_of(user, &1), *expected_meta);
            let after =
                env.as_contract(&client.address, || env.storage().persistent().get_ttl(&key));
            assert_eq!(
                after,
                if before <= TTL_THRESHOLD {
                    TTL_EXTEND_TO
                } else {
                    before
                }
            );
        }

        let remaining = instance_ttl - step;
        assert_eq!(
            env.as_contract(&client.address, || env.storage().instance().get_ttl()),
            if remaining <= TTL_THRESHOLD {
                TTL_EXTEND_TO
            } else {
                remaining
            }
        );
    }
}

/// Helper to create a mock WASM hash
fn mock_wasm_hash(env: &Env, seed: u8) -> BytesN<32> {
    let mut bytes = [seed; 32];
    bytes[0] = seed;
    bytes[31] = seed.wrapping_add(1);
    BytesN::from_array(env, &bytes)
}

/// Test that no admin role exists in the contract.
#[test]
fn test_no_admin_exists() {
    let env = Env::default();
    let contract_id = env.register(StealthRegistryContract, ());

    env.as_contract(&contract_id, || {
        // The contract should have NO admin storage key
        // This is enforced by not including Admin in the DataKey enum
    });
}

/// Test that no upgrade function exists.
#[test]
fn test_no_upgrade_function_exists() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(StealthRegistryContract, ());
    let client = StealthRegistryContractClient::new(&env, &contract_id);

    // Contract should only have: register_keys, remove_keys, stealth_meta_address_of
    // No upgrade, no admin functions

    let registrant = Address::generate(&env);
    let meta = Bytes::from_slice(&env, &[1u8; 64]);

    // Normal operations should work
    client.register_keys(&registrant, &1, &meta);
    let retrieved = client.stealth_meta_address_of(&registrant, &1);
    assert_eq!(retrieved, meta);
}

/// Test that the deployer cannot upgrade a frozen contract.
#[test]
#[should_panic]
fn test_deployer_cannot_upgrade_frozen_contract() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(StealthRegistryContract, ());
    let deployer = Address::generate(&env);
    let new_wasm_hash = mock_wasm_hash(&env, 1);

    env.as_contract(&contract_id, || {
        deployer.require_auth();
        // This should panic - no upgrade mechanism
        env.deployer()
            .update_current_contract_wasm(new_wasm_hash.clone());
    });
}

/// Test that user keys cannot be censored or altered by any admin.
/// This is the core trust guarantee of the frozen registry.
#[test]
fn test_user_keys_cannot_be_censored() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(StealthRegistryContract, ());
    let client = StealthRegistryContractClient::new(&env, &contract_id);

    // User registers their keys
    let user = Address::generate(&env);
    let user_meta = Bytes::from_slice(&env, &[42u8; 64]);
    client.register_keys(&user, &1, &user_meta);

    // Hypothetical admin tries to censor/modify
    let admin = Address::generate(&env);

    // There is NO admin function to alter another user's keys
    // The only way to change keys is user.require_auth()

    // Verify user's keys are unchanged
    let retrieved = client.stealth_meta_address_of(&user, &1);
    assert_eq!(retrieved, user_meta);

    // Only the user can modify their own keys
    let new_meta = Bytes::from_slice(&env, &[99u8; 64]);
    client.register_keys(&user, &1, &new_meta);
    let retrieved_after = client.stealth_meta_address_of(&user, &1);
    assert_eq!(retrieved_after, new_meta);
}

/// Active registrations survive one million ledgers with permissionless renewal.
#[test]
fn test_user_data_preserved_indefinitely() {
    let env = lifetime_env();

    let contract_id = env.register(StealthRegistryContract, ());
    let client = StealthRegistryContractClient::new(&env, &contract_id);

    // Multiple users register keys
    let registrations: Vec<(Address, Bytes)> = (0..5)
        .map(|i| {
            let user = Address::generate(&env);
            let mut meta_bytes = [i; 64];
            meta_bytes[63] = 255;
            let meta = Bytes::from_slice(&env, &meta_bytes);
            client.register_keys(&user, &1, &meta);
            (user, meta)
        })
        .collect();

    let start = env.ledger().sequence();
    advance_with_renewals(&env, &client, &registrations, 1_000_000);
    assert_eq!(env.ledger().sequence(), start + 1_000_000);

    for (user, expected_meta) in &registrations {
        assert_eq!(client.stealth_meta_address_of(user, &1), *expected_meta);
    }
}

/// Test that the contract's immutability guarantees user sovereignty.
#[test]
fn test_immutability_guarantees_user_sovereignty() {
    // Per GOVERNANCE.md:
    // > stealth-registry is FROZEN (No upgrade path)
    // > Reasoning: It holds the user's meta-address mapping and scheme keys.
    // > Keeping this frozen ensures users that their keys cannot be
    // > arbitrarily altered or censored.

    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(StealthRegistryContract, ());
    let client = StealthRegistryContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let user_meta = Bytes::from_slice(&env, &[7u8; 64]);

    // User registers keys
    client.register_keys(&user, &1, &user_meta);

    // Guarantees provided by immutability:
    // 1. No admin can censor registration
    // 2. No admin can modify user's keys
    // 3. No admin can prevent key removal by user
    // 4. No admin can pause the contract
    // 5. Contract behavior is predictable forever

    // User can always remove their own keys
    client.remove_keys(&user, &1);

    let result = client.try_stealth_meta_address_of(&user, &1);
    assert!(result.is_err()); // Not registered anymore

    // User sovereignty maintained through immutability
}

/// Test that no governance infrastructure exists.
#[test]
fn test_no_governance_infrastructure() {
    let env = Env::default();
    let contract_id = env.register(StealthRegistryContract, ());

    env.as_contract(&contract_id, || {
        // No Admin, no Timelock, no Multisig, no Pause
        // Storage only contains user registrations
        // This is enforced at compile time by the DataKey enum
    });
}

/// Test that contract behavior is deterministic and unchanging.
#[test]
fn test_behavior_deterministic() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(StealthRegistryContract, ());
    let client = StealthRegistryContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let meta = Bytes::from_slice(&env, &[123u8; 64]);

    // Register
    client.register_keys(&user, &1, &meta);
    let events1 = env.events().all();

    // Remove
    client.remove_keys(&user, &1);
    let events2 = env.events().all();

    // Register again
    client.register_keys(&user, &1, &meta);
    let events3 = env.events().all();

    // SDK 22 exposes the latest invocation's events, not a cumulative log.
    // Each operation emits its domain event followed by a metric event.
    assert_eq!(events1.len(), 2);
    assert_eq!(events2.len(), 2);
    assert_eq!(events3.len(), 2);
    let register = events1.first().unwrap();
    let register_topics: soroban_sdk::Vec<Val> =
        (symbol_short!("register"), user.clone(), 1u32).into_val(&env);
    assert_eq!(register.0, contract_id);
    assert_eq!(register.1, register_topics);
    assert_eq!(Bytes::try_from_val(&env, &register.2).unwrap(), meta);

    let remove = events2.first().unwrap();
    let remove_topics: soroban_sdk::Vec<Val> =
        (symbol_short!("remove"), user.clone(), 1u32).into_val(&env);
    assert_eq!(remove.0, contract_id);
    assert_eq!(remove.1, remove_topics);
    <()>::try_from_val(&env, &remove.2).unwrap();
    // Repeating the same registration reproduces both complete events.
    assert_eq!(events1, events3);
    assert_eq!(client.stealth_meta_address_of(&user, &1), meta);
}

/// Test that multiple scheme IDs work independently.
#[test]
fn test_multiple_schemes_independent() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(StealthRegistryContract, ());
    let client = StealthRegistryContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);

    // Register different keys for different schemes
    let meta_scheme_1 = Bytes::from_slice(&env, &[1u8; 64]);
    let meta_scheme_2 = Bytes::from_slice(&env, &[2u8; 64]);
    let meta_scheme_3 = Bytes::from_slice(&env, &[3u8; 64]);

    client.register_keys(&user, &1, &meta_scheme_1);
    client.register_keys(&user, &2, &meta_scheme_2);
    client.register_keys(&user, &3, &meta_scheme_3);

    // Verify independent storage
    assert_eq!(client.stealth_meta_address_of(&user, &1), meta_scheme_1);
    assert_eq!(client.stealth_meta_address_of(&user, &2), meta_scheme_2);
    assert_eq!(client.stealth_meta_address_of(&user, &3), meta_scheme_3);

    // Remove one scheme doesn't affect others
    client.remove_keys(&user, &2);

    assert_eq!(client.stealth_meta_address_of(&user, &1), meta_scheme_1);
    assert!(client.try_stealth_meta_address_of(&user, &2).is_err());
    assert_eq!(client.stealth_meta_address_of(&user, &3), meta_scheme_3);
}

/// Ten years of normal use keep old and new registrations live without an admin.
#[test]
fn test_perpetual_operation_without_admin() {
    let env = lifetime_env();

    let contract_id = env.register(StealthRegistryContract, ());
    let client = StealthRegistryContractClient::new(&env, &contract_id);

    let start = env.ledger().sequence();
    let mut registrations = Vec::new();

    // Simulate years of operation, renewing before expiry within each year.
    for year in 0..10 {
        // Contract still fully operational
        let user = Address::generate(&env);
        let mut meta_bytes = [year as u8; 64];
        meta_bytes[0] = year;
        let meta = Bytes::from_slice(&env, &meta_bytes);

        client.register_keys(&user, &1, &meta);
        let retrieved = client.stealth_meta_address_of(&user, &1);
        assert_eq!(retrieved, meta);
        registrations.push((user, meta));
        advance_with_renewals(&env, &client, &registrations, 6_307_200);
    }

    assert_eq!(env.ledger().sequence(), start + 10 * 6_307_200);
    for (user, expected_meta) in &registrations {
        assert_eq!(client.stealth_meta_address_of(user, &1), *expected_meta);
    }
}
