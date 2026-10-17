use proptest::prelude::*;
use soroban_sdk::testutils::{Address as _, EnvTestConfig, Events};
use soroban_sdk::{symbol_short, vec, Address, Bytes, BytesN, Env, IntoVal, TryFromVal, Val};
use stealth_announcer::{
    view_tag_bucket, StealthAnnouncerContract, StealthAnnouncerContractClient,
    METADATA_KIND_VIEW_TAG, STELLAR_V2_SCHEME_ID,
};

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

/// Fill bytes whose `[fill; 32]` encoding is a canonical, on-curve,
/// non-small-order ed25519 public key. The announcer rejects malformed
/// encodings, so the payload properties draw from this pool instead of
/// arbitrary bytes.
const VALID_EPK_FILLS: [u8; 16] = [
    0x01, 0x03, 0x06, 0x09, 0x0a, 0x0b, 0x0c, 0x10, 0x11, 0x13, 0x16, 0x17, 0x19, 0x1a, 0x1d, 0x20,
];

fn valid_epk() -> impl Strategy<Value = [u8; 32]> {
    (0..VALID_EPK_FILLS.len()).prop_map(|index| [VALID_EPK_FILLS[index]; 32])
}

proptest! {
    #![proptest_config(ProptestConfig { cases: cases(), .. ProptestConfig::default() })]
    #[test]
    fn announces_once_for_valid_v2_payloads(epk in valid_epk(), metadata in prop::collection::vec(any::<u8>(), 1..128)) {
        let env = env();
        let contract_id = env.register(StealthAnnouncerContract, ());
        let client = StealthAnnouncerContractClient::new(&env, &contract_id);
        let stealth_address = Address::generate(&env);

        client.announce(&STELLAR_V2_SCHEME_ID, &stealth_address, &bytes32(&env, &epk), &bytes(&env, &metadata));

        prop_assert_eq!(env.events().all().len(), 1);
    }

    #[test]
    fn v2_topics_include_scheme_view_tag_bucket_and_metadata_kind(epk in valid_epk(), metadata in prop::collection::vec(any::<u8>(), 1..128)) {
        let env = env();
        let contract_id = env.register(StealthAnnouncerContract, ());
        let client = StealthAnnouncerContractClient::new(&env, &contract_id);
        let stealth_address = Address::generate(&env);
        let metadata = bytes(&env, &metadata);
        let bucket = view_tag_bucket(&metadata);

        client.announce(&STELLAR_V2_SCHEME_ID, &stealth_address, &bytes32(&env, &epk), &metadata);
        let event = env.events().all().last().unwrap();

        let expected_topics: soroban_sdk::Vec<Val> = vec![
            &env,
            symbol_short!("announce").into_val(&env),
            STELLAR_V2_SCHEME_ID.into_val(&env),
            bucket.into_val(&env),
            METADATA_KIND_VIEW_TAG.into_val(&env),
        ];
        prop_assert_eq!(event.1, expected_topics);
    }

    #[test]
    fn v2_payload_round_trips_without_caller(epk in valid_epk(), metadata in prop::collection::vec(any::<u8>(), 1..128)) {
        let env = env();
        let contract_id = env.register(StealthAnnouncerContract, ());
        let client = StealthAnnouncerContractClient::new(&env, &contract_id);
        let stealth_address = Address::generate(&env);
        let epk = bytes32(&env, &epk);
        let metadata = bytes(&env, &metadata);

        client.announce(&STELLAR_V2_SCHEME_ID, &stealth_address, &epk, &metadata);
        let event = env.events().all().last().unwrap();

        let actual_value: (Address, BytesN<32>, Bytes) =
            <(Address, BytesN<32>, Bytes)>::try_from_val(&env, &event.2).unwrap();
        prop_assert_eq!(actual_value, (stealth_address, epk, metadata));
    }

    #[test]
    fn repeated_v2_announcements_publish_latest_view_tag_bucket(epk in valid_epk(), first_view_tag in any::<u8>(), second_view_tag in any::<u8>()) {
        let env = env();
        let contract_id = env.register(StealthAnnouncerContract, ());
        let client = StealthAnnouncerContractClient::new(&env, &contract_id);
        let stealth_address = Address::generate(&env);
        let epk = bytes32(&env, &epk);
        let first_metadata = bytes(&env, &[first_view_tag]);
        let second_metadata = bytes(&env, &[second_view_tag]);

        client.announce(&STELLAR_V2_SCHEME_ID, &stealth_address, &epk, &first_metadata);
        client.announce(&STELLAR_V2_SCHEME_ID, &stealth_address, &epk, &second_metadata);

        let event = env.events().all().last().unwrap();
        let expected_topics: soroban_sdk::Vec<Val> = vec![
            &env,
            symbol_short!("announce").into_val(&env),
            STELLAR_V2_SCHEME_ID.into_val(&env),
            (second_view_tag as u32).into_val(&env),
            METADATA_KIND_VIEW_TAG.into_val(&env),
        ];
        prop_assert_eq!(event.1, expected_topics);
    }
}

