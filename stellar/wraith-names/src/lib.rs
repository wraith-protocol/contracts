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
}

/// A registered name entry.
#[contracttype]
#[derive(Clone)]
pub struct NameEntry {
    /// The human-readable name.
    pub name: String,
    /// The 64-byte stealth meta-address (spending_pubkey || viewing_pubkey).
    pub stealth_meta_address: Bytes,
    /// The registrant address (for auth). For subdomains this is the parent
    /// owner's address captured at registration time; management is always
    /// re-checked against the current parent owner.
    pub owner: Address,
    /// For a subdomain (`sub.parent`), the name hash of the parent label.
    /// `None` for a flat top-level name. Existing flat names register with
    /// `None`, so prior behaviour is preserved.
    pub parent: Option<BytesN<32>>,
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
    /// More than one level of nesting was supplied (e.g. `a.b.c`).
    NameTooDeep = 8,
    /// A subdomain was used but its parent name is not registered.
    ParentNotFound = 9,
}

/// Maximum byte length of a fully-qualified name: `sub` (32) + `.` (1) +
/// `parent` (32).
const MAX_NAME_LEN: usize = 65;
/// Per-label bounds (applies to each dot-separated component).
const MIN_LABEL_LEN: usize = 3;
const MAX_LABEL_LEN: usize = 32;

#[contract]
pub struct WraithNamesContract;

