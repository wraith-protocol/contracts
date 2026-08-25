//! Topic-3 (view-tag bucket) filter across all announcement sources.
//!
//! Announcer, splitter, and batch-sender must all produce the v2 4-topic
//! layout `("announce", scheme_id, view_tag_bucket, metadata_kind)` so a
//! single `getEvents` filter on topic 3 (`view_tag_bucket`) returns only the
//! matching subset — regardless of which contract originated the payment.

use soroban_sdk::testutils::{Address as _, Events};
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::{
    symbol_short, Address, Bytes, BytesN, Env, FromVal, IntoVal, Symbol, Val, Vec as SorobanVec,
};

use stealth_announcer::{
    StealthAnnouncerContract, StealthAnnouncerContractClient, METADATA_KIND_VIEW_TAG,
    STELLAR_V2_SCHEME_ID,
};
use stealth_batch_sender::{StealthBatchSender, StealthBatchSenderClient, Transfer};
use stealth_splitter::{Beneficiary, StealthSplitterContract, StealthSplitterContractClient};

/// Zero-based index of `view_tag_bucket` (topic 3 in 1-based `getEvents` docs).
/// Topics: (0) "announce", (1) scheme_id, (2) view_tag_bucket, (3) metadata_kind.
const VIEW_TAG_TOPIC_INDEX: u32 = 2;

type HostEvent = (Address, soroban_sdk::Vec<Val>, Val);

fn meta(env: &Env, view_tag: u8) -> Bytes {
    Bytes::from_slice(env, &[view_tag])
}

fn epk(env: &Env, fill: u8) -> BytesN<32> {
    BytesN::from_array(env, &[fill; 32])
}

fn beneficiary(env: &Env, fill: u8) -> Beneficiary {
    Beneficiary {
        meta_address: Bytes::from_slice(env, &[fill; 64]),
        weight: 1,
    }
}

fn stealth_of(env: &Env, event: &HostEvent) -> Address {
    let data: (Address, BytesN<32>, Bytes) = FromVal::from_val(env, &event.2);
    data.0
}

fn view_tag_of(env: &Env, event: &HostEvent) -> u32 {
    FromVal::from_val(env, &event.1.get(VIEW_TAG_TOPIC_INDEX).unwrap())
}

/// v2 announce events visible on the host right now.
fn current_announce_events(env: &Env) -> std::vec::Vec<HostEvent> {
    let mut out = std::vec::Vec::new();
    for event in env.events().all().iter() {
        let topics = event.1.clone();
        if topics.len() != 4 {
            continue;
        }
        let topic0: Symbol = FromVal::from_val(env, &topics.get(0).unwrap());
        if topic0 == symbol_short!("announce") {
            out.push(event);
        }
    }
    out
}

/// Merge newly observed announce events into `acc`, keyed by stealth address.
/// Works whether the host accumulates events or only retains the last invoke.
fn merge_announce(env: &Env, acc: &mut std::vec::Vec<HostEvent>) {
    for event in current_announce_events(env) {
        let stealth = stealth_of(env, &event);
        if !acc
            .iter()
            .any(|existing| stealth_of(env, existing) == stealth)
        {
            acc.push(event);
        }
    }
}

fn filter_by_view_tag<'a>(
    env: &Env,
    events: &'a [HostEvent],
    bucket: u32,
) -> std::vec::Vec<&'a HostEvent> {
    events
        .iter()
        .filter(|event| view_tag_of(env, event) == bucket)
        .collect()
}

fn assert_v2_topics(env: &Env, event: &HostEvent, bucket: u32) {
    let expected: soroban_sdk::Vec<Val> = soroban_sdk::vec![
        env,
        symbol_short!("announce").into_val(env),
        STELLAR_V2_SCHEME_ID.into_val(env),
        bucket.into_val(env),
        METADATA_KIND_VIEW_TAG.into_val(env),
    ];
    assert_eq!(&event.1, &expected);
}

