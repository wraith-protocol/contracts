#![no_std]

use soroban_sdk::{contract, contractimpl, symbol_short, Address, Bytes, BytesN, Env};

#[contract]
pub struct StealthAnnouncerContract;

#[contractimpl]
impl StealthAnnouncerContract {
    /// Emits a stealth address announcement event.
    ///
    /// This is a pure event-emission function with no access control and no
    /// storage. Indexers watch for these events to let recipients detect
    /// incoming payments.
    ///
    /// # Arguments
    /// * `scheme_id` - Identifier for the stealth address scheme (e.g. 1 for the default DKSAP scheme).
    /// * `stealth_address` - The one-time stealth address that received funds.
    /// * `ephemeral_pub_key` - The ephemeral public key used to derive the stealth address.
    /// * `metadata` - Arbitrary metadata (e.g. view tag) to speed up scanning.
    pub fn announce(
        env: Env,
        scheme_id: u32,
        stealth_address: Address,
        ephemeral_pub_key: BytesN<32>,
        metadata: Bytes,
    ) {
        env.events().publish(
            (symbol_short!("announce"), scheme_id, stealth_address),
            (env.current_contract_address(), ephemeral_pub_key, metadata),
        );
    }
}
