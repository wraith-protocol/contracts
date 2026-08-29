#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events},
    token::{Client as TokenClient, StellarAssetClient},
    vec, Address, Bytes, BytesN, Env, FromVal, IntoVal, Val,
};
use stealth_announcer::{
    StealthAnnouncerContract, METADATA_KIND_VIEW_TAG, STELLAR_V2_SCHEME_ID as ANNOUNCER_SCHEME_ID,
};

fn create_token<'a>(env: &Env, admin: &Address) -> (TokenClient<'a>, StellarAssetClient<'a>) {
    let contract_id = env.register_stellar_asset_contract_v2(admin.clone());
    (
        TokenClient::new(env, &contract_id.address()),
        StellarAssetClient::new(env, &contract_id.address()),
    )
}

fn dummy_pub_key(env: &Env) -> Bytes {
    Bytes::from_slice(env, &[0x02u8; 32])
}

fn dummy_metadata(env: &Env) -> Bytes {
    Bytes::from_slice(env, &[0x2Au8])
}

fn metadata_with_tag(env: &Env, tag: u8) -> Bytes {
    Bytes::from_slice(env, &[tag])
}

/// Registers the contract and initialises it with a fresh admin +
/// a real announcer, no asset policy. Returns the client plus the admin address
/// (tests that need to pause/unpause use it).
fn setup(env: &Env) -> (StealthBatchSenderClient<'static>, Address) {
    let contract_id = env.register(StealthBatchSender, ());
    let client = StealthBatchSenderClient::new(env, &contract_id);
    let admin = Address::generate(env);
    let announcer = env.register(StealthAnnouncerContract, ());
    client.init(&admin, &announcer, &None);
    (client, admin)
}

#[test]
fn test_batch_send_success() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);

    token_admin.mint(&sender, &1000);

    let (client, _) = setup(&env);

    let stealth1 = Address::generate(&env);
    let stealth2 = Address::generate(&env);
    let stealth3 = Address::generate(&env);

    let transfers = vec![
        &env,
        Transfer {
            stealth_address: stealth1.clone(),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 100,
            metadata: dummy_metadata(&env),
        },
        Transfer {
            stealth_address: stealth2.clone(),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 200,
            metadata: dummy_metadata(&env),
        },
        Transfer {
            stealth_address: stealth3.clone(),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 300,
            metadata: dummy_metadata(&env),
        },
    ];

    client.batch_send(&sender, &transfers, &token.address);

    assert_eq!(token.balance(&sender), 400);
    assert_eq!(token.balance(&stealth1), 100);
    assert_eq!(token.balance(&stealth2), 200);
    assert_eq!(token.balance(&stealth3), 300);
}

#[test]
fn test_batch_send_emits_v2_four_topic_announce() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    token_admin.mint(&sender, &1000);

    let (client, _) = setup(&env);

    let stealth_a = Address::generate(&env);
    let stealth_b = Address::generate(&env);

    let transfers = vec![
        &env,
        Transfer {
            stealth_address: stealth_a.clone(),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 100,
            metadata: metadata_with_tag(&env, 7),
        },
        Transfer {
            stealth_address: stealth_b.clone(),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 200,
            metadata: metadata_with_tag(&env, 99),
        },
    ];

    client.batch_send(&sender, &transfers, &token.address);

    let announce_sym: Val = symbol_short!("announce").into_val(&env);
    let mut announce_events = vec![&env];
    for event in env.events().all().iter() {
        let first: Option<Val> = event.1.first();
        if first.map(|t| t.shallow_eq(&announce_sym)) == Some(true) {
            announce_events.push_back(event);
        }
    }

    assert_eq!(
        announce_events.len(),
        2,
        "batch-sender must emit one v2 announce per transfer"
    );

    let expected_buckets = [7u32, 99u32];
    let expected_addrs = [stealth_a, stealth_b];
    for i in 0..2 {
        let event = announce_events.get(i).unwrap();
        assert_eq!(event.1.len(), 4, "v2 announce uses all 4 topic slots");

        let expected_topics: soroban_sdk::Vec<Val> = vec![
            &env,
            symbol_short!("announce").into_val(&env),
            ANNOUNCER_SCHEME_ID.into_val(&env),
            expected_buckets[i as usize].into_val(&env),
            METADATA_KIND_VIEW_TAG.into_val(&env),
        ];
        assert_eq!(event.1, expected_topics);

        let (addr, _epk, meta): (Address, BytesN<32>, Bytes) = FromVal::from_val(&env, &event.2);
        assert_eq!(addr, expected_addrs[i as usize]);
        assert_eq!(
            meta,
            metadata_with_tag(&env, expected_buckets[i as usize] as u8)
        );
    }
}

