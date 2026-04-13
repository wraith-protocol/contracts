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
