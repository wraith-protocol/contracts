#![no_std]

use soroban_sdk::{contract, contractimpl, symbol_short, Address, Bytes, BytesN, Env};

/// Stellar v2 deployment scheme id.
///
/// The v1 Stellar announcer emitted topics as
/// `(announce, scheme_id, stealth_address)` and data as
/// `(caller, ephemeral_pub_key, metadata)`. That historical shape is not
/// rewritten in place because existing indexers may already rely on it. The v2
/// rollout is a new announcer deployment that only accepts `scheme_id = 2` and
/// emits the bucketed event shape documented below.
pub const STELLAR_V2_SCHEME_ID: u32 = 2;

/// Initial metadata kind for v2 announcements.
///
/// `1` means `metadata[0]` is the one-byte view tag used for pre-filtering and
/// the remaining bytes, if any, are scheme-specific metadata. Future metadata
/// encodings must reserve a new `metadata_kind` value instead of changing this
/// interpretation.
pub const METADATA_KIND_VIEW_TAG: u32 = 1;

/// Derives the indexed view-tag bucket for v2 announcement topics.
///
/// The bucket is exactly the first metadata byte interpreted as an unsigned
/// integer in `[0, 255]`. Because `METADATA_KIND_VIEW_TAG` commits to the first
/// byte being present, callers must provide non-empty metadata.
pub fn view_tag_bucket(metadata: &Bytes) -> u32 {
    metadata.get(0).expect("metadata must include view tag") as u32
}

#[contract]
pub struct StealthAnnouncerContract;

