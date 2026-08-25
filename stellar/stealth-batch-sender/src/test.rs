#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events},
    token::{Client as TokenClient, StellarAssetClient},
    vec, Address, Bytes, BytesN, Env, FromVal, IntoVal, Val,
};
use stealth_announcer::{
    StealthAnnouncerContract, StealthAnnouncerContractClient, METADATA_KIND_VIEW_TAG,
    STELLAR_V2_SCHEME_ID,
};

fn create_token<'a>(env: &Env, admin: &Address) -> (TokenClient<'a>, StellarAssetClient<'a>) {
    let contract_id = env.register_stellar_asset_contract_v2(admin.clone());
    (
        TokenClient::new(env, &contract_id.address()),
        StellarAssetClient::new(env, &contract_id.address()),
    )
}

fn dummy_pub_key(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[0x02u8; 32])
}

fn dummy_metadata(env: &Env, view_tag: u8) -> Bytes {
    Bytes::from_slice(env, &[view_tag, 0x00])
}

fn register_announcer(env: &Env) -> Address {
    env.register(StealthAnnouncerContract, ())
}

fn make_transfer(env: &Env, amount: i128, view_tag: u8) -> Transfer {
    Transfer {
        stealth_address: Address::generate(env),
        ephemeral_pub_key: dummy_pub_key(env),
        amount,
        metadata: dummy_metadata(env, view_tag),
    }
}

/// Collect v2 announce events: topics `("announce", scheme_id, view_tag_bucket, metadata_kind)`.
fn announce_events(env: &Env) -> soroban_sdk::Vec<(Address, soroban_sdk::Vec<Val>, Val)> {
    let mut out = soroban_sdk::Vec::new(env);
    for event in env.events().all().iter() {
        let topics = event.1.clone();
        if topics.len() != 4 {
            continue;
        }
        let topic0: soroban_sdk::Symbol = FromVal::from_val(env, &topics.get(0).unwrap());
        if topic0 == symbol_short!("announce") {
            out.push_back(event);
        }
    }
    out
}

#[test]
fn test_batch_send_success() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);

    token_admin.mint(&sender, &1000);

    let announcer = register_announcer(&env);
    let contract_id = env.register(StealthBatchSender, ());
    let client = StealthBatchSenderClient::new(&env, &contract_id);

    let stealth1 = Address::generate(&env);
    let stealth2 = Address::generate(&env);
    let stealth3 = Address::generate(&env);

    let transfers = vec![
        &env,
        Transfer {
            stealth_address: stealth1.clone(),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 100,
            metadata: dummy_metadata(&env, 10),
        },
        Transfer {
            stealth_address: stealth2.clone(),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 200,
            metadata: dummy_metadata(&env, 20),
        },
        Transfer {
            stealth_address: stealth3.clone(),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 300,
            metadata: dummy_metadata(&env, 30),
        },
    ];

    client.batch_send(
        &sender,
        &transfers,
        &token.address,
        &announcer,
        &STELLAR_V2_SCHEME_ID,
    );

    assert_eq!(token.balance(&sender), 400);
    assert_eq!(token.balance(&stealth1), 100);
    assert_eq!(token.balance(&stealth2), 200);
    assert_eq!(token.balance(&stealth3), 300);
}

#[test]
fn test_batch_send_emits_v2_four_topic_layout() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    token_admin.mint(&sender, &1000);

    let announcer = register_announcer(&env);
    let contract_id = env.register(StealthBatchSender, ());
    let client = StealthBatchSenderClient::new(&env, &contract_id);

    let stealth1 = Address::generate(&env);
    let stealth2 = Address::generate(&env);
    let epk1 = BytesN::from_array(&env, &[0x11u8; 32]);
    let epk2 = BytesN::from_array(&env, &[0x22u8; 32]);
    let meta1 = Bytes::from_slice(&env, &[42u8, 1u8]);
    let meta2 = Bytes::from_slice(&env, &[99u8, 2u8]);

    let transfers = vec![
        &env,
        Transfer {
            stealth_address: stealth1.clone(),
            ephemeral_pub_key: epk1.clone(),
            amount: 100,
            metadata: meta1.clone(),
        },
        Transfer {
            stealth_address: stealth2.clone(),
            ephemeral_pub_key: epk2.clone(),
            amount: 200,
            metadata: meta2.clone(),
        },
    ];

    client.batch_send(
        &sender,
        &transfers,
        &token.address,
        &announcer,
        &STELLAR_V2_SCHEME_ID,
    );

    let announced = announce_events(&env);
    assert_eq!(announced.len(), 2, "one announce event per transfer");

    let first = announced.get(0).unwrap();
    assert_eq!(first.0, announcer, "events are emitted by the announcer");
    let expected_topics_1: soroban_sdk::Vec<Val> = vec![
        &env,
        symbol_short!("announce").into_val(&env),
        STELLAR_V2_SCHEME_ID.into_val(&env),
        42u32.into_val(&env),
        METADATA_KIND_VIEW_TAG.into_val(&env),
    ];
    assert_eq!(first.1, expected_topics_1);
    let data1: (Address, BytesN<32>, Bytes) = FromVal::from_val(&env, &first.2);
    assert_eq!(data1, (stealth1, epk1, meta1));

    let second = announced.get(1).unwrap();
    assert_eq!(second.0, announcer);
    let expected_topics_2: soroban_sdk::Vec<Val> = vec![
        &env,
        symbol_short!("announce").into_val(&env),
        STELLAR_V2_SCHEME_ID.into_val(&env),
        99u32.into_val(&env),
        METADATA_KIND_VIEW_TAG.into_val(&env),
    ];
    assert_eq!(second.1, expected_topics_2);
    let data2: (Address, BytesN<32>, Bytes) = FromVal::from_val(&env, &second.2);
    assert_eq!(data2, (stealth2, epk2, meta2));
}

