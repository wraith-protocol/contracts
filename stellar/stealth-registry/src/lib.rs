#![no_std]

#[cfg(kani)]
extern crate alloc;

#[cfg(not(kani))]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes, Env,
    IntoVal, Vec,
};
#[cfg(not(kani))]
use wraith_metrics::{contract_ids, dimension_names, emit_metric, metric_names};

#[cfg(kani)]
pub mod mock_sdk;

#[cfg(kani)]
pub mod soroban_sdk {
    pub use crate::mock_sdk::*;
    pub use crate::mock_symbol_short as symbol_short;
    pub use crate::mock_vec as vec;
}

#[cfg(kani)]
pub mod wraith_metrics {
    pub use crate::mock_sdk::contract_ids;
    pub use crate::mock_sdk::dimension_names;
    pub use crate::mock_sdk::emit_metric;
    pub use crate::mock_sdk::metric_names;
}

#[cfg(kani)]
#[allow(unused_imports)]
use mock_sdk::{
    contract_ids, dimension_names, emit_metric, metric_names, Address, Bytes, DataKey, Env, IntoVal,
};
#[cfg(kani)]
use soroban_sdk::symbol_short;

#[cfg(kani)]
mod proofs;

/// Storage keys.
#[cfg(not(kani))]
#[contracttype]
#[derive(Clone, PartialEq, Eq)]
pub enum DataKey {
    /// Maps (registrant, scheme_id) to their stealth meta-address (64 bytes:
    /// spending_pubkey || viewing_pubkey).
    MetaAddress(Address, u32),
}

/// Errors that the registry can produce.
#[cfg_attr(not(kani), contracterror)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RegistryError {
    /// The supplied stealth meta-address is not exactly 64 bytes.
    InvalidMetaAddressLength = 1,
    /// No stealth meta-address has been registered for the given address and scheme.
    NotRegistered = 2,
}

const TTL_THRESHOLD: u32 = 17280; // ~1 day
const TTL_EXTEND_TO: u32 = 518400; // ~30 days

#[cfg_attr(not(kani), contract)]
pub struct StealthRegistryContract;

#[cfg_attr(not(kani), contractimpl)]
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

        // Persist using persistent storage to handle large number of users.
        let key = DataKey::MetaAddress(registrant.clone(), scheme_id);
        env.storage().persistent().set(&key, &stealth_meta_address);

        // Extend TTLs
        Self::extend_ttls(&env, &key);

        // Emit event.
        env.events().publish(
            (symbol_short!("register"), registrant, scheme_id),
            stealth_meta_address,
        );

        // Emit metric event.
        emit_metric(
            &env,
            contract_ids::STEALTH_REGISTRY,
            metric_names::REGISTER_COUNT,
            1,
            soroban_sdk::vec![&env, (dimension_names::SCHEME_ID, scheme_id.into_val(&env))],
        );

        Ok(())
    }

    /// Remove a previously registered stealth meta-address.
    ///
    /// # Arguments
    /// * `registrant` - The address whose meta-address is being removed (must authorise).
    /// * `scheme_id`  - The stealth address scheme identifier.
    pub fn remove_keys(env: Env, registrant: Address, scheme_id: u32) -> Result<(), RegistryError> {
        // Require authorisation from the registrant.
        registrant.require_auth();

        let key = DataKey::MetaAddress(registrant.clone(), scheme_id);
        if !env.storage().persistent().has(&key) {
            return Err(RegistryError::NotRegistered);
        }

        env.storage().persistent().remove(&key);

        // Emit event.
        env.events()
            .publish((symbol_short!("remove"), registrant, scheme_id), ());

        // Emit metric event.
        emit_metric(
            &env,
            contract_ids::STEALTH_REGISTRY,
            metric_names::REMOVE_COUNT,
            1,
            soroban_sdk::vec![&env, (dimension_names::SCHEME_ID, scheme_id.into_val(&env))],
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

        let val = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(RegistryError::NotRegistered);

        if val.is_ok() {
            Self::extend_ttls(&env, &key);
        }

        val
    }

    /// Private helper to extend TTLs for both the persistent entry and the contract instance.
    fn extend_ttls(env: &Env, key: &DataKey) {
        env.storage()
            .persistent()
            .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
    }
}