#[contractimpl]
impl StealthAnnouncerContract {
    /// Emits a Stellar v2 stealth address announcement event.
    ///
    /// This is a pure event-emission function with no access control and no
    /// storage. Indexers watch for these events to let recipients detect
    /// incoming payments.
    ///
    /// v2 event shape:
    /// * topics: `("announce", scheme_id, view_tag_bucket, metadata_kind)`
    /// * data: `(stealth_address, ephemeral_pub_key, metadata)`
    ///
    /// The stable `view_tag_bucket` derivation is `metadata[0] as u32`, where
    /// `metadata_kind = 1` (`METADATA_KIND_VIEW_TAG`) means the first metadata
    /// byte is the view tag and the remaining bytes are scheme-specific. This
    /// lets wallets and indexers filter Stellar RPC `getEvents` by scheme and
    /// bucket before doing client-side cryptographic validation.
    ///
    /// Migration note: v1 announcements used the old Stellar layout
    /// `("announce", scheme_id, stealth_address)` with
    /// `(caller, ephemeral_pub_key, metadata)`. Do not reinterpret historical v1
    /// events as v2. The compatibility path is a new announcer deployment using
    /// `scheme_id = 2`.
    ///
    /// # Arguments
    /// * `scheme_id` - Must be `2` for the v2 Stellar announcer deployment.
    /// * `stealth_address` - The one-time stealth address that received funds.
    /// * `ephemeral_pub_key` - The ephemeral public key used to derive the stealth address.
    /// * `metadata` - Non-empty metadata whose first byte is the view tag.
    pub fn announce(
        env: Env,
        scheme_id: u32,
        stealth_address: Address,
        ephemeral_pub_key: BytesN<32>,
        metadata: Bytes,
    ) {
        assert_eq!(scheme_id, STELLAR_V2_SCHEME_ID);

        let view_tag_bucket = view_tag_bucket(&metadata);
        let metadata_kind = METADATA_KIND_VIEW_TAG;

        env.events().publish(
            (
                symbol_short!("announce"),
                scheme_id,
                view_tag_bucket,
                metadata_kind,
            ),
            (stealth_address, ephemeral_pub_key, metadata),
        );

        // Also emit a legacy v1-shaped announcement for backward
        // compatibility. Many existing indexers expect the older three-topic
        // layout `("announce", scheme_id, stealth_address)` with data
        // `(caller, ephemeral_pub_key, metadata)`. To avoid breaking those
        // indexers during migration, emit the legacy shape in addition to
        // the v2 authoritative shape. The `caller` value is the announcer
        // contract address (Soroban semantics), matching historical
        // behavior documented in audits.
        env.events().publish(
            (symbol_short!("announce"), scheme_id, stealth_address),
            (env.current_contract_address(), ephemeral_pub_key, metadata),
        );
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::{Address as _, EnvTestConfig, Events};
    use soroban_sdk::{vec, Address, Bytes, BytesN, Env, FromVal, IntoVal, Val};

    #[test]
    fn test_announce_emits_event() {
        let env = Env::default();
        let contract_id = env.register(StealthAnnouncerContract, ());
        let client = StealthAnnouncerContractClient::new(&env, &contract_id);

        let stealth_address = Address::generate(&env);
        let ephemeral_pub_key = BytesN::from_array(&env, &[1u8; 32]);
        let metadata = Bytes::from_slice(&env, &[42u8, 7u8]);
        let scheme_id: u32 = STELLAR_V2_SCHEME_ID;

        client.announce(&scheme_id, &stealth_address, &ephemeral_pub_key, &metadata);

        let events = env.events().all();
        // We now emit both v2 (4-topic) and legacy v1-shaped (3-topic) events.
        assert_eq!(events.len(), 2);

        // Locate v2 and v1 events by topic length.
        let mut found_v2 = false;
        let mut found_v1 = false;
        for e in events.iter() {
            // Verify the event was published by the correct contract.
            assert_eq!(e.0, contract_id);
            let topics_len = e.1.len();
            if topics_len == 4 {
                // v2 event
                let expected_topics: soroban_sdk::Vec<Val> = vec![
                    &env,
                    symbol_short!("announce").into_val(&env),
                    scheme_id.into_val(&env),
                    42u32.into_val(&env),
                    METADATA_KIND_VIEW_TAG.into_val(&env),
                ];
                assert_eq!(e.1, expected_topics);

                let actual_value: (Address, BytesN<32>, Bytes) = FromVal::from_val(&env, &e.2);
                assert_eq!(actual_value, (stealth_address, ephemeral_pub_key, metadata));
                found_v2 = true;
            } else if topics_len == 3 {
                // legacy v1-shaped event: topics ("announce", scheme_id, stealth_address)
                let expected_topics_v1: soroban_sdk::Vec<Val> = vec![
                    &env,
                    symbol_short!("announce").into_val(&env),
                    scheme_id.into_val(&env),
                    stealth_address.clone().into_val(&env),
                ];
                assert_eq!(e.1, expected_topics_v1);

                let actual_value_v1: (Address, BytesN<32>, Bytes) = FromVal::from_val(&env, &e.2);
                // Caller in Soroban historical behavior is the contract address.
                assert_eq!(actual_value_v1.1, ephemeral_pub_key);
                assert_eq!(actual_value_v1.2, metadata);
                found_v1 = true;
            }
        }

        assert!(found_v2 && found_v1, "Both v2 and legacy v1 events should be emitted");
    }

    #[test]
    fn test_view_tag_bucket_derives_from_first_metadata_byte() {
        let env = Env::default();
        let contract_id = env.register(StealthAnnouncerContract, ());
        let client = StealthAnnouncerContractClient::new(&env, &contract_id);

        let addr = Address::generate(&env);
        let epk = BytesN::from_array(&env, &[1u8; 32]);
        let first_meta = Bytes::from_slice(&env, &[0u8, 99u8]);
        let second_meta = Bytes::from_slice(&env, &[255u8, 99u8]);

        client.announce(&STELLAR_V2_SCHEME_ID, &addr, &epk, &first_meta);
        let events = env.events().all();
        // find the most recent v2 event and validate its topics
        let v2_event = events
            .iter()
            .rev()
            .find(|e| e.1.len() == 4)
            .expect("v2 event must be present");
        assert_eq!(v2_event.0, contract_id.clone());

        let expected_topics: soroban_sdk::Vec<Val> = vec![
            &env,
            symbol_short!("announce").into_val(&env),
            STELLAR_V2_SCHEME_ID.into_val(&env),
            0u32.into_val(&env),
            METADATA_KIND_VIEW_TAG.into_val(&env),
        ];
        assert_eq!(v2_event.1, expected_topics);

        client.announce(&STELLAR_V2_SCHEME_ID, &addr, &epk, &second_meta);
        let events2 = env.events().all();
        let v2_event2 = events2
            .iter()
            .rev()
            .find(|e| e.1.len() == 4)
            .expect("second v2 event must be present");
        let expected_topics2: soroban_sdk::Vec<Val> = vec![
            &env,
            symbol_short!("announce").into_val(&env),
            STELLAR_V2_SCHEME_ID.into_val(&env),
            255u32.into_val(&env),
            METADATA_KIND_VIEW_TAG.into_val(&env),
        ];
        assert_eq!(v2_event2.1, expected_topics2);
    }

    #[test]
    #[should_panic]
    fn test_announce_rejects_v1_scheme_id() {
        let env = Env::new_with_config(EnvTestConfig {
            capture_snapshot_at_drop: false,
        });
        let contract_id = env.register(StealthAnnouncerContract, ());
        let client = StealthAnnouncerContractClient::new(&env, &contract_id);

        let addr = Address::generate(&env);
        let epk = BytesN::from_array(&env, &[1u8; 32]);
        let meta = Bytes::from_slice(&env, &[0u8; 1]);

        client.announce(&1u32, &addr, &epk, &meta);
    }

    #[test]
    #[should_panic]
    fn test_announce_rejects_missing_view_tag() {
        let env = Env::new_with_config(EnvTestConfig {
            capture_snapshot_at_drop: false,
        });
        let contract_id = env.register(StealthAnnouncerContract, ());
        let client = StealthAnnouncerContractClient::new(&env, &contract_id);

        let addr = Address::generate(&env);
        let epk = BytesN::from_array(&env, &[1u8; 32]);
        let meta = Bytes::new(&env);

        client.announce(&STELLAR_V2_SCHEME_ID, &addr, &epk, &meta);
    }
}
