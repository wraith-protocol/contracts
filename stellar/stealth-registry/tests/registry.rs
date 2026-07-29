#![cfg(test)]

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events},
    vec, Address, Bytes, Env, IntoVal, Val,
};
use stealth_registry::{RegistryError, StealthRegistryContract, StealthRegistryContractClient};

fn setup() -> (Env, StealthRegistryContractClient<'static>) {
    let env = Env::default();
    let contract_id = env.register(StealthRegistryContract, ());
    let client = StealthRegistryContractClient::new(&env, &contract_id);
    (env, client)
}

#[test]
fn test_register_and_lookup() {
    let (env, client) = setup();
    env.mock_all_auths();

    let registrant = Address::generate(&env);
    let scheme_id: u32 = 1;
    let meta_address = Bytes::from_slice(&env, &[42u8; 64]);

    client.register_keys(&registrant, &scheme_id, &meta_address);

    let events = env.events().all();
    assert!(!events.is_empty());
    // register_keys emits the register event first, then a wraith-metrics event.
    let event = events.first().unwrap();
    let expected_topics: soroban_sdk::Vec<Val> = vec![
        &env,
        symbol_short!("register").into_val(&env),
        registrant.clone().into_val(&env),
        scheme_id.into_val(&env),
    ];
    assert_eq!(event.1, expected_topics);

    let stored = client.stealth_meta_address_of(&registrant, &scheme_id);
    assert_eq!(stored, meta_address);
}

#[test]
fn test_register_rejects_wrong_length() {
    let (env, client) = setup();
    env.mock_all_auths();

    let registrant = Address::generate(&env);
    let scheme_id: u32 = 1;
    let bad_meta = Bytes::from_slice(&env, &[0u8; 32]);

    let result = client.try_register_keys(&registrant, &scheme_id, &bad_meta);
    assert_eq!(result, Err(Ok(RegistryError::InvalidMetaAddressLength)));
}

#[test]
fn test_lookup_not_registered() {
    let (env, client) = setup();

    let registrant = Address::generate(&env);
    let scheme_id: u32 = 1;

    let result = client.try_stealth_meta_address_of(&registrant, &scheme_id);
    assert_eq!(result, Err(Ok(RegistryError::NotRegistered)));
}

#[test]
fn test_update_existing_registration() {
    let (env, client) = setup();
    env.mock_all_auths();

    let registrant = Address::generate(&env);
    let scheme_id: u32 = 1;

    let meta_v1 = Bytes::from_slice(&env, &[1u8; 64]);
    client.register_keys(&registrant, &scheme_id, &meta_v1);
    assert_eq!(
        client.stealth_meta_address_of(&registrant, &scheme_id),
        meta_v1
    );

    let meta_v2 = Bytes::from_slice(&env, &[2u8; 64]);
    client.register_keys(&registrant, &scheme_id, &meta_v2);
    assert_eq!(
        client.stealth_meta_address_of(&registrant, &scheme_id),
        meta_v2
    );
}

#[test]
fn test_remove_keys() {
    let (env, client) = setup();
    env.mock_all_auths();

    let registrant = Address::generate(&env);
    let scheme_id: u32 = 1;
    let meta_address = Bytes::from_slice(&env, &[42u8; 64]);

    client.register_keys(&registrant, &scheme_id, &meta_address);
    client.remove_keys(&registrant, &scheme_id);

    let result = client.try_stealth_meta_address_of(&registrant, &scheme_id);
    assert_eq!(result, Err(Ok(RegistryError::NotRegistered)));
}

#[test]
fn test_remove_not_registered() {
    let (env, client) = setup();
    env.mock_all_auths();

    let registrant = Address::generate(&env);
    let scheme_id: u32 = 1;

    let result = client.try_remove_keys(&registrant, &scheme_id);
    assert_eq!(result, Err(Ok(RegistryError::NotRegistered)));
}
