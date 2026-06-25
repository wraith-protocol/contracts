#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation},
    token::{Client as TokenClient, StellarAssetClient},
    vec, Address, Bytes, Env, IntoVal,
};

fn create_token<'a>(
    env: &Env,
    admin: &Address,
) -> (TokenClient<'a>, StellarAssetClient<'a>) {
    let contract_id = env.register_stellar_asset_contract_v2(admin.clone());
    (
        TokenClient::new(env, &contract_id.address()),
        StellarAssetClient::new(env, &contract_id.address()),
    )
}

fn dummy_pub_key(env: &Env) -> Bytes {
    Bytes::from_slice(env, &[0x02u8; 33]) // compressed secp256k1 pubkey
}

#[test]
fn test_batch_send_success() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);

    // Mint 1000 tokens to sender
    token_admin.mint(&sender, &1000);

    let contract_id = env.register_contract(None, StealthBatchSender);
    let client = StealthBatchSenderClient::new(&env, &contract_id);

    let stealth1 = Address::generate(&env);
    let stealth2 = Address::generate(&env);
    let stealth3 = Address::generate(&env);

    let transfers = vec![
        &env,
        Transfer { stealth_address: stealth1.clone(), ephemeral_pub_key: dummy_pub_key(&env), amount: 100 },
        Transfer { stealth_address: stealth2.clone(), ephemeral_pub_key: dummy_pub_key(&env), amount: 200 },
        Transfer { stealth_address: stealth3.clone(), ephemeral_pub_key: dummy_pub_key(&env), amount: 300 },
    ];

    client.batch_send(&sender, &transfers, &token.address);

    // Verify balances
    assert_eq!(token.balance(&sender), 400);
    assert_eq!(token.balance(&stealth1), 100);
    assert_eq!(token.balance(&stealth2), 200);
    assert_eq!(token.balance(&stealth3), 300);
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

    let contract_id = env.register_contract(None, StealthBatchSender);
    let client = StealthBatchSenderClient::new(&env, &contract_id);

    // Build 101 transfers (over cap)
    let mut transfers = Vec::new(&env);
    for _ in 0..=MAX_BATCH_SIZE {
        transfers.push_back(Transfer {
            stealth_address: Address::generate(&env),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 1,
        });
    }

    client.batch_send(&sender, &transfers, &token.address);
}

#[test]
#[should_panic]
fn test_atomicity_on_mid_batch_failure() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);

    // Mint only enough for 2 transfers, but send 3
    token_admin.mint(&sender, &150);

    let contract_id = env.register_contract(None, StealthBatchSender);
    let client = StealthBatchSenderClient::new(&env, &contract_id);

    let transfers = vec![
        &env,
        Transfer { stealth_address: Address::generate(&env), ephemeral_pub_key: dummy_pub_key(&env), amount: 100 },
        Transfer { stealth_address: Address::generate(&env), ephemeral_pub_key: dummy_pub_key(&env), amount: 100 }, // fails here
        Transfer { stealth_address: Address::generate(&env), ephemeral_pub_key: dummy_pub_key(&env), amount: 100 },
    ];

    // Must panic — and because Soroban tx is atomic, first transfer is also rolled back
    client.batch_send(&sender, &transfers, &token.address);
}

#[test]
#[should_panic(expected = "batch must contain at least one transfer")]
fn test_empty_batch_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, _) = create_token(&env, &admin);

    let contract_id = env.register_contract(None, StealthBatchSender);
    let client = StealthBatchSenderClient::new(&env, &contract_id);

    client.batch_send(&sender, &Vec::new(&env), &token.address);
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

    let contract_id = env.register_contract(None, StealthBatchSender);
    let client = StealthBatchSenderClient::new(&env, &contract_id);

    let transfers = vec![
        &env,
        Transfer {
            stealth_address: Address::generate(&env),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 0,
        },
    ];

    client.batch_send(&sender, &transfers, &token.address);
}

#[test]
fn test_max_batch_size_query() {
    let env = Env::default();
    let contract_id = env.register_contract(None, StealthBatchSender);
    let client = StealthBatchSenderClient::new(&env, &contract_id);
    assert_eq!(client.max_batch_size(), 100u32);
}