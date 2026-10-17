use soroban_sdk::testutils::{Address as _, EnvTestConfig, Events};
use soroban_sdk::{
    contract, contractimpl, symbol_short, vec, Address, Bytes, BytesN, Env, FromVal, IntoVal, Val,
};
use stealth_announcer::{
    StealthAnnouncerContract, StealthAnnouncerContractClient, METADATA_KIND_VIEW_TAG,
    STELLAR_V2_SCHEME_ID,
};

fn audit_env() -> Env {
    Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    })
}

fn hex_digit(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => panic!("invalid hex digit"),
    }
}

fn pubkey(hex: &str) -> [u8; 32] {
    let raw = hex.as_bytes();
    let mut out = [0u8; 32];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = (hex_digit(raw[index * 2]) << 4) | hex_digit(raw[index * 2 + 1]);
    }
    out
}

#[test]
fn wa_ann_01_v2_payload_uses_stealth_address_not_caller() {
    let env = audit_env();
    let contract_id = env.register(StealthAnnouncerContract, ());
    let client = StealthAnnouncerContractClient::new(&env, &contract_id);

    let invoker = Address::generate(&env);
    let stealth_address = Address::generate(&env);
    let ephemeral_pub_key = BytesN::from_array(&env, &[1u8; 32]);
    let metadata = Bytes::from_slice(&env, &[0u8; 1]);

    client.announce(
        &STELLAR_V2_SCHEME_ID,
        &stealth_address,
        &ephemeral_pub_key,
        &metadata,
    );

    let events = env.events().all();
    let event = events.last().unwrap();
    let actual_value: (Address, BytesN<32>, Bytes) = FromVal::from_val(&env, &event.2);

    assert_ne!(contract_id, invoker);
    assert_eq!(actual_value, (stealth_address, ephemeral_pub_key, metadata));
}

#[test]
fn wa_ann_02_oversized_metadata_is_accepted() {
    let env = audit_env();
    let contract_id = env.register(StealthAnnouncerContract, ());
    let client = StealthAnnouncerContractClient::new(&env, &contract_id);

    let stealth_address = Address::generate(&env);
    let ephemeral_pub_key = BytesN::from_array(&env, &[3u8; 32]);
    let metadata = Bytes::from_array(&env, &[7u8; 4096]);

    client.announce(
        &STELLAR_V2_SCHEME_ID,
        &stealth_address,
        &ephemeral_pub_key,
        &metadata,
    );

    let events = env.events().all();
    let event = events.last().unwrap();
    let actual_value: (Address, BytesN<32>, Bytes) = FromVal::from_val(&env, &event.2);

    assert_eq!(actual_value, (stealth_address, ephemeral_pub_key, metadata));
}

#[test]
#[should_panic(expected = "invalid ed25519 ephemeral public key")]
fn wa_ann_03_zero_ephemeral_pub_key_is_rejected() {
    let env = audit_env();
    let contract_id = env.register(StealthAnnouncerContract, ());
    let client = StealthAnnouncerContractClient::new(&env, &contract_id);

    let stealth_address = Address::generate(&env);
    let zero_ephemeral_pub_key = BytesN::from_array(&env, &[0u8; 32]);
    let metadata = Bytes::from_slice(&env, &[0u8; 1]);

    client.announce(
        &STELLAR_V2_SCHEME_ID,
        &stealth_address,
        &zero_ephemeral_pub_key,
        &metadata,
    );
}

/// A non-canonical encoding (`y >= p`) is not a valid ed25519 point. The
/// all-`0xff` bytes mask the sign bit to `2^255 - 1`, which is `>= p`.
#[test]
#[should_panic(expected = "invalid ed25519 ephemeral public key")]
fn wa_ann_03_non_canonical_ephemeral_pub_key_is_rejected() {
    let env = audit_env();
    let contract_id = env.register(StealthAnnouncerContract, ());
    let client = StealthAnnouncerContractClient::new(&env, &contract_id);

    client.announce(
        &STELLAR_V2_SCHEME_ID,
        &Address::generate(&env),
        &BytesN::from_array(&env, &[0xff; 32]),
        &Bytes::from_slice(&env, &[0u8; 1]),
    );
}

