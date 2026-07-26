#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, MockAuth, MockAuthInvoke},
    vec, Address, Bytes, Env, IntoVal,
};
use stealth_registry::{RegistryError, StealthRegistryContract, StealthRegistryContractClient};

fn setup() -> (Env, StealthRegistryContractClient<'static>) {
    let env = Env::default();
    let contract_id = env.register(StealthRegistryContract, ());
    let client = StealthRegistryContractClient::new(&env, &contract_id);
    (env, client)
}

#[test]
fn test_finding_storage_key_collision_risk() {
    // Finding: Ensure no two (registrant, scheme_id) pairs collide
    let (env, client) = setup();
    env.mock_all_auths();

    let registrant1 = Address::generate(&env);
    let registrant2 = Address::generate(&env);

    let scheme_id = 1u16;

    let meta1 = Bytes::from_slice(&env, &[1u8; 64]);
    let meta2 = Bytes::from_slice(&env, &[2u8; 64]);

    client.register_keys(&registrant1, &scheme_id, &meta1);
    client.register_keys(&registrant2, &scheme_id, &meta2);

    // Verify they are separate
    assert_eq!(
        client.stealth_meta_address_of(&registrant1, &scheme_id),
        meta1
    );
    assert_eq!(
        client.stealth_meta_address_of(&registrant2, &scheme_id),
        meta2
    );
}

#[test]
fn test_finding_replacement_squatting() {
    // Finding: Ensure an attacker cannot pre-register a victim's slot
    // This is prevented by require_auth() on the registrant
    let (env, client) = setup();

    let attacker = Address::generate(&env);
    let victim = Address::generate(&env);
    let scheme_id = 1u16;
    let meta = Bytes::from_slice(&env, &[42u8; 64]);

    // Attacker tries to register for the victim.
    // The environment requires auth for `victim`, which isn't provided.
    // We mock auth for the attacker only.
    env.mock_auths(&[MockAuth {
        address: &attacker,
        invoke: &MockAuthInvoke {
            contract: &client.address,
            fn_name: "register_keys",
            args: (&victim, scheme_id, meta.clone()).into_val(&env),
            sub_invokes: &[],
        },
    }]);

    // This should fail because the contract actually calls `victim.require_auth()`,
    // but we only provided auth for `attacker`.
    // In soroban tests, a missing auth will panic or return an auth error.
    let result = client.try_register_keys(&victim, &scheme_id, &meta);
    assert!(
        result.is_err(),
        "Registration should fail if victim's auth is missing"
    );
}

#[test]
fn test_finding_scheme_id_forward_compatibility() {
    // Finding: Ensure unknown/future scheme_ids can be registered
    let (env, client) = setup();
    env.mock_all_auths();

    let registrant = Address::generate(&env);
    // Use an arbitrarily high, currently unknown scheme ID
    let future_scheme_id = 9999u16;
    let meta = Bytes::from_slice(&env, &[8u8; 64]);

    // Registration should succeed without knowing what scheme 9999 is
    client.register_keys(&registrant, &future_scheme_id, &meta);

    assert_eq!(
        client.stealth_meta_address_of(&registrant, &future_scheme_id),
        meta
    );
}

#[test]
fn test_finding_replay_protection_across_write_boundary() {
    // Finding: Overwriting is currently allowed (intentional behavior).
    // Ensure updates to the same slot succeed and overwrite previous data.
    let (env, client) = setup();
    env.mock_all_auths();

    let registrant = Address::generate(&env);
    let scheme_id = 1u16;

    let meta_v1 = Bytes::from_slice(&env, &[1u8; 64]);
    client.register_keys(&registrant, &scheme_id, &meta_v1);

    // Validate first write
    assert_eq!(
        client.stealth_meta_address_of(&registrant, &scheme_id),
        meta_v1
    );

    // Write boundary crossed: overwrite with v2
    let meta_v2 = Bytes::from_slice(&env, &[2u8; 64]);
    client.register_keys(&registrant, &scheme_id, &meta_v2);

    // Validate overwrite
    assert_eq!(
        client.stealth_meta_address_of(&registrant, &scheme_id),
        meta_v2
    );
}
