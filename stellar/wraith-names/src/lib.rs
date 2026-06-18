#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes, BytesN, Env,
    String,
};

/// Storage keys.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Maps name hash (BytesN<32>) to NameEntry.
    Name(BytesN<32>),
    /// Reverse lookup: meta-address hash (BytesN<32>) to name hash (BytesN<32>).
    Reverse(BytesN<32>),
    /// The admin address authorized for pause/unpause.
    Admin,
    /// Whether the contract is paused.
    Paused,
}

/// A registered name entry.
#[contracttype]
#[derive(Clone)]
pub struct NameEntry {
    /// The human-readable name.
    pub name: String,
    /// The 64-byte stealth meta-address (spending_pubkey || viewing_pubkey).
    pub stealth_meta_address: Bytes,
    /// The registrant address (for auth).
    pub owner: Address,
}

/// Errors.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum NamesError {
    NameTaken = 1,
    NameTooShort = 2,
    NameTooLong = 3,
    InvalidNameCharacter = 4,
    InvalidMetaAddress = 5,
    NameNotFound = 6,
    NotOwner = 7,
    /// Caller is not the admin.
    NotAdmin = 8,
    /// Contract is paused.
    ContractPaused = 9,
    /// Admin has already been set.
    AdminAlreadySet = 10,
}

#[contract]
pub struct WraithNamesContract;

