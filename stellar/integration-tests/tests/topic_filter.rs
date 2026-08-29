//! Integration test: topic-3 (`view_tag_bucket`) filtering across all three
//! announcement sources — announcer, splitter, and batch-sender.
//!
//! After Issues #62/#63, every source emits (or routes through) the v2 layout:
//!
//! ```text
//! topics = ("announce", scheme_id, view_tag_bucket, metadata_kind)
//! ```
//!
//! Topic 3 (1-indexed; index 2 in the topic vector) is `view_tag_bucket =
//! metadata[0] as u32`. A server-side filter on that slot must return only the
//! matching subset, regardless of which contract originated the payment.
//!
//! Soroban testutils reset the event buffer after each top-level invocation, so
//! this test harvests announce events after every source call and then applies
//! the topic-3 filter to the combined stream.

use soroban_sdk::testutils::{Address as _, Events};
use soroban_sdk::{symbol_short, token, Address, Bytes, BytesN, Env, FromVal, IntoVal, Val};
use stealth_announcer::{
    StealthAnnouncerContract, StealthAnnouncerContractClient, METADATA_KIND_VIEW_TAG,
    STELLAR_V2_SCHEME_ID,
};
use stealth_batch_sender::{StealthBatchSender, StealthBatchSenderClient, Transfer};
use stealth_splitter::{Beneficiary, StealthSplitterContract, StealthSplitterContractClient};

const MATCH_TAG: u8 = 7;
const OTHER_TAGS: [u8; 3] = [99, 11, 200];

struct HarvestedAnnounce {
    contract: Address,
    bucket: u32,
    stealth: Address,
    metadata: Bytes,
}

fn bytes(env: &Env, data: &[u8]) -> Bytes {
    Bytes::from_slice(env, data)
}

fn bytes32(env: &Env, fill: u8) -> BytesN<32> {
    BytesN::from_array(env, &[fill; 32])
}

fn meta(env: &Env, tag: u8) -> Bytes {
    Bytes::from_slice(env, &[tag])
}

fn funded_token(env: &Env, holder: &Address, amount: i128) -> Address {
    let admin = Address::generate(env);
    let token = env.register_stellar_asset_contract_v2(admin).address();
    token::StellarAssetClient::new(env, &token).mint(holder, &amount);
    token
}

/// Pull v2 announce events out of the current invocation's event buffer.
fn harvest_announces(env: &Env, out: &mut std::vec::Vec<HarvestedAnnounce>) {
    let announce_sym: Val = symbol_short!("announce").into_val(env);
    for event in env.events().all().iter() {
        if event.1.len() != 4 {
            continue;
        }
        let first: Option<Val> = event.1.first();
        if first.map(|t| t.shallow_eq(&announce_sym)) != Some(true) {
            continue;
        }
        let bucket: u32 = FromVal::from_val(env, &event.1.get(2).unwrap());
        let kind: u32 = FromVal::from_val(env, &event.1.get(3).unwrap());
        let scheme: u32 = FromVal::from_val(env, &event.1.get(1).unwrap());
        assert_eq!(scheme, STELLAR_V2_SCHEME_ID);
        assert_eq!(kind, METADATA_KIND_VIEW_TAG);
        let (stealth, _epk, metadata): (Address, BytesN<32>, Bytes) =
            FromVal::from_val(env, &event.2);
        out.push(HarvestedAnnounce {
            contract: event.0,
            bucket,
            stealth,
            metadata,
        });
    }
}

fn filter_by_view_tag<'a>(
    events: &'a [HarvestedAnnounce],
    bucket: u32,
) -> std::vec::Vec<&'a HarvestedAnnounce> {
    events.iter().filter(|e| e.bucket == bucket).collect()
}

