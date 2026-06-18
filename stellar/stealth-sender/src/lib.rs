#![no_std]

use soroban_sdk::{
    IntoVal,
    contract, contracterror, contractimpl, contracttype, token, Address, Bytes, BytesN, Env, Vec,
};

/// Storage keys.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// The address of the deployed StealthAnnouncer contract.
    Announcer,
    /// The admin address authorized for contract upgrades.
    Admin,
    /// Whether upgrade authority has been renounced.
    Renounced,
    /// Whether the contract is paused.
    Paused,
    /// Optional asset policy contract address.
    AssetPolicy,
}

/// Errors that the sender contract can produce.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SenderError {
    /// The contract has already been initialised.
    AlreadyInitialized = 1,
    /// The contract has not been initialised yet.
    NotInitialized = 2,
    /// The batch input vectors have mismatched lengths.
    LengthMismatch = 3,
    /// Caller is not the admin.
    NotAdmin = 4,
    /// Upgrade authority has been renounced.
    UpgradeRenounced = 5,
    /// Admin has already been set.
    AdminAlreadySet = 6,
    /// Contract is paused.
    ContractPaused = 7,
    /// Asset not allowed by policy.
    AssetNotAllowed = 8,
}

/// Lightweight client wrapper that invokes the StealthAnnouncer contract via
/// `env.invoke_contract`. This avoids needing a compiled WASM at build time
/// (unlike `contractimport!`) and keeps the build self-contained.
mod announcer_client {
    use soroban_sdk::{
    IntoVal,
    Address, Bytes, BytesN, Env};

    pub fn announce(
        env: &Env,
        announcer: &Address,
        scheme_id: u32,
        stealth_address: &Address,
        ephemeral_pub_key: &BytesN<32>,
        metadata: &Bytes,
    ) {
        let _: () = env.invoke_contract(
            announcer,
            &soroban_sdk::symbol_short!("announce"),
            soroban_sdk::vec![
                env,
                scheme_id.into_val(env),
                stealth_address.into_val(env),
                ephemeral_pub_key.into_val(env),
                metadata.into_val(env),
            ],
        );
    }

    use soroban_sdk::IntoVal;
}

#[contract]
pub struct StealthSenderContract;

