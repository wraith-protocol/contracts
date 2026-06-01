use soroban_sdk::testutils::{Address as _, EnvTestConfig, Events};
use soroban_sdk::{
    contract, contractimpl, symbol_short, vec, Address, Bytes, BytesN, Env, FromVal, IntoVal, Val,
};
use stealth_announcer::{StealthAnnouncerContract, StealthAnnouncerContractClient};

fn audit_env() -> Env {
    Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    })
}

#[test]
fn wa_ann_01_caller_payload_is_contract_not_invoker() {
    let env = audit_env();
    let contract_id = env.register(StealthAnnouncerContract, ());
    let client = StealthAnnouncerContractClient::new(&env, &contract_id);

    let invoker = Address::generate(&env);
    let stealth_address = Address::generate(&env);
    let ephemeral_pub_key = BytesN::from_array(&env, &[1u8; 32]);
    let metadata = Bytes::from_slice(&env, &[0u8; 1]);

    client.announce(&1u32, &stealth_address, &ephemeral_pub_key, &metadata);

    let events = env.events().all();
    let event = events.last().unwrap();
    let actual_value: (Address, BytesN<32>, Bytes) = FromVal::from_val(&env, &event.2);

    assert_ne!(contract_id, invoker);
    assert_eq!(actual_value, (contract_id, ephemeral_pub_key, metadata));
}

#[test]
fn wa_ann_02_oversized_metadata_is_accepted() {
    let env = audit_env();
    let contract_id = env.register(StealthAnnouncerContract, ());
    let client = StealthAnnouncerContractClient::new(&env, &contract_id);

    let stealth_address = Address::generate(&env);
    let ephemeral_pub_key = BytesN::from_array(&env, &[2u8; 32]);
    let metadata = Bytes::from_array(&env, &[7u8; 4096]);

    client.announce(&1u32, &stealth_address, &ephemeral_pub_key, &metadata);

    let events = env.events().all();
    let event = events.last().unwrap();
    let actual_value: (Address, BytesN<32>, Bytes) = FromVal::from_val(&env, &event.2);

    assert_eq!(actual_value, (contract_id, ephemeral_pub_key, metadata));
}

#[test]
fn wa_ann_03_zero_ephemeral_pub_key_is_accepted() {
    let env = audit_env();
    let contract_id = env.register(StealthAnnouncerContract, ());
    let client = StealthAnnouncerContractClient::new(&env, &contract_id);

    let stealth_address = Address::generate(&env);
    let zero_ephemeral_pub_key = BytesN::from_array(&env, &[0u8; 32]);
    let metadata = Bytes::from_slice(&env, &[0u8; 1]);

    client.announce(&1u32, &stealth_address, &zero_ephemeral_pub_key, &metadata);

    let events = env.events().all();
    let event = events.last().unwrap();
    let actual_value: (Address, BytesN<32>, Bytes) = FromVal::from_val(&env, &event.2);

    assert_eq!(
        actual_value,
        (contract_id, zero_ephemeral_pub_key, metadata)
    );
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
    let ephemeral_pub_key = BytesN::from_array(&env, &[4u8; 32]);
    let metadata = Bytes::from_slice(&env, &[0u8; 1]);

    forwarder.forward(
        &announcer_id,
        &1u32,
        &stealth_address,
        &ephemeral_pub_key,
        &metadata,
    );

    let events = env.events().all();
    let event = events.last().unwrap();
    let expected_topics: soroban_sdk::Vec<Val> = vec![
        &env,
        symbol_short!("announce").into_val(&env),
        1u32.into_val(&env),
        stealth_address.into_val(&env),
    ];
    let actual_value: (Address, BytesN<32>, Bytes) = FromVal::from_val(&env, &event.2);

    assert_eq!(event.0, announcer_id.clone());
    assert_eq!(event.1, expected_topics);
    assert_eq!(actual_value, (announcer_id, ephemeral_pub_key, metadata));
}
