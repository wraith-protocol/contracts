//! Conformance fixtures — stealth-sender event pipeline.
//!
//! These tests wire the **real** `StealthAnnouncerContract` behind the sender,
//! following the same pattern as `stealth-vault/tests/announcer.rs`.
//!
//! Wire shapes asserted:
//!
//! `send` (via real announcer):
//!   topics: `("announce", 2u32, bucket: u32, metadata_kind: u32)`
//!   data:   `(stealth_address, epk, metadata)`
//!
//! `withdraw_many`:
//!   `Withdrawn`     topics: `("Withdrawn",)`     data: `(withdrawer, to, amount, token)`
//!   `BatchWithdrawn` topics: `("BatchWithdrawn",)` data: `(withdrawer, count, total_amount)`

use soroban_sdk::testutils::{Address as _, Events, Ledger};
use soroban_sdk::{symbol_short, token, Address, Bytes, BytesN, Env, FromVal, IntoVal, Symbol, TryFromVal};

use stealth_announcer::{
    StealthAnnouncerContract, METADATA_KIND_VIEW_TAG, STELLAR_V2_SCHEME_ID,
};
use stealth_sender::{StealthSenderContract, StealthSenderContractClient, WithdrawalEntry};

// ── Canonical fixture constants ───────────────────────────────────────────────

const FIXTURE_EPK: [u8; 32] = [
    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
    0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
    0xff, 0x00,
];
/// view tag = 0xab → bucket = 171u32
const FIXTURE_METADATA: [u8; 3] = [0xab, 0x00, 0x01];
const FIXTURE_VIEW_TAG_BUCKET: u32 = 0xab; // 171
const FIXTURE_SEND_AMOUNT: i128 = 1_000;

// ── Setup ─────────────────────────────────────────────────────────────────────

struct SenderFixture {
    env: Env,
    client: StealthSenderContractClient<'static>,
    sender_addr: Address,
    stealth_address: Address,
    token_id: Address,
}

fn setup_sender() -> SenderFixture {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.min_persistent_entry_ttl = 600_000;
    });
    let announcer_id = env.register(StealthAnnouncerContract, ());
    let sender_contract_id = env.register(StealthSenderContract, ());
    let client = StealthSenderContractClient::new(&env, &sender_contract_id);
    let admin = Address::generate(&env);
    client.init(&announcer_id, &None, &None, &0u32, &admin);

    let token_admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(token_admin).address();
    let sender_addr = Address::generate(&env);
    let stealth_address = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_id).mint(&sender_addr, &100_000);

    SenderFixture { env, client, sender_addr, stealth_address, token_id }
}

fn epk(env: &Env) -> BytesN<32> { BytesN::from_array(env, &FIXTURE_EPK) }
fn metadata(env: &Env) -> Bytes { Bytes::from_slice(env, &FIXTURE_METADATA) }

/// Find the single `announce` event emitted by the real announcer.
fn find_announce(env: &Env) -> (Address, soroban_sdk::Vec<soroban_sdk::Val>, soroban_sdk::Val) {
    let sym: soroban_sdk::Val = symbol_short!("announce").into_val(env);
    env.events()
        .all()
        .iter()
        .find(|(_, topics, _)| topics.first().map(|t| t.shallow_eq(&sym)) == Some(true))
        .expect("no announce event found")
}

/// Find an event whose topic[0] matches the given full `Symbol` (not a short symbol).
///
/// Used to locate `Withdrawn` and `BatchWithdrawn` events which use
/// `Symbol::new(&env, "...")` (long symbols, not `symbol_short!`).
/// Uses content-based `Symbol` equality rather than `shallow_eq` since long
/// symbols are host-object-backed and `shallow_eq` compares object handles.
fn find_event_by_long_sym(
    env: &Env,
    sym: Symbol,
) -> (Address, soroban_sdk::Vec<soroban_sdk::Val>, soroban_sdk::Val) {
    env.events()
        .all()
        .iter()
        .find(|(_, topics, _)| {
            topics
                .first()
                .and_then(|t| soroban_sdk::Symbol::try_from_val(env, &t).ok())
                .map(|s| s == sym)
                == Some(true)
        })
        .expect("event with matching topic[0] symbol not found")
}

// ── send → real announcer fixtures ───────────────────────────────────────────