#[test]
#[should_panic]
fn zero_length_metadata_is_rejected() {
    let env = env();
    let contract_id = env.register(StealthAnnouncerContract, ());
    let client = StealthAnnouncerContractClient::new(&env, &contract_id);
    let stealth_address = Address::generate(&env);

    client.announce(
        &STELLAR_V2_SCHEME_ID,
        &stealth_address,
        &bytes32(&env, &[1u8; 32]),
        &Bytes::new(&env),
    );
}

#[test]
#[should_panic]
fn non_v2_scheme_id_is_rejected() {
    let env = env();
    let contract_id = env.register(StealthAnnouncerContract, ());
    let client = StealthAnnouncerContractClient::new(&env, &contract_id);
    let stealth_address = Address::generate(&env);
    let metadata = bytes(&env, &[7]);

    client.announce(
        &1u32,
        &stealth_address,
        &bytes32(&env, &[1u8; 32]),
        &metadata,
    );
}

#[test]
fn default_property_case_count_is_at_least_1024() {
    assert!(cases() >= 1024);
}

/// The all-zero key is a well-formed encoding of an order-4 point, so it needs
/// an explicit rejection.
#[test]
#[should_panic(expected = "invalid ed25519 ephemeral public key")]
fn zero_ephemeral_pub_key_is_rejected() {
    let env = env();
    let contract_id = env.register(StealthAnnouncerContract, ());
    let client = StealthAnnouncerContractClient::new(&env, &contract_id);

    client.announce(
        &STELLAR_V2_SCHEME_ID,
        &Address::generate(&env),
        &bytes32(&env, &[0u8; 32]),
        &bytes(&env, &[0u8]),
    );
}

/// `0xff * 32` masks to `2^255 - 1 >= p`, a non-canonical encoding.
#[test]
#[should_panic(expected = "invalid ed25519 ephemeral public key")]
fn non_canonical_ephemeral_pub_key_is_rejected() {
    let env = env();
    let contract_id = env.register(StealthAnnouncerContract, ());
    let client = StealthAnnouncerContractClient::new(&env, &contract_id);

    client.announce(
        &STELLAR_V2_SCHEME_ID,
        &Address::generate(&env),
        &bytes32(&env, &[0xffu8; 32]),
        &bytes(&env, &[0u8]),
    );
}

/// `0x02 * 32` does not decompress to a curve point.
#[test]
#[should_panic(expected = "invalid ed25519 ephemeral public key")]
fn off_curve_ephemeral_pub_key_is_rejected() {
    let env = env();
    let contract_id = env.register(StealthAnnouncerContract, ());
    let client = StealthAnnouncerContractClient::new(&env, &contract_id);

    client.announce(
        &STELLAR_V2_SCHEME_ID,
        &Address::generate(&env),
        &bytes32(&env, &[0x02u8; 32]),
        &bytes(&env, &[0u8]),
    );
}
