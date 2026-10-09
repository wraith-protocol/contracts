#![no_std]

use soroban_sdk::{contract, contractimpl, symbol_short, Address, Bytes, BytesN, Env};

/// Stellar v2
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

/// Storage key for the persistent event sequence counter.
const EVENT_SEQ_KEY: &[u8] = b"event_seq";

/// Unique, incrementing sequence ID for each event emitted by this contract.
/// Used for replay protection and reorg safety — indexers can track the
/// highest sequence ID they've processed to avoid processing stale events.
fn next_event_sequence_id(env: &Env) -> u64 {
    let seq: u64 = env
        .storage()
        .instance()
        .get(&EVENT_SEQ_KEY)
        .unwrap_or(0);
    env.storage().instance().set(&EVENT_SEQ_KEY, &(seq + 1));
    seq
}

/// Emits a Stellar v2 stealth address announcement event with replay protection.
///
/// v2 event shape with replay protection:
/// * topics: `("announce", scheme_id, view_tag_bucket, metadata_kind, event_sequence_id)`
/// * data: `(stealth_address, ephemeral_pub_key, metadata)`
///
/// The stable `view_tag_bucket` derivation is `metadata[0] as u32`, where
/// `metadata_kind = 1` (`METADATA_KIND_VIEW_TAG`) means the first metadata
/// byte is the view tag and the remaining bytes are scheme-specific. This
/// lets wallets and indexers filter Stellar RPC `getEvents` by scheme and
/// bucket before doing client-side cryptographic validation.
///
/// The `event_sequence_id` is topic 4, a monotonically increasing counter.
/// Indexers should track the highest sequence ID they have processed to skip
/// stale or duplicate events after a reorg.
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
#[contractimpl]
impl StealthAnnouncerContract {
    /// Emits a Stellar v2 stealth address announcement event with replay protection.
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
        let event_seq = next_event_sequence_id(&env);

        env.events().publish(
            (
                symbol_short!("announce"),
                scheme_id,
                view_tag_bucket,
                metadata_kind,
                event_seq,
            ),
            (stealth_address, ephemeral_pub_key, metadata),
        );
    }
}

#[contract]
pub struct StealthAnnouncerContract;