#[test]
fn send_emits_announce_event_via_real_announcer() {
    let f = setup_sender();
    f.client.send(
        &f.sender_addr, &f.token_id, &FIXTURE_SEND_AMOUNT,
        &STELLAR_V2_SCHEME_ID, &f.stealth_address, &epk(&f.env), &metadata(&f.env),
    );
    let sym: soroban_sdk::Val = symbol_short!("announce").into_val(&f.env);
    let count = f.env.events().all().iter()
        .filter(|(_, topics, _)| topics.first().map(|t| t.shallow_eq(&sym)) == Some(true))
        .count();
    assert_eq!(count, 1, "send must emit exactly one announce event via the real announcer");
}

#[test]
fn send_announce_topic1_is_v2_scheme_id() {
    let f = setup_sender();
    f.client.send(
        &f.sender_addr, &f.token_id, &FIXTURE_SEND_AMOUNT,
        &STELLAR_V2_SCHEME_ID, &f.stealth_address, &epk(&f.env), &metadata(&f.env),
    );
    let event = find_announce(&f.env);
    let scheme_id: u32 = FromVal::from_val(&f.env, &event.1.get(1).unwrap());
    assert_eq!(scheme_id, STELLAR_V2_SCHEME_ID);
}

#[test]
fn send_announce_topic2_is_view_tag_bucket() {
    let f = setup_sender();
    f.client.send(
        &f.sender_addr, &f.token_id, &FIXTURE_SEND_AMOUNT,
        &STELLAR_V2_SCHEME_ID, &f.stealth_address, &epk(&f.env), &metadata(&f.env),
    );
    let event = find_announce(&f.env);
    let bucket: u32 = FromVal::from_val(&f.env, &event.1.get(2).unwrap());
    assert_eq!(bucket, FIXTURE_VIEW_TAG_BUCKET);
}

#[test]
fn send_announce_topic3_is_metadata_kind_constant() {
    let f = setup_sender();
    f.client.send(
        &f.sender_addr, &f.token_id, &FIXTURE_SEND_AMOUNT,
        &STELLAR_V2_SCHEME_ID, &f.stealth_address, &epk(&f.env), &metadata(&f.env),
    );
    let event = find_announce(&f.env);
    let kind: u32 = FromVal::from_val(&f.env, &event.1.get(3).unwrap());
    assert_eq!(kind, METADATA_KIND_VIEW_TAG);
}

#[test]
fn send_announce_data_round_trip() {
    let f = setup_sender();
    let epk_val = epk(&f.env);
    let meta_val = metadata(&f.env);
    f.client.send(
        &f.sender_addr, &f.token_id, &FIXTURE_SEND_AMOUNT,
        &STELLAR_V2_SCHEME_ID, &f.stealth_address, &epk_val, &meta_val,
    );
    let event = find_announce(&f.env);
    let (decoded_addr, decoded_epk, decoded_meta): (Address, BytesN<32>, Bytes) =
        FromVal::from_val(&f.env, &event.2);

    assert_eq!(decoded_addr, f.stealth_address, "data[0] stealth_address must round-trip");
    assert_eq!(decoded_epk, epk_val, "data[1] ephemeral_pub_key must round-trip");
    assert_eq!(decoded_meta, meta_val, "data[2] metadata must round-trip");
}

// ── withdraw_many event fixtures ──────────────────────────────────────────────

struct WithdrawFixture {
    env: Env,
    client: StealthSenderContractClient<'static>,
    withdrawer: Address,
    token_id: Address,
}

fn setup_withdraw() -> WithdrawFixture {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| { li.min_persistent_entry_ttl = 600_000; });
    let announcer_id = env.register(StealthAnnouncerContract, ());
    let sender_contract_id = env.register(StealthSenderContract, ());
    let client = StealthSenderContractClient::new(&env, &sender_contract_id);
    let admin = Address::generate(&env);
    client.init(&announcer_id, &None, &None, &0u32, &admin);

    let token_admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract_v2(token_admin).address();
    let withdrawer = Address::generate(&env);
    token::StellarAssetClient::new(&env, &token_id).mint(&withdrawer, &100_000);

    WithdrawFixture { env, client, withdrawer, token_id }
}

fn make_entry(token: &Address, to: &Address, amount: i128) -> WithdrawalEntry {
    WithdrawalEntry { token: token.clone(), to: to.clone(), amount }
}

