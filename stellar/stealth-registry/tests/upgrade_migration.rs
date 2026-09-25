#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Bytes, Env,
};
use stealth_registry::{
    DataKey, RegistryError, StealthRegistryContract, StealthRegistryContractClient,
};

const TTL_THRESHOLD: u32 = 17_280;
const TTL_EXTEND_TO: u32 = 518_400;

fn env_with_ttl() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.min_persistent_entry_ttl = TTL_EXTEND_TO;
        li.max_entry_ttl = TTL_EXTEND_TO * 2;
    });
    env
}

fn meta(env: &Env, seed: u8) -> Bytes {
    Bytes::from_slice(env, &[seed; 64])
}

#[test]
fn migration_v0_snapshot_readable_by_v1_contract() {
    let env = env_with_ttl();
    let contract_id = env.register(StealthRegistryContract, ());

    let user = Address::generate(&env);
    let scheme_id: u32 = 1;
    let meta_bytes = meta(&env, 0xAA);

    env.as_contract(&contract_id, || {
        let key = DataKey::MetaAddress(user.clone(), scheme_id);
        env.storage().persistent().set(&key, &meta_bytes);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
    });

    let client = StealthRegistryContractClient::new(&env, &contract_id);
    let result = client.stealth_meta_address_of(&user, &scheme_id);
    assert_eq!(result, meta_bytes);
}

#[test]
fn migration_v0_snapshot_multiple_users_readable() {
    let env = env_with_ttl();
    let contract_id = env.register(StealthRegistryContract, ());
    let client = StealthRegistryContractClient::new(&env, &contract_id);

    let users: Vec<Address> = (0..5).map(|_| Address::generate(&env)).collect();
    let scheme_id: u32 = 1;

    for (i, user) in users.iter().enumerate() {
        let m = meta(&env, i as u8);
        env.as_contract(&contract_id, || {
            let key = DataKey::MetaAddress(user.clone(), scheme_id);
            env.storage().persistent().set(&key, &m);
            env.storage()
                .persistent()
                .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
        });
    }

    for (i, user) in users.iter().enumerate() {
        let expected = meta(&env, i as u8);
        let found = client.stealth_meta_address_of(user, &scheme_id);
        assert_eq!(found, expected);
    }
}

#[test]
fn migration_post_upgrade_records_survive_ttl_threshold() {
    let env = env_with_ttl();
    let contract_id = env.register(StealthRegistryContract, ());
    let client = StealthRegistryContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let scheme_id: u32 = 1;
    let meta_bytes = meta(&env, 0x55);

    client.register_keys(&user, &scheme_id, &meta_bytes);

    env.ledger().with_mut(|li| {
        li.sequence_number += TTL_THRESHOLD + 1;
    });

    let result = client.stealth_meta_address_of(&user, &scheme_id);
    assert_eq!(result, meta_bytes);
}

#[test]
fn migration_batch_pre_upgrade_records_survive_after_access() {
    let env = env_with_ttl();
    let contract_id = env.register(StealthRegistryContract, ());
    let client = StealthRegistryContractClient::new(&env, &contract_id);

    let users: Vec<Address> = (0..3).map(|_| Address::generate(&env)).collect();
    let scheme_id: u32 = 2;

    for (i, user) in users.iter().enumerate() {
        let m = meta(&env, i as u8 + 1);
        env.as_contract(&contract_id, || {
            let key = DataKey::MetaAddress(user.clone(), scheme_id);
            env.storage().persistent().set(&key, &m);
            env.storage()
                .persistent()
                .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
        });
    }

    env.ledger().with_mut(|li| {
        li.sequence_number += TTL_THRESHOLD + 100;
    });

    for (i, user) in users.iter().enumerate() {
        let expected = meta(&env, i as u8 + 1);
        let found = client.stealth_meta_address_of(user, &scheme_id);
        assert_eq!(found, expected);
    }
}

#[test]
fn migration_new_writes_use_v1_persistent_schema() {
    let env = env_with_ttl();
    let contract_id = env.register(StealthRegistryContract, ());
    let client = StealthRegistryContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let scheme_id: u32 = 1;
    let meta_bytes = meta(&env, 0x77);

    client.register_keys(&user, &scheme_id, &meta_bytes);

    env.as_contract(&contract_id, || {
        let key = DataKey::MetaAddress(user.clone(), scheme_id);
        let stored: Option<Bytes> = env.storage().persistent().get(&key);
        assert_eq!(stored, Some(meta_bytes.clone()));

        let in_instance: Option<Bytes> = env.storage().instance().get(&key);
        assert!(in_instance.is_none());
    });
}

