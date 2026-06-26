use proptest::prelude::*;
use soroban_sdk::testutils::{Address as _, EnvTestConfig, Events};
use soroban_sdk::{symbol_short, vec, Address, Bytes, Env, IntoVal, TryFromVal, Val};
use stealth_registry::{RegistryError, StealthRegistryContract, StealthRegistryContractClient};

fn cases() -> u32 {
    std::env::var("WRAITH_PROPTEST_CASES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1024)
}

fn bytes(env: &Env, data: &[u8]) -> Bytes {
    Bytes::from_slice(env, data)
}

fn env() -> Env {
    Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    })
}

fn meta_address(env: &Env, data: [u8; 64]) -> Bytes {
    bytes(env, &data)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: cases(), .. ProptestConfig::default() })]
    #[test]
    fn register_then_lookup_round_trips(scheme_id in any::<u32>(), meta in any::<[u8; 64]>()) {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(StealthRegistryContract, ());
        let client = StealthRegistryContractClient::new(&env, &contract_id);
        let registrant = Address::generate(&env);
        let meta = meta_address(&env, meta);

        client.register_keys(&registrant, &scheme_id, &meta);

        prop_assert_eq!(client.stealth_meta_address_of(&registrant, &scheme_id), meta);
    }

    #[test]
    fn updating_same_key_replaces_previous_value(scheme_id in any::<u32>(), first in any::<[u8; 64]>(), second in any::<[u8; 64]>()) {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(StealthRegistryContract, ());
        let client = StealthRegistryContractClient::new(&env, &contract_id);
        let registrant = Address::generate(&env);
        let first = meta_address(&env, first);
        let second = meta_address(&env, second);

        client.register_keys(&registrant, &scheme_id, &first);
        client.register_keys(&registrant, &scheme_id, &second);

        prop_assert_eq!(client.stealth_meta_address_of(&registrant, &scheme_id), second);
    }

    #[test]
    fn scheme_ids_are_independent(first_scheme in any::<u32>(), second_scheme in any::<u32>(), first in any::<[u8; 64]>(), second in any::<[u8; 64]>()) {
        prop_assume!(first_scheme != second_scheme);

        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(StealthRegistryContract, ());
        let client = StealthRegistryContractClient::new(&env, &contract_id);
        let registrant = Address::generate(&env);
        let first = meta_address(&env, first);
        let second = meta_address(&env, second);

        client.register_keys(&registrant, &first_scheme, &first);
        client.register_keys(&registrant, &second_scheme, &second);

        prop_assert_eq!(client.stealth_meta_address_of(&registrant, &first_scheme), first);
        prop_assert_eq!(client.stealth_meta_address_of(&registrant, &second_scheme), second);
    }

    #[test]
    fn rejects_any_non_64_byte_meta_address(scheme_id in any::<u32>(), data in prop::collection::vec(any::<u8>(), 0..96)) {
        prop_assume!(data.len() != 64);

        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(StealthRegistryContract, ());
        let client = StealthRegistryContractClient::new(&env, &contract_id);
        let registrant = Address::generate(&env);

        let result = client.try_register_keys(&registrant, &scheme_id, &bytes(&env, &data));

        prop_assert_eq!(result, Err(Ok(RegistryError::InvalidMetaAddressLength)));
    }

    #[test]
    fn successful_register_emits_one_verbatim_event(scheme_id in any::<u32>(), meta in any::<[u8; 64]>()) {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(StealthRegistryContract, ());
        let client = StealthRegistryContractClient::new(&env, &contract_id);
        let registrant = Address::generate(&env);
        let meta = meta_address(&env, meta);

        client.register_keys(&registrant, &scheme_id, &meta);

        let events = env.events().all();
        prop_assert_eq!(events.len(), 1);
        let event = events.last().unwrap();
        let expected_topics: soroban_sdk::Vec<Val> = vec![
            &env,
            symbol_short!("register").into_val(&env),
            registrant.into_val(&env),
            scheme_id.into_val(&env),
        ];
        prop_assert_eq!(event.1, expected_topics);
        let actual_value = Bytes::try_from_val(&env, &event.2).unwrap();
        prop_assert_eq!(actual_value, meta);
    }
}

#[test]
fn default_property_case_count_is_at_least_1024() {
    assert!(cases() >= 1024);
}