#[test]
fn withdraw_many_single_entry_emits_withdrawn_and_batch_withdrawn() {
    // `withdraw_many` also triggers a SAC `transfer` event per entry.
    // For 1 entry: [transfer, Withdrawn, BatchWithdrawn] = 3 events total.
    // This test asserts the wire events (Withdrawn + BatchWithdrawn) are both present.
    let f = setup_withdraw();
    let recipient = Address::generate(&f.env);
    let entries = soroban_sdk::vec![&f.env, make_entry(&f.token_id, &recipient, 500)];
    f.client.withdraw_many(&f.withdrawer, &entries);

    let events = f.env.events().all();
    // Count only the custom wire events (Withdrawn + BatchWithdrawn), not SAC transfers.
    // Use content-based Symbol comparison (not shallow_eq) since BatchWithdrawn is a
    // long symbol stored as a host-object reference.
    let withdrawn_sym = Symbol::new(&f.env, "Withdrawn");
    let batch_sym = Symbol::new(&f.env, "BatchWithdrawn");
    let mut withdrawn_count = 0usize;
    let mut batch_count = 0usize;
    for (_, topics, _) in events.iter() {
        if let Some(t) = topics.first() {
            if let Ok(s) = soroban_sdk::Symbol::try_from_val(&f.env, &t) {
                if s == withdrawn_sym { withdrawn_count += 1; }
                if s == batch_sym    { batch_count    += 1; }
            }
        }
    }
    assert_eq!(withdrawn_count, 1, "single-entry withdraw_many must emit exactly 1 Withdrawn event");
    assert_eq!(batch_count, 1, "single-entry withdraw_many must emit exactly 1 BatchWithdrawn event");
}

#[test]
fn withdrawn_topic0_is_withdrawn_symbol() {
    let f = setup_withdraw();
    let recipient = Address::generate(&f.env);
    let entries = soroban_sdk::vec![&f.env, make_entry(&f.token_id, &recipient, 500)];
    f.client.withdraw_many(&f.withdrawer, &entries);

    // Use find_event_by_long_sym to locate Withdrawn, skipping the SAC transfer event.
    let event = find_event_by_long_sym(&f.env, Symbol::new(&f.env, "Withdrawn"));
    let got: Symbol = FromVal::from_val(&f.env, &event.1.get(0).unwrap());
    assert_eq!(got, Symbol::new(&f.env, "Withdrawn"));
}

#[test]
fn withdrawn_data_round_trip() {
    let f = setup_withdraw();
    let recipient = Address::generate(&f.env);
    let amount: i128 = 750;
    let entries = soroban_sdk::vec![&f.env, make_entry(&f.token_id, &recipient, amount)];
    f.client.withdraw_many(&f.withdrawer, &entries);

    // Use find_event_by_long_sym to skip the SAC transfer event.
    let event = find_event_by_long_sym(&f.env, Symbol::new(&f.env, "Withdrawn"));
    let (w, to, amt, tok): (Address, Address, i128, Address) = FromVal::from_val(&f.env, &event.2);

    assert_eq!(w, f.withdrawer, "Withdrawn.withdrawer must round-trip");
    assert_eq!(to, recipient, "Withdrawn.to must round-trip");
    assert_eq!(amt, amount, "Withdrawn.amount must round-trip");
    assert_eq!(tok, f.token_id, "Withdrawn.token must round-trip");
}

#[test]
fn batch_withdrawn_topic0_is_batch_withdrawn_symbol() {
    let f = setup_withdraw();
    let recipient = Address::generate(&f.env);
    let entries = soroban_sdk::vec![&f.env, make_entry(&f.token_id, &recipient, 500)];
    f.client.withdraw_many(&f.withdrawer, &entries);

    let event = f.env.events().all().last().unwrap();
    let got: Symbol = FromVal::from_val(&f.env, &event.1.get(0).unwrap());
    assert_eq!(got, Symbol::new(&f.env, "BatchWithdrawn"));
}

#[test]
fn batch_withdrawn_data_round_trip() {
    let f = setup_withdraw();
    let r1 = Address::generate(&f.env);
    let r2 = Address::generate(&f.env);
    let entries = soroban_sdk::vec![
        &f.env,
        make_entry(&f.token_id, &r1, 300),
        make_entry(&f.token_id, &r2, 400),
    ];
    f.client.withdraw_many(&f.withdrawer, &entries);

    let event = f.env.events().all().last().unwrap();
    let (w, count, total): (Address, u32, i128) = FromVal::from_val(&f.env, &event.2);

    assert_eq!(w, f.withdrawer, "BatchWithdrawn.withdrawer must round-trip");
    assert_eq!(count, 2u32, "BatchWithdrawn.count must equal batch size");
    assert_eq!(total, 700i128, "BatchWithdrawn.total_amount must equal sum of amounts");
}

