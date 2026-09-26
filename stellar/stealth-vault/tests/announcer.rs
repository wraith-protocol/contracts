//! Wires the real `stealth-announcer` behind the vault.
//!
//! The unit tests in `src/lib.rs` register a permissive mock announcer, so they
//! cannot catch a scheme-id mismatch — and the production announcer asserts on
//! `scheme_id`, which would revert every `deposit` in a real deployment. These
//! tests pin the vault against the contract it actually calls.

use soroban_sdk::testutils::{Address as _, Events, Ledger};
use soroban_sdk::{symbol_short, token, Address, BytesN, Env, IntoVal, Val};

use stealth_announcer::{StealthAnnouncerContract, STELLAR_V2_SCHEME_ID};
use stealth_vault::{StealthVaultContract, StealthVaultContractClient};

struct Fixture {
    env: Env,
    client: StealthVaultContractClient<'static>,
    sender: Address,
    recipient: Address,
    token_id: Address,
}

fn fixture() -> Fixture {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.min_persistent_entry_ttl = 600000;
    });

    let announcer_id = env.register(StealthAnnouncerContract, ());
    let vault_id = env.register(StealthVaultContract, ());
    let client = StealthVaultContractClient::new(&env, &vault_id);

    let admin = Address::generate(&env);
    client.init(&admin, &announcer_id);

    let sender = Address::generate(&env);
    let recipient = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    token::StellarAssetClient::new(&env, &token_id).mint(&sender, &10000);

    Fixture {
        env,
        client,
        sender,
        recipient,
        token_id,
    }
}

/// A deposit must reach the production announcer without tripping its
/// `scheme_id` assertion.
#[test]
fn deposit_announces_through_the_real_announcer() {
    let f = fixture();
    let epk = BytesN::from_array(&f.env, &[1u8; 32]);

    f.client.deposit(
        &f.sender,
        &f.recipient,
        &1000,
        &f.token_id,
        &100,
        &2000,
        &epk,
    );

    let announce_topic: Val = symbol_short!("announce").into_val(&f.env);
    let announcements: std::vec::Vec<_> = f
        .env
        .events()
        .all()
        .iter()
        .filter(|(_, topics, _)| {
            topics.first().map(|t| t.shallow_eq(&announce_topic)) == Some(true)
        })
        .collect();

    assert_eq!(announcements.len(), 1, "deposit emits one announcement");

    // Topic layout is (announce, scheme_id, view_tag_bucket, metadata_kind).
    let scheme_id: u32 = announcements[0].1.get(1).unwrap().into_val(&f.env);
    assert_eq!(scheme_id, STELLAR_V2_SCHEME_ID);
}

/// The whole deposit reverts if the announcement does, so funds are never
/// locked without a matching announcement.
#[test]
fn deposit_and_claim_round_trip_against_the_real_announcer() {
    let f = fixture();
    let epk = BytesN::from_array(&f.env, &[2u8; 32]);
    let token_client = token::Client::new(&f.env, &f.token_id);

    let deposit_id = f.client.deposit(
        &f.sender,
        &f.recipient,
        &750,
        &f.token_id,
        &100,
        &2000,
        &epk,
    );
    assert_eq!(token_client.balance(&f.recipient), 0);

    f.env.ledger().with_mut(|li| li.sequence_number = 100);
    f.client.claim(&deposit_id, &f.recipient);

    assert_eq!(token_client.balance(&f.recipient), 750);
}
