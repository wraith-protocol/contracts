#![cfg(test)]

use soroban_sdk::{
    testutils::Address as _,
    token::{Client as TokenClient, StellarAssetClient},
    Address, Bytes, Env, Vec,
};
use stealth_batch_sender::{StealthBatchSender, StealthBatchSenderClient, Transfer, MAX_BATCH_SIZE};

fn create_token<'a>(env: &Env, admin: &Address) -> (TokenClient<'a>, StellarAssetClient<'a>) {
    let contract_id = env.register_stellar_asset_contract_v2(admin.clone());
    (
        TokenClient::new(env, &contract_id.address()),
        StellarAssetClient::new(env, &contract_id.address()),
    )
}

fn dummy_pub_key(env: &Env) -> Bytes {
    Bytes::from_slice(env, &[0x02u8; 33])
}

#[test]
#[should_panic(expected = "batch must contain at least one transfer")]
fn test_empty_batch_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, _) = create_token(&env, &admin);

    let contract_id = env.register(StealthBatchSender, ());
    let client = StealthBatchSenderClient::new(&env, &contract_id);

    client.batch_send(&sender, &Vec::new(&env), &token.address);
}

#[test]
#[should_panic(expected = "ephemeral_pub_key must not be empty")]
fn test_empty_ephemeral_pub_key_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    token_admin.mint(&sender, &100);

    let contract_id = env.register(StealthBatchSender, ());
    let client = StealthBatchSenderClient::new(&env, &contract_id);

    let transfers = soroban_sdk::vec![
        &env,
        Transfer {
            stealth_address: Address::generate(&env),
            ephemeral_pub_key: Bytes::new(&env),
            amount: 10,
        }
    ];

    client.batch_send(&sender, &transfers, &token.address);
}

#[test]
#[should_panic(expected = "transfer amount must be positive")]
fn test_zero_amount_in_batch_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    token_admin.mint(&sender, &100);

    let contract_id = env.register(StealthBatchSender, ());
    let client = StealthBatchSenderClient::new(&env, &contract_id);

    let transfers = soroban_sdk::vec![
        &env,
        Transfer {
            stealth_address: Address::generate(&env),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 0,
        }
    ];

    client.batch_send(&sender, &transfers, &token.address);
}

#[test]
fn test_batch_send_with_multiple_recipients_updates_balances() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    token_admin.mint(&sender, &1_000);

    let contract_id = env.register(StealthBatchSender, ());
    let client = StealthBatchSenderClient::new(&env, &contract_id);

    let r1 = Address::generate(&env);
    let r2 = Address::generate(&env);
    let r3 = Address::generate(&env);

    let transfers = soroban_sdk::vec![
        &env,
        Transfer {
            stealth_address: r1.clone(),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 100,
        },
        Transfer {
            stealth_address: r2.clone(),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 250,
        },
        Transfer {
            stealth_address: r3.clone(),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 350,
        }
    ];

    client.batch_send(&sender, &transfers, &token.address);

    assert_eq!(token.balance(&sender), 300);
    assert_eq!(token.balance(&r1), 100);
    assert_eq!(token.balance(&r2), 250);
    assert_eq!(token.balance(&r3), 350);
    assert_eq!(client.max_batch_size(), MAX_BATCH_SIZE);
}
