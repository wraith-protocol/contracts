//! Conformance fixtures — stealth-announcer event pipeline.
//!
//! Wire shape asserted:
//!   topics: `("announce", scheme_id: u32, view_tag_bucket: u32, metadata_kind: u32)`
//!   data:   `(stealth_address: Address, ephemeral_pub_key: BytesN<32>, metadata: Bytes)`

use soroban_sdk::testutils::{Address as _, Events};
use soroban_sdk::{symbol_short, vec, Address, Bytes, BytesN, Env, FromVal, IntoVal, Val};

use stealth_announcer::{
    view_tag_bucket, StealthAnnouncerContract, StealthAnnouncerContractClient,
    METADATA_KIND_VIEW_TAG, STELLAR_V2_SCHEME_ID,
};

// ── Canonical fixture constants ───────────────────────────────────────────────

const FIXTURE_EPK: [u8; 32] = [
    0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67,
    0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45,
    0x67, 0x89,
];

/// metadata[0] = 0xfe = 254 → view_tag_bucket == 254u32
const FIXTURE_METADATA: [u8; 2] = [0xfe, 0x07];
const FIXTURE_VIEW_TAG_BUCKET: u32 = 0xfe; // 254

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (Env, StealthAnnouncerContractClient<'static>, Address) {
    let env = Env::default();
    let contract_id = env.register(StealthAnnouncerContract, ());
    let client = StealthAnnouncerContractClient::new(&env, &contract_id);
    let stealth_address = Address::generate(&env);
    (env, client, stealth_address)
}

fn epk(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &FIXTURE_EPK)
}

fn metadata(env: &Env) -> Bytes {
    Bytes::from_slice(env, &FIXTURE_METADATA)
}

// ── Fixtures ──────────────────────────────────────────────────────────────────

#[test]
fn announce_emits_exactly_one_event() {
    let (env, client, stealth_address) = setup();
    client.announce(&STELLAR_V2_SCHEME_ID, &stealth_address, &epk(&env), &metadata(&env));
    assert_eq!(env.events().all().len(), 1, "expected exactly one event");
}

#[test]
fn topic0_is_announce_symbol() {
    let (env, client, stealth_address) = setup();
    client.announce(&STELLAR_V2_SCHEME_ID, &stealth_address, &epk(&env), &metadata(&env));

    let event = env.events().all().last().unwrap();
    let got: soroban_sdk::Symbol = FromVal::from_val(&env, &event.1.get(0).unwrap());
    assert_eq!(got, symbol_short!("announce"));
}

#[test]
fn topic1_is_stellar_v2_scheme_id() {
    let (env, client, stealth_address) = setup();
    client.announce(&STELLAR_V2_SCHEME_ID, &stealth_address, &epk(&env), &metadata(&env));

    let event = env.events().all().last().unwrap();
    let scheme_id: u32 = FromVal::from_val(&env, &event.1.get(1).unwrap());
    assert_eq!(scheme_id, STELLAR_V2_SCHEME_ID, "topic[1] must equal STELLAR_V2_SCHEME_ID");
}

#[test]
fn topic2_is_view_tag_bucket_derived_from_metadata_first_byte() {
    let (env, client, stealth_address) = setup();
    let meta = metadata(&env);
    let expected_bucket = view_tag_bucket(&meta);

    client.announce(&STELLAR_V2_SCHEME_ID, &stealth_address, &epk(&env), &meta);

    let event = env.events().all().last().unwrap();
    let got_bucket: u32 = FromVal::from_val(&env, &event.1.get(2).unwrap());
    assert_eq!(got_bucket, FIXTURE_VIEW_TAG_BUCKET, "topic[2] must equal view_tag_bucket(metadata)");
    assert_eq!(got_bucket, expected_bucket, "topic[2] must equal view_tag_bucket helper output");
}

/// topic[3] is the metadata_kind flag — imported directly from the crate constant
/// so any future constant change breaks this test at compile time.
#[test]
fn topic3_is_metadata_kind_view_tag_constant() {
    let (env, client, stealth_address) = setup();
    client.announce(&STELLAR_V2_SCHEME_ID, &stealth_address, &epk(&env), &metadata(&env));

    let event = env.events().all().last().unwrap();
    let got_kind: u32 = FromVal::from_val(&env, &event.1.get(3).unwrap());
    assert_eq!(got_kind, METADATA_KIND_VIEW_TAG, "topic[3] must equal METADATA_KIND_VIEW_TAG");
}

