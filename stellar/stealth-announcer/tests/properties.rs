use proptest::prelude::*;
use soroban_sdk::testutils::{Address as _, EnvTestConfig, Events};
use soroban_sdk::{symbol_short, vec, Address, Bytes, BytesN, Env, IntoVal, TryFromVal, Val};
use stealth_announcer::{StealthAnnouncerContract, StealthAnnouncerContractClient};

fn cases() -> u32 {
    std::env::var("WRAITH_PROPTEST_CASES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1024)
}

fn env() -> Env {
    Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    })
}

fn bytes(env: &Env, data: &[u8]) -> Bytes {
    Bytes::from_slice(env, data)
}

fn bytes32(env: &Env, data: &[u8]) -> BytesN<32> {
    let mut fixed = [0u8; 32];
    fixed.copy_from_slice(data);
    BytesN::from_array(env, &fixed)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: cases(), .. ProptestConfig::default() })]
    #[test]
    fn announces_once_for_valid_payloads(scheme_id in any::<u32>(), epk in any::<[u8; 32]>(), metadata in prop::collection::vec(any::<u8>(), 0..128)) {
        let env = env();
        let contract_id = env.register(StealthAnnouncerContract, ());
        let client = StealthAnnouncerContractClient::new(&env, &contract_id);
        let stealth_address = Address::generate(&env);

        client.announce(&scheme_id, &stealth_address, &bytes32(&env, &epk), &bytes(&env, &metadata));

        prop_assert_eq!(env.events().all().len(), 1);
    }

    #[test]
    fn topics_round_trip_verbatim(scheme_id in any::<u32>(), epk in any::<[u8; 32]>(), metadata in prop::collection::vec(any::<u8>(), 0..128)) {
        let env = env();
        let contract_id = env.register(StealthAnnouncerContract, ());
        let client = StealthAnnouncerContractClient::new(&env, &contract_id);
        let stealth_address = Address::generate(&env);

        client.announce(&scheme_id, &stealth_address, &bytes32(&env, &epk), &bytes(&env, &metadata));
        let event = env.events().all().last().unwrap();

        let expected_topics: soroban_sdk::Vec<Val> = vec![
            &env,
            symbol_short!("announce").into_val(&env),
            scheme_id.into_val(&env),
            stealth_address.into_val(&env),
        ];
        prop_assert_eq!(event.1, expected_topics);
    }

    #[test]
    fn payload_round_trips_verbatim(scheme_id in any::<u32>(), epk in any::<[u8; 32]>(), metadata in prop::collection::vec(any::<u8>(), 0..128)) {
        let env = env();
        let contract_id = env.register(StealthAnnouncerContract, ());
        let client = StealthAnnouncerContractClient::new(&env, &contract_id);
        let stealth_address = Address::generate(&env);
        let epk = bytes32(&env, &epk);
        let metadata = bytes(&env, &metadata);

        client.announce(&scheme_id, &stealth_address, &epk, &metadata);
        let event = env.events().all().last().unwrap();

        let actual_value: (Address, BytesN<32>, Bytes) =
            <(Address, BytesN<32>, Bytes)>::try_from_val(&env, &event.2).unwrap();
        prop_assert_eq!(actual_value, (contract_id, epk, metadata));
    }

    #[test]
    fn repeated_announcements_publish_latest_call(scheme_id in any::<u32>(), next_scheme_id in any::<u32>(), epk in any::<[u8; 32]>()) {
        let env = env();
        let contract_id = env.register(StealthAnnouncerContract, ());
        let client = StealthAnnouncerContractClient::new(&env, &contract_id);
        let stealth_address = Address::generate(&env);
        let epk = bytes32(&env, &epk);
        let metadata = bytes(&env, &[7]);

        client.announce(&scheme_id, &stealth_address, &epk, &metadata);
        client.announce(&next_scheme_id, &stealth_address, &epk, &metadata);

        let event = env.events().all().last().unwrap();
        let expected_topics: soroban_sdk::Vec<Val> = vec![
            &env,
            symbol_short!("announce").into_val(&env),
            next_scheme_id.into_val(&env),
            stealth_address.into_val(&env),
        ];
        prop_assert_eq!(event.1, expected_topics);
    }

    #[test]
    fn zero_length_metadata_is_valid(scheme_id in any::<u32>(), epk in any::<[u8; 32]>()) {
        let env = env();
        let contract_id = env.register(StealthAnnouncerContract, ());
        let client = StealthAnnouncerContractClient::new(&env, &contract_id);
        let stealth_address = Address::generate(&env);

        client.announce(&scheme_id, &stealth_address, &bytes32(&env, &epk), &Bytes::new(&env));

        prop_assert_eq!(env.events().all().len(), 1);
    }
}

#[test]
fn default_property_case_count_is_at_least_1024() {
    assert!(cases() >= 1024);
}
