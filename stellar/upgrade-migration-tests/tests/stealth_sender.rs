#![cfg(test)]

use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Ledger},
    Address, Bytes, BytesN, Env, Vec,
};
use stealth_sender::{SenderError, StealthSenderContractClient};
use stealth_sender_v0::StealthSenderContractClient as StealthSenderV0Client;
use upgrade_migration_tests::{deploy_wasm, upgrade_contract_wasm, wasm};

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

fn mock_announcer(env: &Env) -> Address {
    env.register(MockAnnouncer, ())
}

#[test]
fn v0_to_v1_upgrade_preserves_announcer_and_allows_v1_send() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.min_persistent_entry_ttl = 518_400;
        li.max_entry_ttl = 1_036_800;
    });

    let contract_id = deploy_wasm(&env, wasm::STEALTH_SENDER_V0);
    let v0 = StealthSenderV0Client::new(&env, &contract_id);

    let announcer = mock_announcer(&env);
    v0.init(&announcer).unwrap();

    upgrade_contract_wasm(&env, &contract_id, wasm::STEALTH_SENDER);

    let v1 = StealthSenderContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    assert_eq!(
        v1.try_init(&announcer, &None, &None, &0, &admin),
        Err(Ok(SenderError::AlreadyInitialized))
    );

    let sender = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    let stealth = Address::generate(&env);
    let eph = BytesN::from_array(&env, &[7u8; 32]);
    let metadata = Bytes::from_slice(&env, &[0u8; 1]);

    v1.send(
        &sender,
        &token,
        &100,
        &1,
        &stealth,
        &eph,
        &metadata,
    )
    .unwrap();

    let signers = Vec::from_array(&env, [Address::generate(&env)]);
    v1.init_multisig(&signers, &1).unwrap();
    assert_eq!(v1.signers().len(), 1);
}
