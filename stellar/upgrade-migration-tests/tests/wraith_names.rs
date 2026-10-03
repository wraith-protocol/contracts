#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Bytes, BytesN, Env, String,
};
use upgrade_migration_tests::{deploy_wasm, upgrade_contract_wasm, wasm};
use wraith_names::{DataKey as V1NameKey, NameEntry as V1NameEntry, NamesError, WraithNamesContractClient};
use wraith_names_v0::{
    DataKey as V0NameKey, NameEntry as V0NameEntry, WraithNamesContractClient as WraithNamesV0Client,
};

const TTL_THRESHOLD: u32 = 17_280;
const TTL_EXTEND_TO: u32 = 518_400;

fn env_with_ttl() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.min_persistent_entry_ttl = TTL_EXTEND_TO;
        li.max_entry_ttl = TTL_EXTEND_TO * 2;
    });
    env
}

fn meta(env: &Env, seed: u8) -> Bytes {
    Bytes::from_slice(env, &[seed; 64])
}

fn hash_name(env: &Env, name: &String) -> BytesN<32> {
    let len = name.len() as usize;
    let mut buf = [0u8; 32];
    if len > 0 {
        name.copy_into_slice(&mut buf[..len]);
    }
    let name_bytes = Bytes::from_slice(env, &buf[..len]);
    BytesN::from_array(env, &env.crypto().sha256(&name_bytes).to_array())
}

/// Operational migration: copy v0 instance records into v1 persistent layout before WASM swap.
fn migrate_instance_records_to_persistent(env: &Env, contract_id: &Address, name: &String) {
    let name_hash = hash_name(env, name);
    env.as_contract(contract_id, || {
        let name_key_v0 = V0NameKey::Name(name_hash.clone());
        let entry: V0NameEntry = env
            .storage()
            .instance()
            .get(&name_key_v0)
            .expect("v0 name entry must exist in instance storage");

        let meta_hash = BytesN::from_array(env, &env.crypto().sha256(&entry.stealth_meta_address).to_array());
        let reverse_hash: BytesN<32> = env
            .storage()
            .instance()
            .get(&V0NameKey::Reverse(meta_hash.clone()))
            .expect("v0 reverse entry must exist");

        let v1_entry = V1NameEntry {
            name: entry.name.clone(),
            stealth_meta_address: entry.stealth_meta_address.clone(),
            owner: entry.owner.clone(),
            parent: None,
        };

        let name_key_v1 = V1NameKey::Name(name_hash.clone());
        let reverse_key_v1 = V1NameKey::Reverse(meta_hash);

        env.storage().persistent().set(&name_key_v1, &v1_entry);
        env.storage().persistent().set(&reverse_key_v1, &reverse_hash);
        env.storage()
            .persistent()
            .extend_ttl(&name_key_v1, TTL_THRESHOLD, TTL_EXTEND_TO);
        env.storage()
            .persistent()
            .extend_ttl(&reverse_key_v1, TTL_THRESHOLD, TTL_EXTEND_TO);
    });
}

#[test]
fn v0_to_v1_upgrade_without_storage_migration_is_incompatible() {
    let env = env_with_ttl();
    let contract_id = deploy_wasm(&env, wasm::WRAITH_NAMES_V0);
    let v0 = WraithNamesV0Client::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let name = String::from_str(&env, "alice");
    let meta_addr = meta(&env, 0xAA);
    v0.register(&owner, &name, &meta_addr).unwrap();

    upgrade_contract_wasm(&env, &contract_id, wasm::WRAITH_NAMES);

    let v1 = WraithNamesContractClient::new(&env, &contract_id);
    assert_eq!(
        v1.try_resolve(&name),
        Err(Ok(NamesError::NameNotFound))
    );
}

#[test]
fn v0_to_v1_upgrade_after_storage_migration_preserves_reads_and_v1_writes() {
    let env = env_with_ttl();
    let contract_id = deploy_wasm(&env, wasm::WRAITH_NAMES_V0);
    let v0 = WraithNamesV0Client::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let name = String::from_str(&env, "bob");
    let meta_addr = meta(&env, 0xBB);
    v0.register(&owner, &name, &meta_addr).unwrap();

    migrate_instance_records_to_persistent(&env, &contract_id, &name);
    upgrade_contract_wasm(&env, &contract_id, wasm::WRAITH_NAMES);

    let v1 = WraithNamesContractClient::new(&env, &contract_id);
    assert_eq!(v1.resolve(&name).unwrap(), meta_addr);

    let new_name = String::from_str(&env, "carol");
    let new_meta = meta(&env, 0xCC);
    v1.register(&owner, &new_name, &new_meta).unwrap();

    let name_hash = hash_name(&env, &new_name);
    env.as_contract(&contract_id, || {
        let stored: V1NameEntry = env
            .storage()
            .persistent()
            .get(&V1NameKey::Name(name_hash))
            .unwrap();
        assert!(stored.parent.is_none());
    });
}

#[test]
fn v0_to_v1_rollback_restores_v0_reads_when_no_v1_writes() {
    let env = env_with_ttl();
    let contract_id = deploy_wasm(&env, wasm::WRAITH_NAMES_V0);
    let v0 = WraithNamesV0Client::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let name = String::from_str(&env, "dave");
    let meta_addr = meta(&env, 0xDD);
    v0.register(&owner, &name, &meta_addr).unwrap();

    upgrade_contract_wasm(&env, &contract_id, wasm::WRAITH_NAMES);
    upgrade_contract_wasm(&env, &contract_id, wasm::WRAITH_NAMES_V0);

    let v0_after = WraithNamesV0Client::new(&env, &contract_id);
    assert_eq!(v0_after.resolve(&name).unwrap(), meta_addr);
}
