#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes, Env,
};

/// Storage keys.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Maps (registrant, scheme_id) to their stealth meta-address (64 bytes:
    /// spending_pubkey || viewing_pubkey).
    MetaAddress(Address, u32),
}

/// Errors that the registry can produce.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RegistryError {
    /// The supplied stealth meta-address is not exactly 64 bytes.
    InvalidMetaAddressLength = 1,
    /// No stealth meta-address has been registered for the given address and scheme.
    NotRegistered = 2,
}

#[contract]
pub struct StealthRegistryContract;

#[contractimpl]
impl StealthRegistryContract {
    /// Register or update a stealth meta-address.
    ///
    /// # Arguments
    /// * `registrant` - The address whose meta-address is being set (must authorise).
    /// * `scheme_id`  - The stealth address scheme identifier.
    /// * `stealth_meta_address` - 64-byte value: `spending_pubkey || viewing_pubkey`.
    pub fn register_keys(
        env: Env,
        registrant: Address,
        scheme_id: u32,
        stealth_meta_address: Bytes,
    ) -> Result<(), RegistryError> {
        // Require authorisation from the registrant.
        registrant.require_auth();

        // Validate length.
        if stealth_meta_address.len() != 64 {
            return Err(RegistryError::InvalidMetaAddressLength);
        }

        // Persist.
        let key = DataKey::MetaAddress(registrant.clone(), scheme_id);
        env.storage().instance().set(&key, &stealth_meta_address);

        // Emit event.
        env.events().publish(
            (symbol_short!("register"), registrant, scheme_id),
            stealth_meta_address,
        );

        Ok(())
    }

    /// Look up a previously registered stealth meta-address.
    ///
    /// # Arguments
    /// * `registrant` - The address to look up.
    /// * `scheme_id`  - The stealth address scheme identifier.
    pub fn stealth_meta_address_of(
        env: Env,
        registrant: Address,
        scheme_id: u32,
    ) -> Result<Bytes, RegistryError> {
        let key = DataKey::MetaAddress(registrant, scheme_id);
        env.storage()
            .instance()
            .get(&key)
            .ok_or(RegistryError::NotRegistered)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Events};
    use soroban_sdk::{vec, Bytes, Env, IntoVal, Val};

    #[test]
    fn test_register_and_lookup() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(StealthRegistryContract, ());
        let client = StealthRegistryContractClient::new(&env, &contract_id);

        let registrant = Address::generate(&env);
        let scheme_id: u32 = 1;
        let meta_address = Bytes::from_slice(&env, &[42u8; 64]);

        client.register_keys(&registrant, &scheme_id, &meta_address);

        // Verify event was emitted by the right contract with correct topics.
        let events = env.events().all();
        assert!(!events.is_empty());

        let event = events.last().unwrap();
        assert_eq!(event.0, contract_id);

        let expected_topics: soroban_sdk::Vec<Val> = vec![
            &env,
            symbol_short!("register").into_val(&env),
            registrant.clone().into_val(&env),
            scheme_id.into_val(&env),
        ];
        assert_eq!(event.1, expected_topics);

        // Verify the stored value matches.
        let stored = client.stealth_meta_address_of(&registrant, &scheme_id);
        assert_eq!(stored, meta_address);
    }

    #[test]
    fn test_register_rejects_wrong_length() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(StealthRegistryContract, ());
        let client = StealthRegistryContractClient::new(&env, &contract_id);

        let registrant = Address::generate(&env);
        let scheme_id: u32 = 1;
        let bad_meta = Bytes::from_slice(&env, &[0u8; 32]); // 32 bytes, not 64

        let result = client.try_register_keys(&registrant, &scheme_id, &bad_meta);
        assert_eq!(result, Err(Ok(RegistryError::InvalidMetaAddressLength)));
    }

    #[test]
    fn test_lookup_not_registered() {
        let env = Env::default();

        let contract_id = env.register(StealthRegistryContract, ());
        let client = StealthRegistryContractClient::new(&env, &contract_id);

        let registrant = Address::generate(&env);
        let scheme_id: u32 = 1;

        let result = client.try_stealth_meta_address_of(&registrant, &scheme_id);
        assert_eq!(result, Err(Ok(RegistryError::NotRegistered)));
    }

    #[test]
    fn test_update_existing_registration() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(StealthRegistryContract, ());
        let client = StealthRegistryContractClient::new(&env, &contract_id);

        let registrant = Address::generate(&env);
        let scheme_id: u32 = 1;

        let meta_v1 = Bytes::from_slice(&env, &[1u8; 64]);
        client.register_keys(&registrant, &scheme_id, &meta_v1);
        assert_eq!(
            client.stealth_meta_address_of(&registrant, &scheme_id),
            meta_v1
        );

        // Update to a new meta-address.
        let meta_v2 = Bytes::from_slice(&env, &[2u8; 64]);
        client.register_keys(&registrant, &scheme_id, &meta_v2);
        assert_eq!(
            client.stealth_meta_address_of(&registrant, &scheme_id),
            meta_v2
        );
    }
}