#[test]
fn topic_3_view_tag_filter_across_announcer_splitter_and_batch_sender() {
    let env = Env::default();
    env.mock_all_auths();

    // Shared announcer — splitter and batch-sender route through it, so every
    // announcement (all three sources) uses the same 4-topic v2 layout.
    let announcer_id = env.register(StealthAnnouncerContract, ());
    let announcer = StealthAnnouncerContractClient::new(&env, &announcer_id);

    let funder = Address::generate(&env);
    let token = funded_token(&env, &funder, 1_000_000);

    let mut harvested: std::vec::Vec<HarvestedAnnounce> = std::vec::Vec::new();

    // ── Source 1: direct announcer ──────────────────────────────────────────
    let direct_match = Address::generate(&env);
    let direct_other = Address::generate(&env);
    announcer.announce(
        &STELLAR_V2_SCHEME_ID,
        &direct_match,
        &bytes32(&env, 0x11),
        &meta(&env, MATCH_TAG),
    );
    harvest_announces(&env, &mut harvested);
    announcer.announce(
        &STELLAR_V2_SCHEME_ID,
        &direct_other,
        &bytes32(&env, 0x12),
        &meta(&env, OTHER_TAGS[0]),
    );
    harvest_announces(&env, &mut harvested);

    // ── Source 2: splitter (already routes through announcer) ────────────────
    let splitter_id = env.register(StealthSplitterContract, ());
    let splitter = StealthSplitterContractClient::new(&env, &splitter_id);
    splitter.init(&announcer_id);

    let mut beneficiaries = soroban_sdk::vec![&env];
    beneficiaries.push_back(Beneficiary {
        meta_address: bytes(&env, &[0x11u8; 64]),
        weight: 1,
    });
    beneficiaries.push_back(Beneficiary {
        meta_address: bytes(&env, &[0x22u8; 64]),
        weight: 1,
    });
    let split_id = splitter.create_split(
        &funder,
        &beneficiaries,
        &token,
        &bytes(&env, b"topic-filter-salt"),
    );

    let split_match = Address::generate(&env);
    let split_other = Address::generate(&env);
    splitter.fund_split(
        &funder,
        &split_id,
        &2_000,
        &STELLAR_V2_SCHEME_ID,
        &soroban_sdk::vec![&env, split_match.clone(), split_other.clone()],
        &soroban_sdk::vec![&env, bytes32(&env, 0x21), bytes32(&env, 0x22)],
        &soroban_sdk::vec![&env, meta(&env, MATCH_TAG), meta(&env, OTHER_TAGS[1])],
    );
    harvest_announces(&env, &mut harvested);

    // ── Source 3: batch-sender (now routes through announcer) ────────────────
    let batch_id = env.register(StealthBatchSender, ());
    let batch = StealthBatchSenderClient::new(&env, &batch_id);
    let admin = Address::generate(&env);
    batch.init(&admin, &announcer_id, &None);

    let batch_match = Address::generate(&env);
    let batch_other = Address::generate(&env);
    let transfers = soroban_sdk::vec![
        &env,
        Transfer {
            stealth_address: batch_match.clone(),
            ephemeral_pub_key: bytes(&env, &[0x31u8; 32]),
            amount: 100,
            metadata: meta(&env, MATCH_TAG),
        },
        Transfer {
            stealth_address: batch_other.clone(),
            ephemeral_pub_key: bytes(&env, &[0x32u8; 32]),
            amount: 200,
            metadata: meta(&env, OTHER_TAGS[2]),
        },
    ];
    batch.batch_send(&funder, &transfers, &token);
    harvest_announces(&env, &mut harvested);

    // ── Assertions ──────────────────────────────────────────────────────────
    assert_eq!(
        harvested.len(),
        6,
        "two announcements from each of the three sources"
    );
    assert!(harvested.iter().all(|e| e.contract == announcer_id));

    let matched = filter_by_view_tag(&harvested, MATCH_TAG as u32);
    assert_eq!(
        matched.len(),
        3,
        "topic-3 filter must keep exactly one event per source"
    );

    let seen_match_addrs: std::vec::Vec<Address> = matched
        .iter()
        .map(|e| {
            assert_eq!(e.metadata, meta(&env, MATCH_TAG));
            e.stealth.clone()
        })
        .collect();

    assert!(seen_match_addrs.contains(&direct_match));
    assert!(seen_match_addrs.contains(&split_match));
    assert!(seen_match_addrs.contains(&batch_match));
    assert!(!seen_match_addrs.contains(&direct_other));
    assert!(!seen_match_addrs.contains(&split_other));
    assert!(!seen_match_addrs.contains(&batch_other));

    // Negative: a tag that none of the sources used yields an empty subset.
    assert_eq!(filter_by_view_tag(&harvested, 255).len(), 0);
}