#[contractimpl]
impl WraithNamesContract {
    /// Initialise the contract with an admin address for pause control.
    /// Must be called once before use.
    pub fn init(env: Env, admin: Address) -> Result<(), NamesError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(NamesError::AdminAlreadySet);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Paused, &false);
        Ok(())
    }

    /// Pause the contract. Only callable by admin.
    /// When paused, register/update/release are blocked.
    /// Resolve and name_of remain available (read-only).
    pub fn pause(env: Env) -> Result<(), NamesError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(NamesError::NotAdmin)?;
        admin.require_auth();

        env.storage().instance().set(&DataKey::Paused, &true);
        env.events()
            .publish((symbol_short!("paused"),), ());
        Ok(())
    }

    /// Unpause the contract. Only callable by admin.
    pub fn unpause(env: Env) -> Result<(), NamesError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(NamesError::NotAdmin)?;
        admin.require_auth();

        env.storage().instance().set(&DataKey::Paused, &false);
        env.events()
            .publish((symbol_short!("unpaused"),), ());
        Ok(())
    }

    /// Check if the contract is paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    /// Internal: revert if paused.
    fn require_not_paused(env: &Env) -> Result<(), NamesError> {
        let paused: bool = env
            .storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false);
        if paused {
            return Err(NamesError::ContractPaused);
        }
        Ok(())
    }

    /// Register a name mapped to a stealth meta-address.
    /// The caller (owner) must authorize. Ownership is tied to the caller's address.
    ///
    /// # Arguments
    /// * `owner` - The address registering the name (must authorize).
    /// * `name` - The human-readable name (lowercase alphanumeric, 3-32 chars).
    /// * `stealth_meta_address` - 64-byte stealth meta-address.
    pub fn register(
        env: Env,
        owner: Address,
        name: String,
        stealth_meta_address: Bytes,
    ) -> Result<(), NamesError> {
        Self::require_not_paused(&env)?;
        owner.require_auth();

        Self::validate_name(&env, &name)?;
        if stealth_meta_address.len() != 64 {
            return Err(NamesError::InvalidMetaAddress);
        }

        let name_hash = Self::hash_name(&env, &name);
        let name_key = DataKey::Name(name_hash.clone());

        // Check not taken
        if env.storage().instance().has(&name_key) {
            return Err(NamesError::NameTaken);
        }

        let entry = NameEntry {
            name: name.clone(),
            stealth_meta_address: stealth_meta_address.clone(),
            owner: owner.clone(),
        };

        env.storage().instance().set(&name_key, &entry);

        // Reverse lookup
        let meta_hash =
            BytesN::from_array(&env, &env.crypto().sha256(&stealth_meta_address).to_array());
        env.storage()
            .instance()
            .set(&DataKey::Reverse(meta_hash), &name_hash);

        env.events().publish(
            (symbol_short!("register"), name_hash),
            (name, stealth_meta_address),
        );

        Ok(())
    }

    /// Update the meta-address for an existing name.
    /// Only the current owner can update.
    pub fn update(
        env: Env,
        owner: Address,
        name: String,
        new_meta_address: Bytes,
    ) -> Result<(), NamesError> {
        Self::require_not_paused(&env)?;
        owner.require_auth();

        if new_meta_address.len() != 64 {
            return Err(NamesError::InvalidMetaAddress);
        }

        let name_hash = Self::hash_name(&env, &name);
        let name_key = DataKey::Name(name_hash.clone());

        let entry: NameEntry = env
            .storage()
            .instance()
            .get(&name_key)
            .ok_or(NamesError::NameNotFound)?;

        if entry.owner != owner {
            return Err(NamesError::NotOwner);
        }

        // Remove old reverse
        let old_meta_hash = BytesN::from_array(
            &env,
            &env.crypto().sha256(&entry.stealth_meta_address).to_array(),
        );
        env.storage()
            .instance()
            .remove(&DataKey::Reverse(old_meta_hash));

        // Update
        let new_entry = NameEntry {
            name: name.clone(),
            stealth_meta_address: new_meta_address.clone(),
            owner,
        };
        env.storage().instance().set(&name_key, &new_entry);

        // New reverse
        let new_meta_hash =
            BytesN::from_array(&env, &env.crypto().sha256(&new_meta_address).to_array());
        env.storage()
            .instance()
            .set(&DataKey::Reverse(new_meta_hash), &name_hash);

        env.events().publish(
            (symbol_short!("register"), name_hash),
            (name, new_meta_address),
        );

        Ok(())
    }

    /// Release a name, making it available again.
    pub fn release(env: Env, owner: Address, name: String) -> Result<(), NamesError> {
        Self::require_not_paused(&env)?;
        owner.require_auth();

        let name_hash = Self::hash_name(&env, &name);
        let name_key = DataKey::Name(name_hash.clone());

        let entry: NameEntry = env
            .storage()
            .instance()
            .get(&name_key)
            .ok_or(NamesError::NameNotFound)?;

        if entry.owner != owner {
            return Err(NamesError::NotOwner);
        }

        // Remove reverse
        let meta_hash = BytesN::from_array(
            &env,
            &env.crypto().sha256(&entry.stealth_meta_address).to_array(),
        );
        env.storage()
            .instance()
            .remove(&DataKey::Reverse(meta_hash));

        // Remove name
        env.storage().instance().remove(&name_key);

        env.events()
            .publish((symbol_short!("release"), name_hash), name);

        Ok(())
    }

    /// Resolve a name to its stealth meta-address.
    /// Available even when paused (read-only).
    pub fn resolve(env: Env, name: String) -> Result<Bytes, NamesError> {
        let name_hash = Self::hash_name(&env, &name);
        let entry: NameEntry = env
            .storage()
            .instance()
            .get(&DataKey::Name(name_hash))
            .ok_or(NamesError::NameNotFound)?;
        Ok(entry.stealth_meta_address)
    }

    /// Reverse lookup: find the name for a given stealth meta-address.
    /// Available even when paused (read-only).
    pub fn name_of(env: Env, stealth_meta_address: Bytes) -> Result<String, NamesError> {
        let meta_hash =
            BytesN::from_array(&env, &env.crypto().sha256(&stealth_meta_address).to_array());
        let name_hash: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::Reverse(meta_hash))
            .ok_or(NamesError::NameNotFound)?;
        let entry: NameEntry = env
            .storage()
            .instance()
            .get(&DataKey::Name(name_hash))
            .ok_or(NamesError::NameNotFound)?;
        Ok(entry.name)
    }

    /// Hash a name string to BytesN<32> for use as storage key.
    fn hash_name(env: &Env, name: &String) -> BytesN<32> {
        let len = name.len() as usize;
        let mut buf = [0u8; 32];
        if len > 0 {
            name.copy_into_slice(&mut buf[..len]);
        }
        let bytes = Bytes::from_slice(env, &buf[..len]);
        BytesN::from_array(env, &env.crypto().sha256(&bytes).to_array())
    }

    /// Validate name: 3-32 chars, lowercase alphanumeric only.
    fn validate_name(_env: &Env, name: &String) -> Result<(), NamesError> {
        let len = name.len() as usize;
        if len < 3 {
            return Err(NamesError::NameTooShort);
        }
        if len > 32 {
            return Err(NamesError::NameTooLong);
        }

        let mut buf = [0u8; 32];
        name.copy_into_slice(&mut buf[..len]);
        for i in 0..len {
            let c = buf[i];
            let is_lower = c >= b'a' && c <= b'z';
            let is_digit = c >= b'0' && c <= b'9';
            if !is_lower && !is_digit {
                return Err(NamesError::InvalidNameCharacter);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{Bytes, Env, String};

    fn setup_with_admin<'a>(env: &Env) -> (WraithNamesContractClient<'a>, Address) {
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(env, &contract_id);
        let admin = Address::generate(env);
        client.init(&admin);
        (client, admin)
    }

    // === PAUSE TESTS ===

    #[test]
    fn test_admin_can_pause() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _admin) = setup_with_admin(&env);

        assert!(!client.is_paused());
        client.pause();
        assert!(client.is_paused());
    }

    #[test]
    fn test_admin_can_unpause() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _admin) = setup_with_admin(&env);

        client.pause();
        assert!(client.is_paused());
        client.unpause();
        assert!(!client.is_paused());
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #9)")]
    fn test_register_blocked_when_paused() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _admin) = setup_with_admin(&env);

        client.pause();

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "alice");
        let meta = Bytes::from_slice(&env, &[42u8; 64]);
        client.register(&owner, &name, &meta); // Should panic: ContractPaused
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #9)")]
    fn test_update_blocked_when_paused() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _admin) = setup_with_admin(&env);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "alice");
        let meta = Bytes::from_slice(&env, &[42u8; 64]);
        client.register(&owner, &name, &meta);

        client.pause();

        let new_meta = Bytes::from_slice(&env, &[99u8; 64]);
        client.update(&owner, &name, &new_meta); // Should panic: ContractPaused
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #9)")]
    fn test_release_blocked_when_paused() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _admin) = setup_with_admin(&env);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "alice");
        let meta = Bytes::from_slice(&env, &[42u8; 64]);
        client.register(&owner, &name, &meta);

        client.pause();

        client.release(&owner, &name); // Should panic: ContractPaused
    }

    #[test]
    fn test_resolve_works_when_paused() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _admin) = setup_with_admin(&env);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "alice");
        let meta = Bytes::from_slice(&env, &[42u8; 64]);
        client.register(&owner, &name, &meta);

        client.pause();

        // Read-only operations still work
        let resolved = client.resolve(&name);
        assert_eq!(resolved, meta);
    }

    #[test]
    fn test_name_of_works_when_paused() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _admin) = setup_with_admin(&env);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "alice");
        let meta = Bytes::from_slice(&env, &[42u8; 64]);
        client.register(&owner, &name, &meta);

        client.pause();

        let found = client.name_of(&meta);
        assert_eq!(found, name);
    }

    #[test]
    fn test_register_works_after_unpause() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _admin) = setup_with_admin(&env);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "alice");
        let meta = Bytes::from_slice(&env, &[42u8; 64]);

        client.pause();
        client.unpause();

        client.register(&owner, &name, &meta);
        assert_eq!(client.resolve(&name), meta);
    }

    // === EXISTING TESTS (with admin init) ===

    #[test]
    fn test_register_and_resolve() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _admin) = setup_with_admin(&env);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "alice");
        let meta = Bytes::from_slice(&env, &[42u8; 64]);

        client.register(&owner, &name, &meta);

        let resolved = client.resolve(&name);
        assert_eq!(resolved, meta);
    }

    #[test]
    fn test_name_taken() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _admin) = setup_with_admin(&env);

        let owner1 = Address::generate(&env);
        let owner2 = Address::generate(&env);
        let name = String::from_str(&env, "bob");
        let meta1 = Bytes::from_slice(&env, &[1u8; 64]);
        let meta2 = Bytes::from_slice(&env, &[2u8; 64]);

        client.register(&owner1, &name, &meta1);
        let result = client.try_register(&owner2, &name, &meta2);
        assert_eq!(result, Err(Ok(NamesError::NameTaken)));
    }

    #[test]
    fn test_name_of_reverse() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _admin) = setup_with_admin(&env);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "charlie");
        let meta = Bytes::from_slice(&env, &[99u8; 64]);

        client.register(&owner, &name, &meta);

        let found_name = client.name_of(&meta);
        assert_eq!(found_name, name);
    }

    #[test]
    fn test_release() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _admin) = setup_with_admin(&env);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "dave");
        let meta = Bytes::from_slice(&env, &[88u8; 64]);

        client.register(&owner, &name, &meta);
        client.release(&owner, &name);

        let result = client.try_resolve(&name);
        assert_eq!(result, Err(Ok(NamesError::NameNotFound)));

        // Can re-register after release
        let owner2 = Address::generate(&env);
        let meta2 = Bytes::from_slice(&env, &[77u8; 64]);
        client.register(&owner2, &name, &meta2);
        assert_eq!(client.resolve(&name), meta2);
    }

    #[test]
    fn test_invalid_name() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _admin) = setup_with_admin(&env);

        let owner = Address::generate(&env);
        let meta = Bytes::from_slice(&env, &[1u8; 64]);

        // Too short
        let result = client.try_register(&owner, &String::from_str(&env, "ab"), &meta);
        assert_eq!(result, Err(Ok(NamesError::NameTooShort)));

        // Invalid chars
        let result = client.try_register(&owner, &String::from_str(&env, "Alice"), &meta);
        assert_eq!(result, Err(Ok(NamesError::InvalidNameCharacter)));
    }
}
