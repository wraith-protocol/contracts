#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Vec};

/// Storage keys.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// The admin address.
    Admin,
    /// Set of allowed asset addresses.
    Allowed(Address),
}

/// Errors.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PolicyError {
    /// Caller is not the admin.
    NotAdmin = 1,
    /// Admin already set.
    AdminAlreadySet = 2,
}

#[contract]
pub struct WraithAssetPolicy;

#[contractimpl]
impl WraithAssetPolicy {
    /// Initialize the policy contract with an admin.
    pub fn init(env: Env, admin: Address) -> Result<(), PolicyError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(PolicyError::AdminAlreadySet);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    /// Add an asset to the allowlist. Admin only.
    pub fn allow_asset(env: Env, asset: Address) -> Result<(), PolicyError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(PolicyError::NotAdmin)?;
        admin.require_auth();

        env.storage()
            .instance()
            .set(&DataKey::Allowed(asset), &true);
        Ok(())
    }

    /// Remove an asset from the allowlist. Admin only.
    pub fn disallow_asset(env: Env, asset: Address) -> Result<(), PolicyError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(PolicyError::NotAdmin)?;
        admin.require_auth();

        env.storage()
            .instance()
            .set(&DataKey::Allowed(asset), &false);
        Ok(())
    }

    /// Check if an asset is allowed.
    /// Returns true if allowed, false if not.
    pub fn is_allowed(env: Env, asset: Address) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Allowed(asset))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::Env;

    #[test]
    fn test_init_and_allow() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithAssetPolicy, ());
        let client = WraithAssetPolicyClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.init(&admin);

        let xlm = Address::generate(&env);
        assert!(!client.is_allowed(&xlm));

        client.allow_asset(&xlm);
        assert!(client.is_allowed(&xlm));
    }

    #[test]
    fn test_disallow() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithAssetPolicy, ());
        let client = WraithAssetPolicyClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.init(&admin);

        let xlm = Address::generate(&env);
        client.allow_asset(&xlm);
        assert!(client.is_allowed(&xlm));

        client.disallow_asset(&xlm);
        assert!(!client.is_allowed(&xlm));
    }

    #[test]
    fn test_default_disallowed() {
        let env = Env::default();

        let contract_id = env.register(WraithAssetPolicy, ());
        let client = WraithAssetPolicyClient::new(&env, &contract_id);

        let unknown = Address::generate(&env);
        assert!(!client.is_allowed(&unknown));
    }
}
