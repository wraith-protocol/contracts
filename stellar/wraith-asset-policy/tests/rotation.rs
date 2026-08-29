#![cfg(test)]

use soroban_sdk::{
    testutils::Address as _,
    Address, Env,
};
use wraith_asset_policy::{WraithAssetPolicy, WraithAssetPolicyClient};

#[test]
fn test_policy_rotation_and_removal_work() {
    let env = Env::default();
    env.mock_all_auths();

    let policy_id = env.register(WraithAssetPolicy, ());
    let client = WraithAssetPolicyClient::new(&env, &policy_id);
    let admin = Address::generate(&env);
    let a1 = Address::generate(&env);
    let a2 = Address::generate(&env);
    let a3 = Address::generate(&env);

    client.init(&admin, &soroban_sdk::vec![&env, a1.clone()]);

    assert!(client.check_asset(&a1));
    assert!(!client.check_asset(&a2));

    client.add_asset(&a2);
    assert!(client.check_asset(&a2));

    client.add_asset(&a3);
    client.remove_asset(&a1);
    assert!(!client.check_asset(&a1));
    assert!(client.check_asset(&a2));
    assert!(client.check_asset(&a3));
}

#[test]
fn test_removing_missing_asset_is_noop() {
    let env = Env::default();
    env.mock_all_auths();

    let policy_id = env.register(WraithAssetPolicy, ());
    let client = WraithAssetPolicyClient::new(&env, &policy_id);
    let admin = Address::generate(&env);
    let asset = Address::generate(&env);

    client.init(&admin, &soroban_sdk::vec![&env]);
    client.remove_asset(&asset);
    assert!(!client.check_asset(&asset));
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_cannot_reinit_after_default_setup() {
    let env = Env::default();
    env.mock_all_auths();

    let policy_id = env.register(WraithAssetPolicy, ());
    let client = WraithAssetPolicyClient::new(&env, &policy_id);
    let admin = Address::generate(&env);

    client.init(&admin, &soroban_sdk::vec![&env]);
    client.init(&admin, &soroban_sdk::vec![&env]);
}
