#![cfg(test)]

use soroban_sdk::{
    contract, contractimpl, symbol_short,
    testutils::{Address as _, Events},
    Address, Bytes, BytesN, Env, IntoVal, Symbol,
};
use stealth_announcer::{StealthAnnouncerContract, StealthAnnouncerContractClient};
use stealth_registry::{StealthRegistryContract, StealthRegistryContractClient};

#[test]
fn test_conformance_announcer() {
    let env = Env::default();
    let contract_id = env.register(StealthAnnouncerContract, ());
    let client = StealthAnnouncerContractClient::new(&env, &contract_id);

    let stealth_address = Address::generate(&env);
    let ephemeral_pub_key = BytesN::from_array(&env, &[3; 32]);
    let mut meta_arr = [0u8; 32];
    meta_arr[0] = 0x00; // view tag
    let metadata = Bytes::from_array(&env, &meta_arr);
    let scheme_id: u32 = 2; // STELLAR_V2_SCHEME_ID

    client.announce(&scheme_id, &stealth_address, &ephemeral_pub_key, &metadata);

    let events = env.events().all();
    assert_eq!(events.len(), 1);

    let (contract_id, topics, data) = events.get(0).unwrap();
    assert_eq!(contract_id, client.address);

    let topic0: Symbol = topics.get(0).unwrap().into_val(&env);
    let topic1: u32 = topics.get(1).unwrap().into_val(&env);
    let topic2: u32 = topics.get(2).unwrap().into_val(&env);
    let topic3: u32 = topics.get(3).unwrap().into_val(&env);

    assert_eq!(topic0, symbol_short!("announce"));
    assert_eq!(topic1, scheme_id);
    assert_eq!(topic2, 0u32); // view_tag_bucket
    assert_eq!(topic3, 1u32); // METADATA_KIND_VIEW_TAG

    let (d_stealth_address, d_ephemeral_pub_key, d_metadata): (Address, BytesN<32>, Bytes) =
        data.into_val(&env);

    assert_eq!(d_stealth_address, stealth_address);
    assert_eq!(d_ephemeral_pub_key, ephemeral_pub_key);
    assert_eq!(d_metadata, metadata);
    assert_eq!(d_metadata.get(0).unwrap(), 0x00); // verify view tag
}

/// Cross-chain conformance fixture for the rejection path.
///
/// The canonical announcement fixture above uses a valid key. These cases pin
/// the inputs the Stellar announcer must reject: the all-zero key and ed25519
/// encodings that are non-canonical (`y >= p`) or off-curve. A rejected
/// announcement must not emit an event.
#[test]
#[should_panic(expected = "invalid ed25519 ephemeral public key")]
fn test_conformance_announcer_rejects_zero_ephemeral_pub_key() {
    announce_with_raw_key(&[0u8; 32]);
}

#[test]
#[should_panic(expected = "invalid ed25519 ephemeral public key")]
fn test_conformance_announcer_rejects_non_canonical_ephemeral_pub_key() {
    announce_with_raw_key(&[0xffu8; 32]);
}

#[test]
#[should_panic(expected = "invalid ed25519 ephemeral public key")]
fn test_conformance_announcer_rejects_off_curve_ephemeral_pub_key() {
    announce_with_raw_key(&[0x02u8; 32]);
}

fn announce_with_raw_key(raw: &[u8; 32]) {
    let env = Env::default();
    let contract_id = env.register(StealthAnnouncerContract, ());
    let client = StealthAnnouncerContractClient::new(&env, &contract_id);

    let mut meta_arr = [0u8; 32];
    meta_arr[0] = 0x00;
    client.announce(
        &2u32,
        &Address::generate(&env),
        &BytesN::from_array(&env, raw),
        &Bytes::from_array(&env, &meta_arr),
    );
}

#[test]
fn test_conformance_registry() {
    let env = Env::default();
    let contract_id = env.register(StealthRegistryContract, ());
    let client = StealthRegistryContractClient::new(&env, &contract_id);

    let registrant = Address::generate(&env);
    env.mock_all_auths();

    let scheme_id: u32 = 1;
    let stealth_meta_address = Bytes::from_array(&env, &[5; 64]);

    client.register_keys(&registrant, &scheme_id, &stealth_meta_address);

    let events = env.events().all();
    let event = events
        .iter()
        .find(|(_, topics, _)| {
            let topic0: Symbol = topics.get(0).unwrap().into_val(&env);
            topic0 == symbol_short!("register")
        })
        .unwrap();

    let (_, topics, data) = event;

    let topic0: Symbol = topics.get(0).unwrap().into_val(&env);
    let topic1: Address = topics.get(1).unwrap().into_val(&env);
    let topic2: u32 = topics.get(2).unwrap().into_val(&env);

    assert_eq!(topic0, symbol_short!("register"));
    assert_eq!(topic1, registrant);
    assert_eq!(topic2, scheme_id);

    let d_stealth_meta_address: Bytes = data.into_val(&env);
    assert_eq!(d_stealth_meta_address, stealth_meta_address);
}

