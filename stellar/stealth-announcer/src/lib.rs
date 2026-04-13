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

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Events};
    use soroban_sdk::{vec, Address, Bytes, BytesN, Env, IntoVal, Val};

    #[test]
    fn test_announce_emits_event() {
        let env = Env::default();
        let contract_id = env.register(StealthAnnouncerContract, ());
        let client = StealthAnnouncerContractClient::new(&env, &contract_id);

        let stealth_address = Address::generate(&env);
        let ephemeral_pub_key = BytesN::from_array(&env, &[1u8; 32]);
        let metadata = Bytes::from_slice(&env, &[0u8; 1]);
        let scheme_id: u32 = 1;

        client.announce(&scheme_id, &stealth_address, &ephemeral_pub_key, &metadata);

        let events = env.events().all();
        assert_eq!(events.len(), 1);

        let event = events.last().unwrap();

        // Verify the event was published by the correct contract.
        assert_eq!(event.0, contract_id);

        // Verify topics: ("announce", scheme_id, stealth_address).
        let expected_topics: soroban_sdk::Vec<Val> = vec![
            &env,
            symbol_short!("announce").into_val(&env),
            scheme_id.into_val(&env),
            stealth_address.into_val(&env),
        ];
        assert_eq!(event.1, expected_topics);
    }

    #[test]
    fn test_announce_different_schemes() {
        let env = Env::default();
        let contract_id = env.register(StealthAnnouncerContract, ());
        let client = StealthAnnouncerContractClient::new(&env, &contract_id);

        let addr = Address::generate(&env);
        let epk = BytesN::from_array(&env, &[1u8; 32]);
        let meta = Bytes::from_slice(&env, &[0u8; 1]);

        // Announce with scheme_id = 1.
        client.announce(&1u32, &addr, &epk, &meta);
        let events = env.events().all();
        assert!(!events.is_empty());
        let event = events.last().unwrap();
        assert_eq!(event.0, contract_id.clone());

        let expected_topics: soroban_sdk::Vec<Val> = vec![
            &env,
            symbol_short!("announce").into_val(&env),
            1u32.into_val(&env),
            addr.clone().into_val(&env),
        ];
        assert_eq!(event.1, expected_topics);

        // Announce again with scheme_id = 2 — still works.
        client.announce(&2u32, &addr, &epk, &meta);
        let events2 = env.events().all();
        let event2 = events2.last().unwrap();
        let expected_topics2: soroban_sdk::Vec<Val> = vec![
            &env,
            symbol_short!("announce").into_val(&env),
            2u32.into_val(&env),
            addr.into_val(&env),
        ];
        assert_eq!(event2.1, expected_topics2);
    }
}
