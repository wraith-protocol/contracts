#![no_std]

use core::convert::TryInto;

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes, BytesN,
    Env, String,
};
use soroban_sdk::xdr::{AccountId, PublicKey, ScAddress};

pub const WRAITH_NAMES_DOMAIN: &[u8] = b"wraith-names:v1";

/// Storage keys.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Maps name hash (BytesN<32>) to NameEntry.
    Name(BytesN<32>),
    /// Reverse lookup: meta-address hash (BytesN<32>) to name hash (BytesN<32>).
    Reverse(BytesN<32>),
    /// Replay protection for signed on-behalf calls.
    Replay(BytesN<32>),
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
    SignatureExpired = 8,
    SignatureReplay = 9,
    InvalidSigner = 10,
}

#[contract]
pub struct WraithNamesContract;

#[contractimpl]
impl WraithNamesContract {
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
        owner.require_auth();
        Self::register_internal(&env, owner, name, stealth_meta_address)
    }

    /// Register a name on behalf of an owner using a signed authorization.
    pub fn register_on_behalf(
        env: Env,
        owner: Address,
        name: String,
        stealth_meta_address: Bytes,
        signature: BytesN<64>,
        expiry: u64,
    ) -> Result<(), NamesError> {
        let replay_key = Self::verify_on_behalf_authorization(
            &env,
            &owner,
            b"wraith-names:register",
            &name,
            &stealth_meta_address,
            &signature,
            expiry,
        )?;
        Self::register_internal(&env, owner, name, stealth_meta_address)?;
        env.storage().instance().set(&DataKey::Replay(replay_key), &true);
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
        owner.require_auth();
        Self::update_internal(&env, owner, name, new_meta_address)
    }

    /// Update a name on behalf of an owner using a signed authorization.
    pub fn update_on_behalf(
        env: Env,
        owner: Address,
        name: String,
        new_meta_address: Bytes,
        signature: BytesN<64>,
        expiry: u64,
    ) -> Result<(), NamesError> {
        let replay_key = Self::verify_on_behalf_authorization(
            &env,
            &owner,
            b"wraith-names:update",
            &name,
            &new_meta_address,
            &signature,
            expiry,
        )?;
        Self::update_internal(&env, owner, name, new_meta_address)?;
        env.storage().instance().set(&DataKey::Replay(replay_key), &true);
        Ok(())
    }

    /// Release a name, making it available again.
    pub fn release(env: Env, owner: Address, name: String) -> Result<(), NamesError> {
        owner.require_auth();
        Self::release_internal(&env, owner, name)
    }

    /// Release a name on behalf of an owner using a signed authorization.
    pub fn release_on_behalf(
        env: Env,
        owner: Address,
        name: String,
        signature: BytesN<64>,
        expiry: u64,
    ) -> Result<(), NamesError> {
        let empty_meta = Bytes::new(&env);
        let replay_key = Self::verify_on_behalf_authorization(
            &env,
            &owner,
            b"wraith-names:release",
            &name,
            &empty_meta,
            &signature,
            expiry,
        )?;
        Self::release_internal(&env, owner, name)?;
        env.storage().instance().set(&DataKey::Replay(replay_key), &true);
        Ok(())
    }

    fn owner_public_key(env: &Env, owner: &Address) -> Result<BytesN<32>, NamesError> {
        let sc_address: ScAddress = owner
            .try_into()
            .map_err(|_| NamesError::InvalidSigner)?;

        match sc_address {
            ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(public_key))) => {
                let public_key_bytes: [u8; 32] = public_key
                    .as_slice()
                    .try_into()
                    .map_err(|_| NamesError::InvalidSigner)?;
                Ok(BytesN::from_array(env, &public_key_bytes))
            }
            _ => Err(NamesError::InvalidSigner),
        }
    }

    fn register_internal(
        env: &Env,
        owner: Address,
        name: String,
        stealth_meta_address: Bytes,
    ) -> Result<(), NamesError> {
        Self::validate_name(env, &name)?;
        if stealth_meta_address.len() != 64 {
            return Err(NamesError::InvalidMetaAddress);
        }

        let name_hash = Self::hash_name(env, &name);
        let name_key = DataKey::Name(name_hash.clone());

        if env.storage().instance().has(&name_key) {
            return Err(NamesError::NameTaken);
        }

        let entry = NameEntry {
            name: name.clone(),
            stealth_meta_address: stealth_meta_address.clone(),
            owner,
        };

        env.storage().instance().set(&name_key, &entry);

        let meta_hash = BytesN::from_array(env, &env.crypto().sha256(&stealth_meta_address).to_array());
        env.storage()
            .instance()
            .set(&DataKey::Reverse(meta_hash), &name_hash);

        env.events().publish(
            (symbol_short!("register"), name_hash),
            (name, stealth_meta_address),
        );

        Ok(())
    }

    fn update_internal(
        env: &Env,
        owner: Address,
        name: String,
        new_meta_address: Bytes,
    ) -> Result<(), NamesError> {
        if new_meta_address.len() != 64 {
            return Err(NamesError::InvalidMetaAddress);
        }

        let name_hash = Self::hash_name(env, &name);
        let name_key = DataKey::Name(name_hash.clone());

        let entry: NameEntry = env
            .storage()
            .instance()
            .get(&name_key)
            .ok_or(NamesError::NameNotFound)?;

        if entry.owner != owner {
            return Err(NamesError::NotOwner);
        }

        let old_meta_hash = BytesN::from_array(
            env,
            &env.crypto().sha256(&entry.stealth_meta_address).to_array(),
        );
        env.storage()
            .instance()
            .remove(&DataKey::Reverse(old_meta_hash));

        let new_entry = NameEntry {
            name: name.clone(),
            stealth_meta_address: new_meta_address.clone(),
            owner,
        };
        env.storage().instance().set(&name_key, &new_entry);

        let new_meta_hash = BytesN::from_array(env, &env.crypto().sha256(&new_meta_address).to_array());
        env.storage()
            .instance()
            .set(&DataKey::Reverse(new_meta_hash), &name_hash);

        env.events().publish(
            (symbol_short!("register"), name_hash),
            (name, new_meta_address),
        );

        Ok(())
    }

    fn release_internal(env: &Env, owner: Address, name: String) -> Result<(), NamesError> {
        let name_hash = Self::hash_name(env, &name);
        let name_key = DataKey::Name(name_hash.clone());

        let entry: NameEntry = env
            .storage()
            .instance()
            .get(&name_key)
            .ok_or(NamesError::NameNotFound)?;

        if entry.owner != owner {
            return Err(NamesError::NotOwner);
        }

        let meta_hash = BytesN::from_array(env, &env.crypto().sha256(&entry.stealth_meta_address).to_array());
        env.storage().instance().remove(&DataKey::Reverse(meta_hash));
        env.storage().instance().remove(&name_key);

        env.events()
            .publish((symbol_short!("release"), name_hash), name);

        Ok(())
    }

    fn verify_on_behalf_authorization(
        env: &Env,
        owner: &Address,
        operation: &[u8],
        name: &String,
        stealth_meta_address: &Bytes,
        signature: &BytesN<64>,
        expiry: u64,
    ) -> Result<BytesN<32>, NamesError> {
        let current_sequence = u64::from(env.ledger().sequence());
        if current_sequence >= expiry {
            return Err(NamesError::SignatureExpired);
        }

        let public_key = Self::owner_public_key(env, owner)?;
        let message = Self::authorization_message(
            env,
            operation,
            name,
            stealth_meta_address,
            expiry,
        );
        let message_hash = env.crypto().sha256(&message);

        let replay_key: BytesN<32> = message_hash.clone().into();

        if env.storage().instance().has(&DataKey::Replay(replay_key.clone())) {
            return Err(NamesError::SignatureReplay);
        }

        let digest = Bytes::from(message_hash.clone());
        env.crypto().ed25519_verify(&public_key, &digest, signature);

        Ok(replay_key)
    }

    /// Resolve a name to its stealth meta-address.
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

    fn authorization_message(
        env: &Env,
        operation: &[u8],
        name: &String,
        stealth_meta_address: &Bytes,
        expiry: u64,
    ) -> Bytes {
        let mut message = Bytes::from_slice(env, WRAITH_NAMES_DOMAIN);
        message.extend_from_slice(operation);
        let name_len = name.len() as usize;
        let mut name_buf = [0u8; 32];
        name.copy_into_slice(&mut name_buf[..name_len]);
        let name_bytes = Bytes::from_slice(env, &name_buf[..name_len]);
        message.append(&name_bytes);
        message.append(stealth_meta_address);
        message.extend_from_slice(&expiry.to_be_bytes());
        message
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
    use ed25519_dalek::SigningKey;
    use proptest::prelude::*;
    use soroban_sdk::TryFromVal;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::xdr::{AccountId, PublicKey, ScAddress, Uint256};
    use soroban_sdk::{Bytes, Env, String};

    fn signing_account(env: &Env, seed: [u8; 32]) -> (Address, SigningKey) {
        let signing_key = SigningKey::from_bytes(&seed);
        let public_key = Uint256::try_from(signing_key.verifying_key().to_bytes().as_ref())
            .expect("valid ed25519 key");
        let sc_address = ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(public_key)));
        let owner = Address::try_from_val(env, &sc_address).expect("account address");
        (owner, signing_key)
    }

    fn sign_authorization(
        env: &Env,
        signing_key: &SigningKey,
        operation: &[u8],
        name: &String,
        stealth_meta_address: &Bytes,
        expiry: u64,
    ) -> BytesN<64> {
        use ed25519_dalek::Signer;

        let message = WraithNamesContract::authorization_message(
            env,
            operation,
            name,
            stealth_meta_address,
            expiry,
        );
        let message_hash = env.crypto().sha256(&message);
        let message_bytes = message_hash.to_array();
        let signature = signing_key.sign(&message_bytes);
        BytesN::from_array(env, &signature.to_bytes())
    }

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
    fn test_register_on_behalf() {
        let env = Env::default();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let (owner, signing_key) = signing_account(&env, [7u8; 32]);
        let name = String::from_str(&env, "eve");
        let meta = Bytes::from_slice(&env, &[42u8; 64]);
        let expiry = u64::from(env.ledger().sequence()) + 10;
        let signature = sign_authorization(
            &env,
            &signing_key,
            b"wraith-names:register",
            &name,
            &meta,
            expiry,
        );

        client.register_on_behalf(&owner, &name, &meta, &signature, &expiry);

        let resolved = client.resolve(&name);
        assert_eq!(resolved, meta);
    }

    #[test]
    #[should_panic]
    fn test_register_on_behalf_wrong_signer_panics() {
        let env = Env::default();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let (owner, _) = signing_account(&env, [11u8; 32]);
        let (_, wrong_signing_key) = signing_account(&env, [22u8; 32]);
        let name = String::from_str(&env, "mallory");
        let meta = Bytes::from_slice(&env, &[5u8; 64]);
        let expiry = u64::from(env.ledger().sequence()) + 10;
        let signature = sign_authorization(
            &env,
            &wrong_signing_key,
            b"wraith-names:register",
            &name,
            &meta,
            expiry,
        );

        client.register_on_behalf(&owner, &name, &meta, &signature, &expiry);
    }

    #[test]
    fn test_register_on_behalf_expired() {
        let env = Env::default();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let (owner, signing_key) = signing_account(&env, [33u8; 32]);
        let name = String::from_str(&env, "trent");
        let meta = Bytes::from_slice(&env, &[8u8; 64]);
        let expiry = u64::from(env.ledger().sequence());
        let signature = sign_authorization(
            &env,
            &signing_key,
            b"wraith-names:register",
            &name,
            &meta,
            expiry,
        );

        let result = client.try_register_on_behalf(&owner, &name, &meta, &signature, &expiry);
        assert_eq!(result, Err(Ok(NamesError::SignatureExpired)));
    }

    #[test]
    fn test_register_on_behalf_replay() {
        let env = Env::default();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let (owner, signing_key) = signing_account(&env, [44u8; 32]);
        let name = String::from_str(&env, "victor");
        let meta = Bytes::from_slice(&env, &[9u8; 64]);
        let expiry = u64::from(env.ledger().sequence()) + 10;
        let signature = sign_authorization(
            &env,
            &signing_key,
            b"wraith-names:register",
            &name,
            &meta,
            expiry,
        );

        client.register_on_behalf(&owner, &name, &meta, &signature, &expiry);
        let result = client.try_register_on_behalf(&owner, &name, &meta, &signature, &expiry);
        assert_eq!(result, Err(Ok(NamesError::SignatureReplay)));
    }

    #[test]
    fn test_update_on_behalf_and_release_on_behalf() {
        let env = Env::default();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let (owner, signing_key) = signing_account(&env, [55u8; 32]);
        let name = String::from_str(&env, "wendy");
        let meta = Bytes::from_slice(&env, &[10u8; 64]);
        let expiry = u64::from(env.ledger().sequence()) + 10;
        let register_signature = sign_authorization(
            &env,
            &signing_key,
            b"wraith-names:register",
            &name,
            &meta,
            expiry,
        );

        client.register_on_behalf(&owner, &name, &meta, &register_signature, &expiry);

        let updated_meta = Bytes::from_slice(&env, &[11u8; 64]);
        let update_expiry = u64::from(env.ledger().sequence()) + 10;
        let update_signature = sign_authorization(
            &env,
            &signing_key,
            b"wraith-names:update",
            &name,
            &updated_meta,
            update_expiry,
        );
        client.update_on_behalf(&owner, &name, &updated_meta, &update_signature, &update_expiry);
        assert_eq!(client.resolve(&name), updated_meta);

        let release_expiry = u64::from(env.ledger().sequence()) + 10;
        let release_signature = sign_authorization(
            &env,
            &signing_key,
            b"wraith-names:release",
            &name,
            &Bytes::new(&env),
            release_expiry,
        );
        client.release_on_behalf(&owner, &name, &release_signature, &release_expiry);

        let result = client.try_resolve(&name);
        assert_eq!(result, Err(Ok(NamesError::NameNotFound)));
    }

    #[test]
    fn test_on_behalf_malformed_inputs() {
        let env = Env::default();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let (owner, signing_key) = signing_account(&env, [66u8; 32]);
        let name = String::from_str(&env, "zoe");
        let invalid_meta = Bytes::from_slice(&env, &[1u8; 63]);
        let expiry = u64::from(env.ledger().sequence()) + 10;
        let signature = sign_authorization(
            &env,
            &signing_key,
            b"wraith-names:register",
            &name,
            &invalid_meta,
            expiry,
        );

        let result = client.try_register_on_behalf(&owner, &name, &invalid_meta, &signature, &expiry);
        assert_eq!(result, Err(Ok(NamesError::InvalidMetaAddress)));
    }

    proptest! {
        #[test]
        fn prop_register_on_behalf_roundtrip(
            seed in any::<[u8; 32]>(),
            meta_seed in any::<[u8; 64]>(),
            name in proptest::string::string_regex("[a-z0-9]{3,32}").expect("valid name strategy"),
        ) {
            let env = Env::default();

            let contract_id = env.register(WraithNamesContract, ());
            let client = WraithNamesContractClient::new(&env, &contract_id);

            let (owner, signing_key) = signing_account(&env, seed);
            let name = String::from_str(&env, &name);
            let meta = Bytes::from_slice(&env, &meta_seed);
            let expiry = u64::from(env.ledger().sequence()) + 10;
            let signature = sign_authorization(
                &env,
                &signing_key,
                b"wraith-names:register",
                &name,
                &meta,
                expiry,
            );

            client.register_on_behalf(&owner, &name, &meta, &signature, &expiry);
            prop_assert_eq!(client.resolve(&name), meta);
        }
    }
}