/// An encoding that does not decompress to a curve point is rejected. The
/// all-`0x02` bytes are the placeholder the old fixtures used and are off-curve.
#[test]
#[should_panic(expected = "invalid ed25519 ephemeral public key")]
fn wa_ann_03_off_curve_ephemeral_pub_key_is_rejected() {
    let env = audit_env();
    let contract_id = env.register(StealthAnnouncerContract, ());
    let client = StealthAnnouncerContractClient::new(&env, &contract_id);

    client.announce(
        &STELLAR_V2_SCHEME_ID,
        &Address::generate(&env),
        &BytesN::from_array(&env, &[2u8; 32]),
        &Bytes::from_slice(&env, &[0u8; 1]),
    );
}

/// Small-order points are not usable public keys; the identity (`01 00 …`)
/// is the canonical example.
#[test]
#[should_panic(expected = "invalid ed25519 ephemeral public key")]
fn wa_ann_03_small_order_ephemeral_pub_key_is_rejected() {
    let env = audit_env();
    let contract_id = env.register(StealthAnnouncerContract, ());
    let client = StealthAnnouncerContractClient::new(&env, &contract_id);

    let mut identity = [0u8; 32];
    identity[0] = 1;

    client.announce(
        &STELLAR_V2_SCHEME_ID,
        &Address::generate(&env),
        &BytesN::from_array(&env, &identity),
        &Bytes::from_slice(&env, &[0u8; 1]),
    );
}

/// The rejection must not block valid keys: an RFC 8032 public key is accepted.
#[test]
fn wa_ann_03_valid_ephemeral_pub_key_is_accepted() {
    let env = audit_env();
    let contract_id = env.register(StealthAnnouncerContract, ());
    let client = StealthAnnouncerContractClient::new(&env, &contract_id);

    let stealth_address = Address::generate(&env);
    let ephemeral_pub_key = BytesN::from_array(
        &env,
        &pubkey("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"),
    );
    let metadata = Bytes::from_slice(&env, &[0u8; 1]);

    client.announce(
        &STELLAR_V2_SCHEME_ID,
        &stealth_address,
        &ephemeral_pub_key,
        &metadata,
    );

    let events = env.events().all();
    let event = events.last().unwrap();
    let actual_value: (Address, BytesN<32>, Bytes) = FromVal::from_val(&env, &event.2);
    assert_eq!(actual_value, (stealth_address, ephemeral_pub_key, metadata));
}

#[contract]
pub struct ForwarderContract;

#[contractimpl]
impl ForwarderContract {
    pub fn forward(
        env: Env,
        announcer: Address,
        scheme_id: u32,
        stealth_address: Address,
        ephemeral_pub_key: BytesN<32>,
        metadata: Bytes,
    ) {
        let client = StealthAnnouncerContractClient::new(&env, &announcer);
        client.announce(&scheme_id, &stealth_address, &ephemeral_pub_key, &metadata);
    }
}

#[test]
fn wa_ann_04_cpi_can_emit_announcements_without_auth() {
    let env = audit_env();
    let announcer_id = env.register(StealthAnnouncerContract, ());
    let forwarder_id = env.register(ForwarderContract, ());
    let forwarder = ForwarderContractClient::new(&env, &forwarder_id);

    let stealth_address = Address::generate(&env);
    let ephemeral_pub_key = BytesN::from_array(&env, &[6u8; 32]);
    let metadata = Bytes::from_slice(&env, &[0u8; 1]);

    forwarder.forward(
        &announcer_id,
        &STELLAR_V2_SCHEME_ID,
        &stealth_address,
        &ephemeral_pub_key,
        &metadata,
    );

    let events = env.events().all();
    let event = events.last().unwrap();
    let expected_topics: soroban_sdk::Vec<Val> = vec![
        &env,
        symbol_short!("announce").into_val(&env),
        STELLAR_V2_SCHEME_ID.into_val(&env),
        0u32.into_val(&env),
        METADATA_KIND_VIEW_TAG.into_val(&env),
    ];
    let actual_value: (Address, BytesN<32>, Bytes) = FromVal::from_val(&env, &event.2);

    assert_eq!(event.0, announcer_id.clone());
    assert_eq!(event.1, expected_topics);
    assert_eq!(actual_value, (stealth_address, ephemeral_pub_key, metadata));
}
