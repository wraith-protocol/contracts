#![cfg(test)]

use soroban_sdk::{
    contract, contractimpl,
    testutils::Address as _,
    token, Address, Bytes, BytesN, Env,
};
use stealth_vault::{StealthVaultContract, StealthVaultContractClient, VaultError};

#[contract]
pub struct MockAnnouncer;

#[contractimpl]
impl MockAnnouncer {
    pub fn announce(
        _env: Env,
        _scheme_id: u32,
        _stealth_address: Address,
        _ephemeral_pub_key: BytesN<32>,
        _metadata: Bytes,
    ) {
    }
}

#[test]
fn test_init_twice_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let announcer = env.register(MockAnnouncer, ());
    let vault_id = env.register(StealthVaultContract, ());
    let client = StealthVaultContractClient::new(&env, &vault_id);

    client.init(&announcer);
    let result = client.try_init(&announcer);
    assert_eq!(result, Err(Ok(VaultError::AlreadyInitialized)));
}

#[test]
fn test_claim_after_unlock_transfers_funds() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.min_persistent_entry_ttl = 600_000;
    });

    let announcer = env.register(MockAnnouncer, ());
    let vault_id = env.register(StealthVaultContract, ());
    let client = StealthVaultContractClient::new(&env, &vault_id);
    client.init(&announcer);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    let token_client = token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&sender, &10_000);

    let deposit_id = client.deposit(
        &sender,
        &recipient,
        &750,
        &token_id,
        &10,
        &2000,
        &BytesN::from_array(&env, &[9u8; 32]),
    );

    env.ledger().with_mut(|li| li.sequence_number = 10);
    client.claim(&deposit_id, &recipient);
    assert_eq!(token::Client::new(&env, &token_id).balance(&recipient), 750);
}

#[test]
fn test_refund_is_blocked_until_window_is_reached() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.min_persistent_entry_ttl = 600_000;
    });

    let announcer = env.register(MockAnnouncer, ());
    let vault_id = env.register(StealthVaultContract, ());
    let client = StealthVaultContractClient::new(&env, &vault_id);
    client.init(&announcer);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    let token_client = token::StellarAssetClient::new(&env, &token_id);
    token_client.mint(&sender, &10_000);

    let deposit_id = client.deposit(
        &sender,
        &recipient,
        &500,
        &token_id,
        &100,
        &2000,
        &BytesN::from_array(&env, &[11u8; 32]),
    );

    env.ledger().with_mut(|li| li.sequence_number = 1999);
    let result = client.try_refund(&deposit_id);
    assert_eq!(result, Err(Ok(VaultError::NotYetRefundable)));
}
