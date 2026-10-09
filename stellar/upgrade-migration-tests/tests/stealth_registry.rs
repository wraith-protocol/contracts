#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Bytes, Env};
use stealth_registry::{RegistryError, StealthRegistryContractClient};
use stealth_registry_v0::StealthRegistryContractClient as StealthRegistryV0Client;
use upgrade_migration_tests::{deploy_wasm, upgrade_contract_wasm, wasm};

#[test]
fn v0_instance_storage_invisible_to_v1_persistent_reader_after_wasm_swap() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = deploy_wasm(&env, wasm::STEALTH_REGISTRY_V0);
    let v0 = StealthRegistryV0Client::new(&env, &contract_id);

    let registrant = Address::generate(&env);
    let scheme_id: u32 = 1;
    let meta = Bytes::from_slice(&env, &[42u8; 64]);
    v0.register_keys(&registrant, &scheme_id, &meta).unwrap();

    upgrade_contract_wasm(&env, &contract_id, wasm::STEALTH_REGISTRY);

    let v1 = StealthRegistryContractClient::new(&env, &contract_id);
    assert_eq!(
        v1.try_stealth_meta_address_of(&registrant, &scheme_id),
        Err(Ok(RegistryError::NotRegistered))
    );
}

#[test]
fn registry_wasm_rollback_restores_v0_instance_reads() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = deploy_wasm(&env, wasm::STEALTH_REGISTRY_V0);
    let v0 = StealthRegistryV0Client::new(&env, &contract_id);

    let registrant = Address::generate(&env);
    let scheme_id: u32 = 2;
    let meta = Bytes::from_slice(&env, &[9u8; 64]);
    v0.register_keys(&registrant, &scheme_id, &meta).unwrap();

    upgrade_contract_wasm(&env, &contract_id, wasm::STEALTH_REGISTRY);
    upgrade_contract_wasm(&env, &contract_id, wasm::STEALTH_REGISTRY_V0);

    let v0_after = StealthRegistryV0Client::new(&env, &contract_id);
    assert_eq!(
        v0_after.stealth_meta_address_of(&registrant, &scheme_id).unwrap(),
        meta
    );
}