#[test]
fn batch_of_two_emits_two_withdrawn_plus_one_batch_withdrawn() {
    // For 2 entries: [transfer, Withdrawn, transfer, Withdrawn, BatchWithdrawn] = 5 total.
    // This test confirms exactly 2 Withdrawn events and 1 BatchWithdrawn event.
    let f = setup_withdraw();
    let r1 = Address::generate(&f.env);
    let r2 = Address::generate(&f.env);
    let entries = soroban_sdk::vec![
        &f.env,
        make_entry(&f.token_id, &r1, 100),
        make_entry(&f.token_id, &r2, 200),
    ];
    f.client.withdraw_many(&f.withdrawer, &entries);

    let events = f.env.events().all();
    let withdrawn_sym = Symbol::new(&f.env, "Withdrawn");
    let batch_sym = Symbol::new(&f.env, "BatchWithdrawn");
    let mut withdrawn_count = 0usize;
    let mut batch_count = 0usize;
    for (_, topics, _) in events.iter() {
        if let Some(t) = topics.first() {
            if let Ok(s) = soroban_sdk::Symbol::try_from_val(&f.env, &t) {
                if s == withdrawn_sym { withdrawn_count += 1; }
                if s == batch_sym    { batch_count    += 1; }
            }
        }
    }

    assert_eq!(withdrawn_count, 2, "2-entry batch must emit exactly 2 Withdrawn events");
    assert_eq!(batch_count, 1, "2-entry batch must emit exactly 1 BatchWithdrawn event");
}

#[test]
fn per_entry_withdrawn_amounts_match_batch_inputs() {
    let f = setup_withdraw();
    let r1 = Address::generate(&f.env);
    let r2 = Address::generate(&f.env);
    let entries = soroban_sdk::vec![
        &f.env,
        make_entry(&f.token_id, &r1, 111),
        make_entry(&f.token_id, &r2, 222),
    ];
    f.client.withdraw_many(&f.withdrawer, &entries);

    // Collect all Withdrawn events in order, skipping SAC transfer events.
    // Use content-based Symbol comparison for robustness.
    let withdrawn_sym = Symbol::new(&f.env, "Withdrawn");
    let withdrawn_events: std::vec::Vec<_> = f
        .env
        .events()
        .all()
        .iter()
        .filter(|(_, t, _)| {
            t.first()
                .and_then(|v| soroban_sdk::Symbol::try_from_val(&f.env, &v).ok())
                .map(|s| s == withdrawn_sym)
                == Some(true)
        })
        .collect();

    assert_eq!(withdrawn_events.len(), 2, "must have exactly 2 Withdrawn events");
    let (_, _, amt0, _): (Address, Address, i128, Address) =
        FromVal::from_val(&f.env, &withdrawn_events[0].2);
    let (_, _, amt1, _): (Address, Address, i128, Address) =
        FromVal::from_val(&f.env, &withdrawn_events[1].2);

    assert_eq!(amt0, 111i128, "first Withdrawn amount must match entry[0]");
    assert_eq!(amt1, 222i128, "second Withdrawn amount must match entry[1]");
}

// ── send() token-amount conformance (Task 4.3) ────────────────────────────────
//
// The `announce` event emitted by stealth-sender.send() carries no amount
// field — the stealth address and key material are announced, but the token
// amount is a separate transfer concern.  The only way to assert the correct
// amount was transferred is to check the stealth address's token balance
// before and after the call.
//
// The sender is initialised with 0 fee basis points, so transfer_amount ==
// FIXTURE_SEND_AMOUNT exactly.

#[test]
fn send_transfers_exact_fixture_amount_to_stealth_address() {
    let f = setup_sender();

    let token_client = token::Client::new(&f.env, &f.token_id);

    // Balance before: stealth address has not received anything yet.
    let balance_before = token_client.balance(&f.stealth_address);

    f.client.send(
        &f.sender_addr,
        &f.token_id,
        &FIXTURE_SEND_AMOUNT,
        &STELLAR_V2_SCHEME_ID,
        &f.stealth_address,
        &epk(&f.env),
        &metadata(&f.env),
    );

    // Balance after: must have increased by exactly FIXTURE_SEND_AMOUNT.
    // (No fee configured — init called with 0u32 fee_basis_points.)
    let balance_after = token_client.balance(&f.stealth_address);
    assert_eq!(
        balance_after - balance_before,
        FIXTURE_SEND_AMOUNT,
        "send() must transfer exactly FIXTURE_SEND_AMOUNT tokens to the stealth address"
    );
}