#[test]
fn migration_multi_scheme_writes_are_independent_persistent_keys() {
    let env = env_with_ttl();
    let contract_id = env.register(StealthRegistryContract, ());
    let client = StealthRegistryContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let meta_1 = meta(&env, 0x11);
    let meta_2 = meta(&env, 0x22);
    let meta_3 = meta(&env, 0x33);

    client.register_keys(&user, &1, &meta_1);
    client.register_keys(&user, &2, &meta_2);
    client.register_keys(&user, &3, &meta_3);

    assert_eq!(client.stealth_meta_address_of(&user, &1), meta_1);
    assert_eq!(client.stealth_meta_address_of(&user, &2), meta_2);
    assert_eq!(client.stealth_meta_address_of(&user, &3), meta_3);

    let updated = meta(&env, 0xFF);
    client.register_keys(&user, &2, &updated);
    assert_eq!(client.stealth_meta_address_of(&user, &1), meta_1);
    assert_eq!(client.stealth_meta_address_of(&user, &2), updated);
    assert_eq!(client.stealth_meta_address_of(&user, &3), meta_3);
}

#[test]
fn migration_rollback_impossible_frozen_contract_legacy_tier_invisible() {
    let env = env_with_ttl();
    let contract_id = env.register(StealthRegistryContract, ());
    let client = StealthRegistryContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let scheme_id: u32 = 1;
    let meta_bytes = meta(&env, 0xBB);

    env.as_contract(&contract_id, || {
        let key = DataKey::MetaAddress(user.clone(), scheme_id);
        env.storage().instance().set(&key, &meta_bytes);
    });

    let result = client.try_stealth_meta_address_of(&user, &scheme_id);
    assert_eq!(result, Err(Ok(RegistryError::NotRegistered)));
}

#[test]
fn migration_incompatible_wrong_length_rejected_at_register() {
    let env = env_with_ttl();
    let contract_id = env.register(StealthRegistryContract, ());
    let client = StealthRegistryContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);

    let too_short = Bytes::from_slice(&env, &[0xCC; 63]);
    let result = client.try_register_keys(&user, &1, &too_short);
    assert_eq!(result, Err(Ok(RegistryError::InvalidMetaAddressLength)));

    let too_long = Bytes::from_slice(&env, &[0xDD; 65]);
    let result = client.try_register_keys(&user, &1, &too_long);
    assert_eq!(result, Err(Ok(RegistryError::InvalidMetaAddressLength)));

    let result = client.try_stealth_meta_address_of(&user, &1);
    assert_eq!(result, Err(Ok(RegistryError::NotRegistered)));
}

#[test]
fn migration_incompatible_pre_injected_wrong_length_opaque_to_api() {
    let env = env_with_ttl();
    let contract_id = env.register(StealthRegistryContract, ());
    let client = StealthRegistryContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let scheme_id: u32 = 1;
    let corrupted = Bytes::from_slice(&env, &[0xEE; 32]);

    env.as_contract(&contract_id, || {
        let key = DataKey::MetaAddress(user.clone(), scheme_id);
        env.storage().persistent().set(&key, &corrupted);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_EXTEND_TO);
    });

    let result = client.stealth_meta_address_of(&user, &scheme_id);
    assert_eq!(result, corrupted);
}

#[test]
fn migration_v1_event_schema_matches_documented_layout() {
    use soroban_sdk::testutils::Events;

    let env = env_with_ttl();
    let contract_id = env.register(StealthRegistryContract, ());
    let client = StealthRegistryContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);
    let scheme_id: u32 = 1;
    let meta_bytes = meta(&env, 0x99);

    client.register_keys(&user, &scheme_id, &meta_bytes);
    let events = env.events().all();
    assert!(!events.is_empty());

    let reg_event = events.iter().find(|(cid, topics, _)| {
        *cid == contract_id && {
            use soroban_sdk::Val;
            let t: soroban_sdk::Vec<Val> = topics.clone();
            t.len() >= 3
        }
    });
    assert!(reg_event.is_some());

    client.remove_keys(&user, &scheme_id);
    let events_after_remove = env.events().all();
    assert!(!events_after_remove.is_empty());
}