#[test]
fn topic3_view_tag_filter_matches_across_announcer_splitter_and_batch_sender() {
    let env = Env::default();
    env.mock_all_auths();

    let mut recorded: std::vec::Vec<HostEvent> = std::vec::Vec::new();

    // --- Deploy the three sources + a shared announcer + token ---------------
    let announcer_id = env.register(StealthAnnouncerContract, ());
    let announcer = StealthAnnouncerContractClient::new(&env, &announcer_id);

    let splitter_id = env.register(StealthSplitterContract, ());
    let splitter = StealthSplitterContractClient::new(&env, &splitter_id);
    splitter.init(&announcer_id);

    let batch_id = env.register(StealthBatchSender, ());
    let batch = StealthBatchSenderClient::new(&env, &batch_id);

    let admin = Address::generate(&env);
    let funder = Address::generate(&env);
    let token = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    StellarAssetClient::new(&env, &token).mint(&funder, &1_000_000);

    // Distinct stealth addresses so we can attribute filtered results.
    let ann_match = Address::generate(&env);
    let ann_other = Address::generate(&env);
    let split_match = Address::generate(&env);
    let split_other = Address::generate(&env);
    let batch_match = Address::generate(&env);
    let batch_other = Address::generate(&env);

    const MATCH_TAG: u8 = 42;
    const ANN_OTHER_TAG: u8 = 99;
    const SPLIT_OTHER_TAG: u8 = 7;
    const BATCH_OTHER_TAG: u8 = 200;

    // --- Source 1: direct announcer -----------------------------------------
    announcer.announce(
        &STELLAR_V2_SCHEME_ID,
        &ann_match,
        &epk(&env, 0x11),
        &meta(&env, MATCH_TAG),
    );
    merge_announce(&env, &mut recorded);
    announcer.announce(
        &STELLAR_V2_SCHEME_ID,
        &ann_other,
        &epk(&env, 0x12),
        &meta(&env, ANN_OTHER_TAG),
    );
    merge_announce(&env, &mut recorded);

    // --- Source 2: splitter (routes through announcer) ----------------------
    let mut beneficiaries = SorobanVec::new(&env);
    beneficiaries.push_back(beneficiary(&env, 1));
    beneficiaries.push_back(beneficiary(&env, 2));
    let split_id = splitter.create_split(
        &funder,
        &beneficiaries,
        &token,
        &Bytes::from_slice(&env, b"topic-filter-salt"),
    );

    let mut stealths = SorobanVec::new(&env);
    stealths.push_back(split_match.clone());
    stealths.push_back(split_other.clone());
    let mut keys = SorobanVec::new(&env);
    keys.push_back(epk(&env, 0x21));
    keys.push_back(epk(&env, 0x22));
    let mut metas = SorobanVec::new(&env);
    metas.push_back(meta(&env, MATCH_TAG));
    metas.push_back(meta(&env, SPLIT_OTHER_TAG));

    splitter.fund_split(
        &funder,
        &split_id,
        &1_000i128,
        &STELLAR_V2_SCHEME_ID,
        &stealths,
        &keys,
        &metas,
    );
    merge_announce(&env, &mut recorded);

    // --- Source 3: batch-sender (also routes through announcer) -------------
    let mut transfers = SorobanVec::new(&env);
    transfers.push_back(Transfer {
        stealth_address: batch_match.clone(),
        ephemeral_pub_key: epk(&env, 0x31),
        amount: 100,
        metadata: meta(&env, MATCH_TAG),
    });
    transfers.push_back(Transfer {
        stealth_address: batch_other.clone(),
        ephemeral_pub_key: epk(&env, 0x32),
        amount: 100,
        metadata: meta(&env, BATCH_OTHER_TAG),
    });
    batch.batch_send(
        &funder,
        &transfers,
        &token,
        &announcer_id,
        &STELLAR_V2_SCHEME_ID,
    );
    merge_announce(&env, &mut recorded);

    assert_eq!(
        recorded.len(),
        6,
        "all three sources must emit two announce events each"
    );
    for event in &recorded {
        assert_eq!(
            event.0, announcer_id,
            "every announce is published by the announcer contract"
        );
        assert_eq!(event.1.len(), 4);
    }

    // --- Topic-3 filter: only view-tag 42 -----------------------------------
    let matched = filter_by_view_tag(&env, &recorded, MATCH_TAG as u32);
    assert_eq!(
        matched.len(),
        3,
        "topic-3 filter must return exactly one hit per source"
    );

    let mut stealths_seen = std::vec::Vec::new();
    for event in &matched {
        assert_v2_topics(&env, event, MATCH_TAG as u32);
        stealths_seen.push(stealth_of(&env, event));
    }
    assert!(stealths_seen.contains(&ann_match));
    assert!(stealths_seen.contains(&split_match));
    assert!(stealths_seen.contains(&batch_match));

    // Non-matching tags must not leak into the filtered set.
    assert!(!stealths_seen.contains(&ann_other));
    assert!(!stealths_seen.contains(&split_other));
    assert!(!stealths_seen.contains(&batch_other));

    // Each other tag is uniquely attributable to one source.
    let only_ann = filter_by_view_tag(&env, &recorded, ANN_OTHER_TAG as u32);
    assert_eq!(only_ann.len(), 1);
    assert_v2_topics(&env, only_ann[0], ANN_OTHER_TAG as u32);
    assert_eq!(stealth_of(&env, only_ann[0]), ann_other);

    let only_split = filter_by_view_tag(&env, &recorded, SPLIT_OTHER_TAG as u32);
    assert_eq!(only_split.len(), 1);
    assert_v2_topics(&env, only_split[0], SPLIT_OTHER_TAG as u32);
    assert_eq!(stealth_of(&env, only_split[0]), split_other);

    let only_batch = filter_by_view_tag(&env, &recorded, BATCH_OTHER_TAG as u32);
    assert_eq!(only_batch.len(), 1);
    assert_v2_topics(&env, only_batch[0], BATCH_OTHER_TAG as u32);
    assert_eq!(stealth_of(&env, only_batch[0]), batch_other);

    // A tag that no source used returns nothing.
    assert!(filter_by_view_tag(&env, &recorded, 123u32).is_empty());
}