#[contractimpl]
impl WraithNamesContract {
    /// Register a name mapped to a stealth meta-address.
    ///
    /// A flat name (`alice`) is owned by `owner`. A subdomain (`payments.alice`)
    /// may only be registered by the current owner of the parent name, and
    /// `owner` must equal that parent owner. At most one level of nesting is
    /// allowed.
    ///
    /// # Arguments
    /// * `owner` - The address registering the name (must authorize). For a
    ///   subdomain this must be the parent owner.
    /// * `name` - The human-readable name. A flat label, or `sub.parent` for a
    ///   subdomain. Each label is lowercase alphanumeric, 3-32 chars.
    /// * `stealth_meta_address` - 64-byte stealth meta-address.
    pub fn register(
        env: Env,
        owner: Address,
        name: String,
        stealth_meta_address: Bytes,
    ) -> Result<(), NamesError> {
        owner.require_auth();

        if stealth_meta_address.len() != 64 {
            return Err(NamesError::InvalidMetaAddress);
        }

        let (name_hash, parent_hash) = Self::parse_name(&env, &name)?;
        let name_key = DataKey::Name(name_hash.clone());

        // Check not taken
        if env.storage().instance().has(&name_key) {
            return Err(NamesError::NameTaken);
        }

        // Subdomains require an existing parent and that `owner` is the parent
        // owner.
        if let Some(ref ph) = parent_hash {
            let parent: NameEntry = env
                .storage()
                .instance()
                .get(&DataKey::Name(ph.clone()))
                .ok_or(NamesError::ParentNotFound)?;
            if parent.owner != owner {
                return Err(NamesError::NotOwner);
            }
        }

        let entry = NameEntry {
            name: name.clone(),
            stealth_meta_address: stealth_meta_address.clone(),
            owner: owner.clone(),
            parent: parent_hash,
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
    ///
    /// For a flat name only the current owner can update. For a subdomain only
    /// the current owner of the parent name can update.
    pub fn update(
        env: Env,
        owner: Address,
        name: String,
        new_meta_address: Bytes,
    ) -> Result<(), NamesError> {
        owner.require_auth();

        if new_meta_address.len() != 64 {
            return Err(NamesError::InvalidMetaAddress);
        }

        let (name_hash, _) = Self::parse_name(&env, &name)?;
        let name_key = DataKey::Name(name_hash.clone());

        let entry: NameEntry = env
            .storage()
            .instance()
            .get(&name_key)
            .ok_or(NamesError::NameNotFound)?;

        Self::require_manager(&env, &owner, &entry)?;

        // Remove old reverse
        let old_meta_hash = BytesN::from_array(
            &env,
            &env.crypto().sha256(&entry.stealth_meta_address).to_array(),
        );
        env.storage()
            .instance()
            .remove(&DataKey::Reverse(old_meta_hash));

        // Update (preserve parent linkage)
        let new_entry = NameEntry {
            name: name.clone(),
            stealth_meta_address: new_meta_address.clone(),
            owner,
            parent: entry.parent,
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
    ///
    /// For a flat name only the current owner can release. For a subdomain only
    /// the current owner of the parent name can release. Releasing a parent
    /// does not delete its subdomains, but they will no longer resolve (see
    /// `resolve`) because the parent no longer exists.
    pub fn release(env: Env, owner: Address, name: String) -> Result<(), NamesError> {
        owner.require_auth();

        let (name_hash, _) = Self::parse_name(&env, &name)?;
        let name_key = DataKey::Name(name_hash.clone());

        let entry: NameEntry = env
            .storage()
            .instance()
            .get(&name_key)
            .ok_or(NamesError::NameNotFound)?;

        Self::require_manager(&env, &owner, &entry)?;

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
    ///
    /// For a subdomain (`payments.alice`) resolution walks to the parent
    /// (`alice`): if the parent no longer exists the subdomain does not resolve.
    pub fn resolve(env: Env, name: String) -> Result<Bytes, NamesError> {
        let (name_hash, _) = Self::parse_name(&env, &name)?;
        let entry: NameEntry = env
            .storage()
            .instance()
            .get(&DataKey::Name(name_hash))
            .ok_or(NamesError::NameNotFound)?;

        if let Some(ref ph) = entry.parent {
            if !env.storage().instance().has(&DataKey::Name(ph.clone())) {
                return Err(NamesError::NameNotFound);
            }
        }

        Ok(entry.stealth_meta_address)
    }

    /// Reverse lookup: find the name for a given stealth meta-address.
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

    /// Authorize a management action (update/release) against `entry`.
    ///
    /// Flat names are managed by their own owner. Subdomains are managed by the
    /// current owner of the parent name, re-read from storage so that a parent
    /// ownership change is always respected.
    fn require_manager(env: &Env, caller: &Address, entry: &NameEntry) -> Result<(), NamesError> {
        match &entry.parent {
            None => {
                if &entry.owner != caller {
                    return Err(NamesError::NotOwner);
                }
            }
            Some(ph) => {
                let parent: NameEntry = env
                    .storage()
                    .instance()
                    .get(&DataKey::Name(ph.clone()))
                    .ok_or(NamesError::ParentNotFound)?;
                if &parent.owner != caller {
                    return Err(NamesError::NotOwner);
                }
            }
        }
        Ok(())
    }

    /// Parse and validate a (possibly hierarchical) name.
    ///
    /// Returns the storage hash of the full name and, for a subdomain, the
    /// storage hash of the parent label. A flat name returns `(hash, None)`;
    /// `sub.parent` returns `(hash(sub.parent), Some(hash(parent)))`.
    fn parse_name(
        env: &Env,
        name: &String,
    ) -> Result<(BytesN<32>, Option<BytesN<32>>), NamesError> {
        let len = name.len() as usize;
        if len < MIN_LABEL_LEN {
            return Err(NamesError::NameTooShort);
        }
        if len > MAX_NAME_LEN {
            return Err(NamesError::NameTooLong);
        }

        let mut buf = [0u8; MAX_NAME_LEN];
        name.copy_into_slice(&mut buf[..len]);

        let mut dot_pos: Option<usize> = None;
        let mut dot_count: u32 = 0;
        for i in 0..len {
            if buf[i] == b'.' {
                dot_count += 1;
                dot_pos = Some(i);
            }
        }

        match dot_pos {
            None => {
                Self::validate_label(&buf[..len])?;
                let full = Self::hash_bytes(env, &buf[..len]);
                Ok((full, None))
            }
            Some(pos) => {
                if dot_count > 1 {
                    return Err(NamesError::NameTooDeep);
                }
                // sub = buf[..pos], parent = buf[pos + 1..len]
                Self::validate_label(&buf[..pos])?;
                Self::validate_label(&buf[pos + 1..len])?;
                let full = Self::hash_bytes(env, &buf[..len]);
                let parent = Self::hash_bytes(env, &buf[pos + 1..len]);
                Ok((full, Some(parent)))
            }
        }
    }

    /// SHA-256 of raw bytes as a BytesN<32> storage key.
    fn hash_bytes(env: &Env, data: &[u8]) -> BytesN<32> {
        let bytes = Bytes::from_slice(env, data);
        BytesN::from_array(env, &env.crypto().sha256(&bytes).to_array())
    }

    /// Validate a single label: 3-32 chars, lowercase alphanumeric only.
    fn validate_label(label: &[u8]) -> Result<(), NamesError> {
        let len = label.len();
        if len < MIN_LABEL_LEN {
            return Err(NamesError::NameTooShort);
        }
        if len > MAX_LABEL_LEN {
            return Err(NamesError::NameTooLong);
        }
        for &c in label {
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

    #[test]
    fn test_register_and_resolve() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

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

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

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

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

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

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

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

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let meta = Bytes::from_slice(&env, &[1u8; 64]);

        // Too short
        let result = client.try_register(&owner, &String::from_str(&env, "ab"), &meta);
        assert_eq!(result, Err(Ok(NamesError::NameTooShort)));

        // Invalid chars
        let result = client.try_register(&owner, &String::from_str(&env, "Alice"), &meta);
        assert_eq!(result, Err(Ok(NamesError::InvalidNameCharacter)));
    }

    #[test]
    fn test_subdomain_register_and_resolve() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let parent = String::from_str(&env, "alice");
        let parent_meta = Bytes::from_slice(&env, &[42u8; 64]);
        client.register(&owner, &parent, &parent_meta);

        let sub = String::from_str(&env, "payments.alice");
        let sub_meta = Bytes::from_slice(&env, &[7u8; 64]);
        client.register(&owner, &sub, &sub_meta);

        // Subdomain resolves to its own record, parent is unchanged.
        assert_eq!(client.resolve(&sub), sub_meta);
        assert_eq!(client.resolve(&parent), parent_meta);
    }

    #[test]
    fn test_subdomain_requires_existing_parent() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let sub = String::from_str(&env, "team.ghost");
        let meta = Bytes::from_slice(&env, &[3u8; 64]);

        let result = client.try_register(&owner, &sub, &meta);
        assert_eq!(result, Err(Ok(NamesError::ParentNotFound)));
    }

    #[test]
    fn test_subdomain_permission_boundary() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let attacker = Address::generate(&env);
        let parent = String::from_str(&env, "alice");
        let parent_meta = Bytes::from_slice(&env, &[42u8; 64]);
        client.register(&owner, &parent, &parent_meta);

        // Non parent-owner cannot register a subdomain under alice.
        let sub = String::from_str(&env, "payments.alice");
        let sub_meta = Bytes::from_slice(&env, &[7u8; 64]);
        let result = client.try_register(&attacker, &sub, &sub_meta);
        assert_eq!(result, Err(Ok(NamesError::NotOwner)));

        // Parent owner registers it, attacker cannot update or release it.
        client.register(&owner, &sub, &sub_meta);
        let other_meta = Bytes::from_slice(&env, &[8u8; 64]);
        assert_eq!(client.try_update(&attacker, &sub, &other_meta), Err(Ok(NamesError::NotOwner)));
        assert_eq!(client.try_release(&attacker, &sub), Err(Ok(NamesError::NotOwner)));
    }

    #[test]
    fn test_subdomain_update_and_release_by_parent_owner() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let parent = String::from_str(&env, "alice");
        let parent_meta = Bytes::from_slice(&env, &[42u8; 64]);
        client.register(&owner, &parent, &parent_meta);

        let sub = String::from_str(&env, "payments.alice");
        let sub_meta = Bytes::from_slice(&env, &[7u8; 64]);
        client.register(&owner, &sub, &sub_meta);

        // Parent owner updates the subdomain.
        let new_meta = Bytes::from_slice(&env, &[9u8; 64]);
        client.update(&owner, &sub, &new_meta);
        assert_eq!(client.resolve(&sub), new_meta);

        // Parent owner releases the subdomain; it can be re-registered.
        client.release(&owner, &sub);
        assert_eq!(client.try_resolve(&sub), Err(Ok(NamesError::NameNotFound)));
        client.register(&owner, &sub, &sub_meta);
        assert_eq!(client.resolve(&sub), sub_meta);
    }

    #[test]
    fn test_subdomain_orphaned_when_parent_released() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let parent = String::from_str(&env, "alice");
        let parent_meta = Bytes::from_slice(&env, &[42u8; 64]);
        client.register(&owner, &parent, &parent_meta);

        let sub = String::from_str(&env, "payments.alice");
        let sub_meta = Bytes::from_slice(&env, &[7u8; 64]);
        client.register(&owner, &sub, &sub_meta);

        // Releasing the parent makes the subdomain stop resolving.
        client.release(&owner, &parent);
        assert_eq!(client.try_resolve(&sub), Err(Ok(NamesError::NameNotFound)));
    }

    #[test]
    fn test_name_too_deep() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let meta = Bytes::from_slice(&env, &[1u8; 64]);

        // Two levels of nesting are rejected.
        let result = client.try_register(&owner, &String::from_str(&env, "a.b.alice"), &meta);
        assert_eq!(result, Err(Ok(NamesError::NameTooDeep)));
    }
}