#[test]
fn test_view_tag_bucket_derived_from_first_metadata_byte() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    token_admin.mint(&sender, &10);

    let announcer = register_announcer(&env);
    let client = StealthBatchSenderClient::new(&env, &env.register(StealthBatchSender, ()));

    let transfers = vec![&env, make_transfer(&env, 1, 0), make_transfer(&env, 1, 255)];
    client.batch_send(
        &sender,
        &transfers,
        &token.address,
        &announcer,
        &STELLAR_V2_SCHEME_ID,
    );

    let announced = announce_events(&env);
    assert_eq!(announced.len(), 2);

    let bucket0: u32 = FromVal::from_val(&env, &announced.get(0).unwrap().1.get(2).unwrap());
    let bucket255: u32 = FromVal::from_val(&env, &announced.get(1).unwrap().1.get(2).unwrap());
    assert_eq!(bucket0, 0u32);
    assert_eq!(bucket255, 255u32);
}

#[test]
#[should_panic(expected = "metadata must include view tag")]
fn test_empty_metadata_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    token_admin.mint(&sender, &100);

    let announcer = register_announcer(&env);
    let client = StealthBatchSenderClient::new(&env, &env.register(StealthBatchSender, ()));

    let transfers = vec![
        &env,
        Transfer {
            stealth_address: Address::generate(&env),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 1,
            metadata: Bytes::new(&env),
        },
    ];

    client.batch_send(
        &sender,
        &transfers,
        &token.address,
        &announcer,
        &STELLAR_V2_SCHEME_ID,
    );
}

#[test]
#[should_panic(expected = "batch exceeds MAX_BATCH_SIZE")]
fn test_batch_size_cap() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    token_admin.mint(&sender, &100_000);

    let announcer = register_announcer(&env);
    let contract_id = env.register(StealthBatchSender, ());
    let client = StealthBatchSenderClient::new(&env, &contract_id);

    let mut transfers = Vec::new(&env);
    for _ in 0..=MAX_BATCH_SIZE {
        transfers.push_back(make_transfer(&env, 1, 1));
    }

    client.batch_send(
        &sender,
        &transfers,
        &token.address,
        &announcer,
        &STELLAR_V2_SCHEME_ID,
    );
}

#[test]
#[should_panic]
fn test_atomicity_on_mid_batch_failure() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);

    token_admin.mint(&sender, &150);

    let announcer = register_announcer(&env);
    let contract_id = env.register(StealthBatchSender, ());
    let client = StealthBatchSenderClient::new(&env, &contract_id);

    let transfers = vec![
        &env,
        make_transfer(&env, 100, 1),
        make_transfer(&env, 100, 2),
        make_transfer(&env, 100, 3),
    ];

    client.batch_send(
        &sender,
        &transfers,
        &token.address,
        &announcer,
        &STELLAR_V2_SCHEME_ID,
    );
}

#[test]
#[should_panic(expected = "batch must contain at least one transfer")]
fn test_empty_batch_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, _) = create_token(&env, &admin);

    let announcer = register_announcer(&env);
    let contract_id = env.register(StealthBatchSender, ());
    let client = StealthBatchSenderClient::new(&env, &contract_id);

    client.batch_send(
        &sender,
        &Vec::new(&env),
        &token.address,
        &announcer,
        &STELLAR_V2_SCHEME_ID,
    );
}

#[test]
#[should_panic(expected = "transfer amount must be positive")]
fn test_zero_amount_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    token_admin.mint(&sender, &100);

    let announcer = register_announcer(&env);
    let contract_id = env.register(StealthBatchSender, ());
    let client = StealthBatchSenderClient::new(&env, &contract_id);

    let transfers = vec![&env, make_transfer(&env, 0, 1)];

    client.batch_send(
        &sender,
        &transfers,
        &token.address,
        &announcer,
        &STELLAR_V2_SCHEME_ID,
    );
}

#[test]
fn test_max_batch_size_query() {
    let env = Env::default();
    let contract_id = env.register(StealthBatchSender, ());
    let client = StealthBatchSenderClient::new(&env, &contract_id);
    assert_eq!(client.max_batch_size(), 100u32);
}

/// Sanity: the real announcer client used by batch-sender is the v2 contract.
#[test]
fn test_announcer_client_matches_v2_schema() {
    let env = Env::default();
    let announcer = register_announcer(&env);
    let client = StealthAnnouncerContractClient::new(&env, &announcer);
    let stealth = Address::generate(&env);
    let epk = dummy_pub_key(&env);
    let meta = dummy_metadata(&env, 7);
    client.announce(&STELLAR_V2_SCHEME_ID, &stealth, &epk, &meta);

    let announced = announce_events(&env);
    assert_eq!(announced.len(), 1);
    let topics = announced.get(0).unwrap().1;
    let bucket: u32 = FromVal::from_val(&env, &topics.get(2).unwrap());
    let kind: u32 = FromVal::from_val(&env, &topics.get(3).unwrap());
    assert_eq!(bucket, 7u32);
    assert_eq!(kind, METADATA_KIND_VIEW_TAG);
}
 