use soroban_sdk::String;
use stealth_sender::{StealthSenderContract, StealthSenderContractClient, WithdrawalEntry};
use wraith_names::{WraithNamesContract, WraithNamesContractClient};

#[test]
fn test_conformance_names() {
    let env = Env::default();
    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.init(&admin);

    env.mock_all_auths();

    let owner = Address::generate(&env);
    let name_str = "alice";
    let name = String::from_str(&env, name_str);

    let mut sm_arr = [5u8; 64];
    sm_arr[0] = 0x02;
    let stealth_meta_address = Bytes::from_array(&env, &sm_arr);

    client.register(&owner, &name, &stealth_meta_address);
    let events = env.events().all();
    let e_register = events
        .iter()
        .find(|(_, topics, _)| {
            let topic0: Symbol = topics.get(0).unwrap().into_val(&env);
            topic0 == symbol_short!("register")
        })
        .unwrap();
    let r_topic0: Symbol = e_register.1.get(0).unwrap().into_val(&env);
    let r_topic1: BytesN<32> = e_register.1.get(1).unwrap().into_val(&env);

    let name_bytes = Bytes::from_slice(&env, name_str.as_bytes());
    let name_hash: BytesN<32> = env.crypto().sha256(&name_bytes).into();

    assert_eq!(r_topic0, symbol_short!("register"));
    assert_eq!(r_topic1, name_hash);

    let (d_name, d_sma): (String, Bytes) = e_register.2.into_val(&env);
    assert_eq!(d_name, name);
    assert_eq!(d_sma, stealth_meta_address);

    client.release(&owner, &name);
    let events2 = env.events().all();
    let e_release = events2
        .iter()
        .find(|(_, topics, _)| {
            let topic0: Symbol = topics.get(0).unwrap().into_val(&env);
            topic0 == symbol_short!("release")
        })
        .unwrap();
    let r_topic0_rel: Symbol = e_release.1.get(0).unwrap().into_val(&env);
    let r_topic1_rel: BytesN<32> = e_release.1.get(1).unwrap().into_val(&env);

    assert_eq!(r_topic0_rel, symbol_short!("release"));
    assert_eq!(r_topic1_rel, name_hash);

    let d_name_rel: String = e_release.2.into_val(&env);
    assert_eq!(d_name_rel, name);
}

#[test]
fn test_conformance_sender_withdrawer() {
    let env = Env::default();
    let announcer_id = env.register(StealthAnnouncerContract, ());
    let sender_id = env.register(StealthSenderContract, ());
    let client = StealthSenderContractClient::new(&env, &sender_id);

    let admin = Address::generate(&env);
    client.init(&announcer_id, &None, &None, &0, &admin);

    env.mock_all_auths();

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();

    let withdrawer = Address::generate(&env);

    let token_admin_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    token_admin_client.mint(&withdrawer, &20000000);

    let to = Address::generate(&env);
    let amount: i128 = 10000000;

    let mut entries = soroban_sdk::Vec::new(&env);
    entries.push_back(WithdrawalEntry {
        token: token_id.clone(),
        to: to.clone(),
        amount,
    });

    client.withdraw_many(&withdrawer, &entries);

    let events = env.events().all();

    // There are 2 events emitted in withdraw_many:
    // 1. Withdrawn
    // 2. BatchWithdrawn
    // And possibly transfer events from token contract.
    // We filter by contract_id == sender_id
    let sender_events: std::vec::Vec<_> =
        events.iter().filter(|(c, _, _)| *c == sender_id).collect();

    let e_withdrawn = sender_events.get(sender_events.len() - 2).unwrap();
    let w_topic0: Symbol = e_withdrawn.1.get(0).unwrap().into_val(&env);
    assert_eq!(w_topic0, Symbol::new(&env, "Withdrawn"));
    let (d_w, d_to, d_a, d_t): (Address, Address, i128, Address) = e_withdrawn.2.into_val(&env);
    assert_eq!(d_w, withdrawer);
    assert_eq!(d_to, to);
    assert_eq!(d_a, amount);
    assert_eq!(d_t, token_id);

    let e_batch = sender_events.get(sender_events.len() - 1).unwrap();
    let bw_topic0: Symbol = e_batch.1.get(0).unwrap().into_val(&env);
    assert_eq!(bw_topic0, Symbol::new(&env, "BatchWithdrawn"));

    let (d_w2, d_l, d_ta): (Address, u32, i128) = e_batch.2.into_val(&env);
    assert_eq!(d_w2, withdrawer);
    assert_eq!(d_l, 1);
    assert_eq!(d_ta, amount);
}
