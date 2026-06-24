#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Asset(Address),
}

#[contract]
pub struct WraithAssetPolicy;

#[contractimpl]
impl WraithAssetPolicy {
    pub fn init(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn add_asset(env: Env, asset: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).expect("not initialized");
        admin.require_auth();
        env.storage().persistent().set(&DataKey::Asset(asset), &true);
    }

    pub fn remove_asset(env: Env, asset: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).expect("not initialized");
        admin.require_auth();
        env.storage().persistent().remove(&DataKey::Asset(asset));
    }

    pub fn check_asset(env: Env, asset: Address) -> bool {
        env.storage().persistent().has(&DataKey::Asset(asset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn test_policy_allowlist_flow() {
        let env = Env::default();
        env.mock_all_auths();

        let policy_id = env.register(WraithAssetPolicy, ());
        let client = WraithAssetPolicyClient::new(&env, &policy_id);

        let admin = Address::generate(&env);
        let asset_1 = Address::generate(&env);
        let asset_2 = Address::generate(&env);

        client.init(&admin);

        // Initially both assets should be blocked.
        assert!(!client.check_asset(&asset_1));
        assert!(!client.check_asset(&asset_2));

        // Allow asset 1.
        client.add_asset(&asset_1);
        assert!(client.check_asset(&asset_1));
        assert!(!client.check_asset(&asset_2));

        // Allow asset 2.
        client.add_asset(&asset_2);
        assert!(client.check_asset(&asset_1));
        assert!(client.check_asset(&asset_2));

        // Remove asset 1.
        client.remove_asset(&asset_1);
        assert!(!client.check_asset(&asset_1));
        assert!(client.check_asset(&asset_2));
    }

    #[test]
    #[should_panic(expected = "already initialized")]
    fn test_cannot_initialize_twice() {
        let env = Env::default();
        let policy_id = env.register(WraithAssetPolicy, ());
        let client = WraithAssetPolicyClient::new(&env, &policy_id);

        let admin = Address::generate(&env);
        client.init(&admin);
        client.init(&admin);
    }
}