#[test]
fn test_batch_send_rejects_uninitialized() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    token_admin.mint(&sender, &100);

    let contract_id = env.register(StealthBatchSender, ());
    let client = StealthBatchSenderClient::new(&env, &contract_id);

    let transfers = vec![
        &env,
        Transfer {
            stealth_address: Address::generate(&env),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 100,
            metadata: dummy_metadata(&env),
        },
    ];

    let result = client.try_batch_send(&sender, &transfers, &token.address);
    assert_eq!(result, Err(Ok(BatchSenderError::NotInitialized)));
}

#[test]
fn test_init_rejects_second_call() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin) = setup(&env);
    let announcer = Address::generate(&env);

    let result = client.try_init(&admin, &announcer, &None);
    assert_eq!(result, Err(Ok(BatchSenderError::AlreadyInitialized)));
}

#[test]
fn test_batch_size_cap() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    token_admin.mint(&sender, &100_000);

    let (client, _) = setup(&env);

    let mut transfers = Vec::new(&env);
    for _ in 0..=MAX_BATCH_SIZE {
        transfers.push_back(Transfer {
            stealth_address: Address::generate(&env),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 1,
            metadata: dummy_metadata(&env),
        });
    }

    let result = client.try_batch_send(&sender, &transfers, &token.address);
    assert_eq!(result, Err(Ok(BatchSenderError::BatchTooLarge)));
}

#[test]
#[should_panic]
fn test_atomicity_on_mid_batch_failure() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);

    // Sender can afford the first transfer but not all three — the
    // shortfall should roll back every transfer in the batch, not just
    // the one that failed.
    token_admin.mint(&sender, &150);

    let (client, _) = setup(&env);

    let transfers = vec![
        &env,
        Transfer {
            stealth_address: Address::generate(&env),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 100,
            metadata: dummy_metadata(&env),
        },
        Transfer {
            stealth_address: Address::generate(&env),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 100,
            metadata: dummy_metadata(&env),
        },
        Transfer {
            stealth_address: Address::generate(&env),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 100,
            metadata: dummy_metadata(&env),
        },
    ];

    // insufficient balance on the second transfer aborts the whole tx
    client.batch_send(&sender, &transfers, &token.address);
}

#[test]
fn test_empty_batch_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, _) = create_token(&env, &admin);

    let (client, _) = setup(&env);

    let result = client.try_batch_send(&sender, &Vec::new(&env), &token.address);
    assert_eq!(result, Err(Ok(BatchSenderError::EmptyBatch)));
}

#[test]
fn test_zero_amount_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    token_admin.mint(&sender, &100);

    let (client, _) = setup(&env);

    let transfers = vec![
        &env,
        Transfer {
            stealth_address: Address::generate(&env),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 0,
            metadata: dummy_metadata(&env),
        },
    ];

    let result = client.try_batch_send(&sender, &transfers, &token.address);
    assert_eq!(result, Err(Ok(BatchSenderError::NonPositiveAmount)));
}

#[test]
fn test_empty_ephemeral_key_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    token_admin.mint(&sender, &100);

    let (client, _) = setup(&env);

    let transfers = vec![
        &env,
        Transfer {
            stealth_address: Address::generate(&env),
            ephemeral_pub_key: Bytes::new(&env),
            amount: 100,
            metadata: dummy_metadata(&env),
        },
    ];

    let result = client.try_batch_send(&sender, &transfers, &token.address);
    assert_eq!(result, Err(Ok(BatchSenderError::EmptyEphemeralKey)));
}

#[test]
fn test_max_batch_size_query() {
    let env = Env::default();
    let contract_id = env.register(StealthBatchSender, ());
    let client = StealthBatchSenderClient::new(&env, &contract_id);
    assert_eq!(client.max_batch_size(), 100u32);
}