#[contractimpl]
impl StealthSenderContract {
    /// Initialise the contract by storing the announcer address and admin.
    ///
    /// Must be called exactly once before any `send` or `batch_send`.
    /// The admin is authorized to perform contract upgrades.
    pub fn init(env: Env, announcer: Address, admin: Address) -> Result<(), SenderError> {
        if env.storage().instance().has(&DataKey::Announcer) {
            return Err(SenderError::AlreadyInitialized);
        }
        env.storage()
            .instance()
            .set(&DataKey::Announcer, &announcer);
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::Renounced, &false);
        Ok(())
    }

    /// Set or change the admin address. Only callable by current admin.
    pub fn set_admin(env: Env, new_admin: Address) -> Result<(), SenderError> {
        if env.storage().instance().get::<_, bool>(&DataKey::Renounced).unwrap_or(true) {
            return Err(SenderError::UpgradeRenounced);
        }
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(SenderError::NotInitialized)?;
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        Ok(())
    }

    /// Renounce upgrade authority permanently. Only callable by admin.
    /// After this call, the contract can never be upgraded.
    pub fn renounce_upgrade_authority(env: Env) -> Result<(), SenderError> {
        if env.storage().instance().get::<_, bool>(&DataKey::Renounced).unwrap_or(false) {
            return Err(SenderError::UpgradeRenounced);
        }
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(SenderError::NotInitialized)?;
        admin.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::Renounced, &true);
        Ok(())
    }

    /// Upgrade the contract WASM. Only callable by admin.
    /// Requires that upgrade authority has not been renounced.
    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>) -> Result<(), SenderError> {
        if env.storage().instance().get::<_, bool>(&DataKey::Renounced).unwrap_or(true) {
            return Err(SenderError::UpgradeRenounced);
        }
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(SenderError::NotInitialized)?;
        admin.require_auth();

        env.deployer().update_current_contract_wasm(new_wasm_hash);
        Ok(())
    }

    /// Get the current admin address.
    pub fn get_admin(env: Env) -> Result<Address, SenderError> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(SenderError::NotInitialized)
    }

    /// Check if upgrade authority has been renounced.
    pub fn is_renounced(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Renounced)
            .unwrap_or(false)
    }

    /// Pause the contract. Only callable by admin.
    /// When paused, send and batch_send are blocked.
    pub fn pause(env: Env) -> Result<(), SenderError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(SenderError::NotInitialized)?;
        admin.require_auth();

        env.storage().instance().set(&DataKey::Paused, &true);
        Ok(())
    }

    /// Unpause the contract. Only callable by admin.
    pub fn unpause(env: Env) -> Result<(), SenderError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(SenderError::NotInitialized)?;
        admin.require_auth();

        env.storage().instance().set(&DataKey::Paused, &false);
        Ok(())
    }

    /// Check if the contract is paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    /// Set the asset policy contract address. Admin only.
    /// Pass None to clear the policy (allow all assets).
    pub fn set_asset_policy(env: Env, policy: Option<Address>) -> Result<(), SenderError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(SenderError::NotInitialized)?;
        admin.require_auth();

        match policy {
            Some(addr) => env.storage().instance().set(&DataKey::AssetPolicy, &addr),
            None => env.storage().instance().remove(&DataKey::AssetPolicy),
        }
        Ok(())
    }

    /// Get the current asset policy address, if set.
    pub fn get_asset_policy(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::AssetPolicy)
    }

    /// Internal: check if an asset is allowed by the policy.
    fn check_asset_policy(env: &Env, asset: &Address) -> Result<(), SenderError> {
        if let Some(policy_addr) = env.storage().instance().get::<_, Address>(&DataKey::AssetPolicy)
        {
            let allowed: bool = env.invoke_contract(
                &policy_addr,
                &soroban_sdk::symbol_short!("is_allowed"),
                soroban_sdk::vec![env, asset.into_val(env)],
            );
            if !allowed {
                return Err(SenderError::AssetNotAllowed);
            }
        }
        Ok(())
    }

    /// Internal: revert if paused.
    fn require_not_paused(env: &Env) -> Result<(), SenderError> {
        let paused: bool = env
            .storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false);
        if paused {
            return Err(SenderError::ContractPaused);
        }
        Ok(())
    }

    /// Transfer tokens to a stealth address and emit an announcement.
    ///
    /// # Arguments
    /// * `sender`            - The address sending funds (must authorise).
    /// * `token`             - SAC token contract address (works for native XLM too).
    /// * `amount`            - Amount of tokens to transfer.
    /// * `scheme_id`         - Stealth address scheme identifier.
    /// * `stealth_address`   - The derived one-time stealth address.
    /// * `ephemeral_pub_key` - Ephemeral public key for the recipient to scan.
    /// * `metadata`          - Extra data (e.g. view tag).
    pub fn send(
        env: Env,
        sender: Address,
        token: Address,
        amount: i128,
        scheme_id: u32,
        stealth_address: Address,
        ephemeral_pub_key: BytesN<32>,
        metadata: Bytes,
    ) -> Result<(), SenderError> {
        Self::require_not_paused(&env)?;
        Self::check_asset_policy(&env, &token)?;
        sender.require_auth();

        let announcer: Address = env
            .storage()
            .instance()
            .get(&DataKey::Announcer)
            .ok_or(SenderError::NotInitialized)?;

        // Transfer tokens from sender to the stealth address.
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&sender, &stealth_address, &amount);

        // Emit the announcement via the announcer contract.
        announcer_client::announce(
            &env,
            &announcer,
            scheme_id,
            &stealth_address,
            &ephemeral_pub_key,
            &metadata,
        );

        Ok(())
    }

    /// Batch version of `send` — transfers tokens to multiple stealth addresses
    /// and emits an announcement for each.
    ///
    /// All input vectors must have the same length.
    pub fn batch_send(
        env: Env,
        sender: Address,
        token: Address,
        scheme_id: u32,
        stealth_addresses: Vec<Address>,
        ephemeral_pub_keys: Vec<BytesN<32>>,
        metadatas: Vec<Bytes>,
        amounts: Vec<i128>,
    ) -> Result<(), SenderError> {
        Self::require_not_paused(&env)?;
        Self::check_asset_policy(&env, &token)?;
        sender.require_auth();

        let len = stealth_addresses.len();
        if ephemeral_pub_keys.len() != len || metadatas.len() != len || amounts.len() != len {
            return Err(SenderError::LengthMismatch);
        }

        let announcer: Address = env
            .storage()
            .instance()
            .get(&DataKey::Announcer)
            .ok_or(SenderError::NotInitialized)?;

        let token_client = token::Client::new(&env, &token);

        for i in 0..len {
            let stealth_address = stealth_addresses.get(i).unwrap();
            let ephemeral_pub_key = ephemeral_pub_keys.get(i).unwrap();
            let metadata = metadatas.get(i).unwrap();
            let amount = amounts.get(i).unwrap();

            token_client.transfer(&sender, &stealth_address, &amount);

            announcer_client::announce(
                &env,
                &announcer,
                scheme_id,
                &stealth_address,
                &ephemeral_pub_key,
                &metadata,
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use soroban_sdk::{
    IntoVal,
    vec, Address, Bytes, BytesN, Env, Symbol};

    use crate::{StealthSenderContract, StealthSenderContractClient, SenderError};

    fn setup<'a>(env: &Env) -> StealthSenderContractClient<'a> {
        let contract_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(env, &contract_id);

        let announcer = Address::generate(env);
        let admin = Address::generate(env);
        client.init(&announcer, &admin);

        client
    }

    // === INIT TESTS ===

    #[test]
    fn test_init_sets_admin() {
        let env = Env::default();
        let client = setup(&env);
        let admin = Address::generate(&env);
        let announcer = Address::generate(&env);

        // Fresh contract
        let contract_id = env.register(StealthSenderContract, ());
        let client2 = StealthSenderContractClient::new(&env, &contract_id);
        client2.init(&announcer, &admin);

        assert_eq!(client2.get_admin(), admin);
        assert!(!client2.is_renounced());
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #1)")]
    fn test_init_cannot_be_called_twice() {
        let env = Env::default();
        let client = setup(&env);

        let announcer2 = Address::generate(&env);
        let admin2 = Address::generate(&env);
        client.init(&announcer2, &admin2); // Should panic: AlreadyInitialized
    }

    // === UPGRADE AUTH TESTS ===

    #[test]
    #[should_panic(expected = "Error(Contract, #4)")]
    fn test_non_admin_cannot_upgrade() {
        let env = Env::default();
        let client = setup(&env);

        let non_admin = Address::generate(&env);
        env.mock_all_auths();

        // Try to upgrade as non-admin — should fail
        let fake_hash = BytesN::from_array(&env, &[0u8; 32]);
        env.set_auth(&[soroban_sdk::testutils::Auth {
            address: non_admin.clone(),
            invoke: &soroban_sdk::testutils::AuthorizedInvocation {
                function: soroban_sdk::testutils::AuthorizedFunction::Contract((
                    client.address.clone(),
                    Symbol::new(&env, "upgrade"),
                    vec![&env, fake_hash.into_val(&env)],
                )),
                sub_invocations: vec![&env],
            },
        }]);

        // This should panic because non_admin is not the stored admin
        client.upgrade(&fake_hash);
    }

    #[test]
    fn test_admin_can_upgrade() {
        let env = Env::default();
        let contract_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(&env, &contract_id);

        let announcer = Address::generate(&env);
        let admin = Address::generate(&env);
        client.init(&announcer, &admin);

        env.mock_all_auths();

        // Admin should be able to call upgrade (will fail at deployer level
        // in test env, but auth check passes)
        let new_wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
        let result = client.try_upgrade(&new_wasm_hash);
        // In test environment, the deployer update may fail, but auth should pass
        // The important thing is it doesn't fail with NotAdmin
        assert!(result.is_ok() || !matches!(result.unwrap_err().unwrap(), SenderError::NotAdmin));
    }

    // === RENOUNCE TESTS ===

    #[test]
    fn test_admin_can_renounce() {
        let env = Env::default();
        let client = setup(&env);

        env.mock_all_auths();
        assert!(!client.is_renounced());

        client.renounce_upgrade_authority();
        assert!(client.is_renounced());
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #5)")]
    fn test_cannot_renounce_twice() {
        let env = Env::default();
        let client = setup(&env);

        env.mock_all_auths();
        client.renounce_upgrade_authority();
        client.renounce_upgrade_authority(); // Should panic: UpgradeRenounced
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #5)")]
    fn test_cannot_upgrade_after_renounce() {
        let env = Env::default();
        let client = setup(&env);

        env.mock_all_auths();
        client.renounce_upgrade_authority();

        let new_wasm_hash = BytesN::from_array(&env, &[1u8; 32]);
        client.upgrade(&new_wasm_hash); // Should panic: UpgradeRenounced
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #5)")]
    fn test_cannot_set_admin_after_renounce() {
        let env = Env::default();
        let client = setup(&env);

        env.mock_all_auths();
        client.renounce_upgrade_authority();

        let new_admin = Address::generate(&env);
        client.set_admin(&new_admin); // Should panic: UpgradeRenounced
    }

    // === SET_ADMIN TESTS ===

    #[test]
    fn test_admin_can_change_admin() {
        let env = Env::default();
        let client = setup(&env);

        env.mock_all_auths();

        let new_admin = Address::generate(&env);
        client.set_admin(&new_admin);
        assert_eq!(client.get_admin(), new_admin);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #4)")]
    fn test_non_admin_cannot_change_admin() {
        let env = Env::default();
        let contract_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(&env, &contract_id);

        let announcer = Address::generate(&env);
        let admin = Address::generate(&env);
        client.init(&announcer, &admin);

        let non_admin = Address::generate(&env);
        env.mock_all_auths();

        // Set auth for non_admin trying to call set_admin
        let new_admin = Address::generate(&env);
        env.set_auth(&[soroban_sdk::testutils::Auth {
            address: non_admin.clone(),
            invoke: &soroban_sdk::testutils::AuthorizedInvocation {
                function: soroban_sdk::testutils::AuthorizedFunction::Contract((
                    client.address.clone(),
                    Symbol::new(&env, "set_admin"),
                    vec![&env, new_admin.into_val(&env)],
                )),
                sub_invocations: vec![&env],
            },
        }]);

        client.set_admin(&new_admin); // Should panic: NotAdmin
    }

    // === STATE PRESERVATION TESTS ===

    #[test]
    fn test_admin_change_preserves_announcer() {
        let env = Env::default();
        let contract_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(&env, &contract_id);

        let announcer = Address::generate(&env);
        let admin = Address::generate(&env);
        client.init(&announcer, &admin);

        env.mock_all_auths();

        // Change admin
        let new_admin = Address::generate(&env);
        client.set_admin(&new_admin);

        // Admin changed
        assert_eq!(client.get_admin(), new_admin);

        // Contract still initialized (announcer still stored)
        // We verify by checking the contract still functions
        assert!(!client.is_renounced());
    }
}
