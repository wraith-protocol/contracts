use proptest::prelude::*;
use soroban_sdk::testutils::{Address as _, EnvTestConfig, Events};
use soroban_sdk::{Address, Bytes, Env, String};
use wraith_names::{NamesError, WraithNamesContract, WraithNamesContractClient};

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

fn name(env: &Env, value: &str) -> String {
    String::from_str(env, value)
}

fn valid_name_strategy() -> impl Strategy<Value = std::string::String> {
    "[a-z0-9]{3,32}"
}

fn invalid_name_strategy() -> impl Strategy<Value = std::string::String> {
    prop_oneof![
        "[a-z0-9]{0,2}",
        "[a-z0-9]{33,40}",
        "[A-Z][a-z0-9]{2,10}",
        "[a-z0-9]{1,8}[-_][a-z0-9]{1,8}",
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: cases(), .. ProptestConfig::default() })]
    #[test]
    fn register_then_resolve_round_trips_valid_names(name_value in valid_name_strategy(), meta in any::<[u8; 64]>()) {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let name = name(&env, &name_value);
        let meta = bytes(&env, &meta);

        client.register(&owner, &name, &meta);

        prop_assert_eq!(client.resolve(&name), meta);
    }

    #[test]
    fn name_of_round_trips_the_registered_name(name_value in valid_name_strategy(), meta in any::<[u8; 64]>()) {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let name = name(&env, &name_value);
        let meta = bytes(&env, &meta);

        client.register(&owner, &name, &meta);

        prop_assert_eq!(client.name_of(&meta), name);
    }

    #[test]
    fn update_replaces_meta_address_for_owner(name_value in valid_name_strategy(), first in any::<[u8; 64]>(), second in any::<[u8; 64]>()) {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let name = name(&env, &name_value);
        let first = bytes(&env, &first);
        let second = bytes(&env, &second);

        client.register(&owner, &name, &first);
        client.update(&owner, &name, &second);

        prop_assert_eq!(client.resolve(&name), second);
        prop_assert_eq!(client.try_name_of(&first), Err(Ok(NamesError::NameNotFound)));
    }

    #[test]
    fn invalid_names_are_rejected(name_value in invalid_name_strategy(), meta in any::<[u8; 64]>()) {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let name = name(&env, &name_value);
        let meta = bytes(&env, &meta);

        let result = client.try_register(&owner, &name, &meta);

        prop_assert!(result.is_err());
    }

    #[test]
    fn non_64_byte_meta_addresses_are_rejected(name_value in valid_name_strategy(), data in prop::collection::vec(any::<u8>(), 0..96)) {
        prop_assume!(data.len() != 64);

        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let name = name(&env, &name_value);

        let result = client.try_register(&owner, &name, &bytes(&env, &data));

        prop_assert_eq!(result, Err(Ok(NamesError::InvalidMetaAddress)));
    }

    #[test]
    fn release_removes_forward_and_reverse_lookup(name_value in valid_name_strategy(), meta in any::<[u8; 64]>()) {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let name = name(&env, &name_value);
        let meta = bytes(&env, &meta);

        client.register(&owner, &name, &meta);
        client.release(&owner, &name);

        prop_assert_eq!(client.try_resolve(&name), Err(Ok(NamesError::NameNotFound)));
        prop_assert_eq!(client.try_name_of(&meta), Err(Ok(NamesError::NameNotFound)));
    }

    #[test]
    fn successful_register_emits_one_event(name_value in valid_name_strategy(), meta in any::<[u8; 64]>()) {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let name = name(&env, &name_value);
        let meta = bytes(&env, &meta);

        client.register(&owner, &name, &meta);

        prop_assert_eq!(env.events().all().len(), 1);
    }
}

#[test]
fn default_property_case_count_is_at_least_1024() {
    assert!(cases() >= 1024);
}