#[test]
fn topic_tuple_has_exactly_four_entries() {
    let (env, client, stealth_address) = setup();
    client.announce(&STELLAR_V2_SCHEME_ID, &stealth_address, &epk(&env), &metadata(&env));

    let event = env.events().all().last().unwrap();
    assert_eq!(event.1.len(), 4, "topic tuple must have exactly 4 entries");
}

#[test]
fn full_topic_tuple_round_trip() {
    let (env, client, stealth_address) = setup();
    let meta = metadata(&env);
    let bucket = view_tag_bucket(&meta);

    client.announce(&STELLAR_V2_SCHEME_ID, &stealth_address, &epk(&env), &meta);

    let event = env.events().all().last().unwrap();
    let expected_topics: soroban_sdk::Vec<Val> = vec![
        &env,
        symbol_short!("announce").into_val(&env),
        STELLAR_V2_SCHEME_ID.into_val(&env),
        bucket.into_val(&env),
        METADATA_KIND_VIEW_TAG.into_val(&env),
    ];
    assert_eq!(event.1, expected_topics, "full topic tuple must match expected shape");
}

#[test]
fn data_payload_round_trip() {
    let (env, client, stealth_address) = setup();
    let epk_val = epk(&env);
    let meta_val = metadata(&env);

    client.announce(&STELLAR_V2_SCHEME_ID, &stealth_address, &epk_val, &meta_val);

    let event = env.events().all().last().unwrap();
    let (decoded_address, decoded_epk, decoded_meta): (Address, BytesN<32>, Bytes) =
        FromVal::from_val(&env, &event.2);

    assert_eq!(decoded_address, stealth_address, "data[0] stealth_address must round-trip");
    assert_eq!(decoded_epk, epk_val, "data[1] ephemeral_pub_key must round-trip");
    assert_eq!(decoded_meta, meta_val, "data[2] metadata must round-trip");
}

#[test]
fn decoded_metadata_first_byte_is_view_tag() {
    let (env, client, stealth_address) = setup();
    client.announce(&STELLAR_V2_SCHEME_ID, &stealth_address, &epk(&env), &metadata(&env));

    let event = env.events().all().last().unwrap();
    let (_, _, decoded_meta): (Address, BytesN<32>, Bytes) = FromVal::from_val(&env, &event.2);

    let first_byte = decoded_meta.get(0).expect("metadata must have at least one byte");
    assert_eq!(first_byte as u32, FIXTURE_VIEW_TAG_BUCKET, "metadata[0] must equal the view tag used in topic[2]");
}

#[test]
fn different_view_tags_produce_different_buckets() {
    // Soroban's test env accumulates events within a single transaction
    // boundary; each client call is its own invocation.  Capture the bucket
    // from each call immediately after it returns.
    let (env, client, stealth_address) = setup();

    let meta_low = Bytes::from_slice(&env, &[0x00u8, 0x00]);
    let meta_high = Bytes::from_slice(&env, &[0xffu8, 0x00]);

    client.announce(&STELLAR_V2_SCHEME_ID, &stealth_address, &epk(&env), &meta_low);
    let bucket_low: u32 =
        FromVal::from_val(&env, &env.events().all().last().unwrap().1.get(2).unwrap());

    client.announce(&STELLAR_V2_SCHEME_ID, &stealth_address, &epk(&env), &meta_high);
    let bucket_high: u32 =
        FromVal::from_val(&env, &env.events().all().last().unwrap().1.get(2).unwrap());

    assert_eq!(bucket_low, 0u32, "view tag 0x00 must produce bucket 0");
    assert_eq!(bucket_high, 255u32, "view tag 0xff must produce bucket 255");
    assert_ne!(bucket_low, bucket_high, "different view tags must produce different buckets");
}

#[test]
fn event_is_emitted_by_announcer_contract() {
    let env = Env::default();
    let contract_id = env.register(StealthAnnouncerContract, ());
    let client = StealthAnnouncerContractClient::new(&env, &contract_id);
    let stealth_address = Address::generate(&env);

    client.announce(&STELLAR_V2_SCHEME_ID, &stealth_address, &epk(&env), &metadata(&env));

    let event = env.events().all().last().unwrap();
    assert_eq!(event.0, contract_id, "event must be emitted by the announcer contract");
}
