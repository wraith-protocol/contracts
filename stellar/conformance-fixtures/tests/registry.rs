//! Conformance fixtures — stealth-registry event pipeline.
//!
//! Wire shapes asserted:
//!
//! `register_keys`:
//!   * topics : `("register", registrant: Address, scheme_id: u32)`
//!   * data   : `stealth_meta_address: Bytes`  (exactly 64 bytes)
//!
//! `remove_keys`:
//!   * topics : `("remove", registrant: Address, scheme_id: u32)`
//!   * data   : `()` (unit)
//!
//! Implementation note: both `register_keys` and `remove_keys` emit a second
//! internal `metric` event (via `wraith-metrics::emit_metric`) in the same
//! transaction.  Tests must locate events by symbol rather than relying on
//! `.last()` or positional indices.  Similarly, `env.events().all()` in the
//! Soroban test environment only retains events from the most recent contract
//! invocation, so multi-call tests capture events immediately after each call.

use soroban_sdk::testutils::{Address as _, Events, Ledger};
use soroban_sdk::{symbol_short, Address, Bytes, Env, FromVal, IntoVal, Val};

use stealth_registry::{StealthRegistryContract, StealthRegistryContractClient};

// ── Canonical fixture constants ───────────────────────────────────────────────

const FIXTURE_SCHEME_ID: u32 = 1;

const FIXTURE_META_ADDRESS: [u8; 64] = {
    let mut b = [0u8; 64];
    let mut i = 0usize;
    while i < 32 {
        b[i] = (i as u8) + 1;
        i += 1;
    }
    let mut j = 0usize;
    while j < 32 {
        b[32 + j] = (j as u8) + 0x21;
        j += 1;
    }
    b
};

// ── Helpers ───────────────────────────────────────────────────────────────────

struct Fixture<'a> {
    env: Env,
    client: StealthRegistryContractClient<'a>,
}

fn setup() -> Fixture<'static> {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.min_persistent_entry_ttl = 600_000;
    });
    let contract_id = env.register(StealthRegistryContract, ());
    let client = StealthRegistryContractClient::new(&env, &contract_id);
    Fixture { env, client }
}

fn meta_address(env: &Env) -> Bytes {
    Bytes::from_slice(env, &FIXTURE_META_ADDRESS)
}

/// Find the first event whose topic[0] matches `sym`, or panic.
///
/// Both `register_keys` and `remove_keys` emit an additional `metric` event
/// in the same transaction.  This helper skips that event and returns the
/// wire event with the given symbol.
fn find_event_by_sym(
    env: &Env,
    sym: soroban_sdk::Symbol,
) -> (Address, soroban_sdk::Vec<Val>, Val) {
    let sym_val: Val = sym.into_val(env);
    env.events()
        .all()
        .iter()
        .find(|(_, topics, _)| topics.first().map(|t| t.shallow_eq(&sym_val)) == Some(true))
        .expect("event with matching topic[0] symbol not found")
}

// ── register_keys fixtures ────────────────────────────────────────────────────

/// `register_keys` emits exactly one wire event with topic[0] = "register".
///
/// (A second internal `metric` event is also emitted in the same transaction,
/// so the raw event count is 2.  This test counts only the "register" events.)
#[test]
fn register_emits_exactly_one_event() {
    let f = setup();
    let registrant = Address::generate(&f.env);
    f.client.register_keys(&registrant, &FIXTURE_SCHEME_ID, &meta_address(&f.env));

    let sym_val: Val = symbol_short!("register").into_val(&f.env);
    let count = f
        .env
        .events()
        .all()
        .iter()
        .filter(|(_, topics, _)| topics.first().map(|t| t.shallow_eq(&sym_val)) == Some(true))
        .count();
    assert_eq!(count, 1, "expected exactly one 'register' event");
}

#[test]
fn register_topic0_is_register_symbol() {
    let f = setup();
    let registrant = Address::generate(&f.env);
    f.client.register_keys(&registrant, &FIXTURE_SCHEME_ID, &meta_address(&f.env));

    let event = find_event_by_sym(&f.env, symbol_short!("register"));
    let got: soroban_sdk::Symbol = FromVal::from_val(&f.env, &event.1.get(0).unwrap());
    assert_eq!(got, symbol_short!("register"));
}

#[test]
fn register_topic1_is_registrant_address() {
    let f = setup();
    let registrant = Address::generate(&f.env);
    f.client.register_keys(&registrant, &FIXTURE_SCHEME_ID, &meta_address(&f.env));

    let event = find_event_by_sym(&f.env, symbol_short!("register"));
    let got_addr: Address = FromVal::from_val(&f.env, &event.1.get(1).unwrap());
    assert_eq!(got_addr, registrant, "topic[1] must be the registrant");
}

#[test]
fn register_topic2_is_scheme_id() {
    let f = setup();
    let registrant = Address::generate(&f.env);
    f.client.register_keys(&registrant, &FIXTURE_SCHEME_ID, &meta_address(&f.env));

    let event = find_event_by_sym(&f.env, symbol_short!("register"));
    let got_scheme: u32 = FromVal::from_val(&f.env, &event.1.get(2).unwrap());
    assert_eq!(got_scheme, FIXTURE_SCHEME_ID, "topic[2] must be the scheme_id");
}

#[test]
fn register_topic_tuple_has_three_entries() {
    let f = setup();
    let registrant = Address::generate(&f.env);
    f.client.register_keys(&registrant, &FIXTURE_SCHEME_ID, &meta_address(&f.env));

    let event = find_event_by_sym(&f.env, symbol_short!("register"));
    assert_eq!(event.1.len(), 3, "register topic tuple must have 3 entries");
}

#[test]
fn register_data_round_trips_meta_address() {
    let f = setup();
    let registrant = Address::generate(&f.env);
    let expected_meta = meta_address(&f.env);
    f.client.register_keys(&registrant, &FIXTURE_SCHEME_ID, &expected_meta);

    let event = find_event_by_sym(&f.env, symbol_short!("register"));
    let decoded_meta: Bytes = FromVal::from_val(&f.env, &event.2);

    assert_eq!(decoded_meta, expected_meta, "data must round-trip the stealth meta-address");
    assert_eq!(decoded_meta.len(), 64, "meta-address must be exactly 64 bytes");
}

#[test]
fn register_full_topic_round_trip() {
    let f = setup();
    let registrant = Address::generate(&f.env);
    f.client.register_keys(&registrant, &FIXTURE_SCHEME_ID, &meta_address(&f.env));

    let event = find_event_by_sym(&f.env, symbol_short!("register"));
    let expected: soroban_sdk::Vec<Val> = soroban_sdk::vec![
        &f.env,
        symbol_short!("register").into_val(&f.env),
        registrant.into_val(&f.env),
        FIXTURE_SCHEME_ID.into_val(&f.env),
    ];
    assert_eq!(event.1, expected, "full register topic tuple must match");
}

/// Calling `register_keys` a second time for the same (registrant, scheme_id)
/// emits another `register` event with the new meta-address.
///
/// Each call is a separate transaction in the Soroban test env, so events are
/// captured immediately after each call.
#[test]
fn register_update_emits_second_event_with_new_meta_address() {
    let f = setup();
    let registrant = Address::generate(&f.env);

    f.client.register_keys(&registrant, &FIXTURE_SCHEME_ID, &meta_address(&f.env));
    // Capture first register event right after first call.
    let first_event = find_event_by_sym(&f.env, symbol_short!("register"));
    let first_decoded: Bytes = FromVal::from_val(&f.env, &first_event.2);
    assert_eq!(first_decoded, meta_address(&f.env), "first call data must be the initial meta");

    let new_meta = Bytes::from_slice(&f.env, &[0xffu8; 64]);
    f.client.register_keys(&registrant, &FIXTURE_SCHEME_ID, &new_meta);
    // Capture second register event right after second call.
    let second_event = find_event_by_sym(&f.env, symbol_short!("register"));
    let second_decoded: Bytes = FromVal::from_val(&f.env, &second_event.2);
    assert_eq!(second_decoded, new_meta, "second call data must be the updated meta-address");
}

// ── remove_keys fixtures ──────────────────────────────────────────────────────

/// After register + remove, the remove invocation emits exactly one wire
/// `remove` event.
#[test]
fn remove_emits_exactly_one_event_after_register() {
    let f = setup();
    let registrant = Address::generate(&f.env);
    f.client.register_keys(&registrant, &FIXTURE_SCHEME_ID, &meta_address(&f.env));
    f.client.remove_keys(&registrant, &FIXTURE_SCHEME_ID);

    // After remove_keys, the current invocation's events include the remove
    // wire event and the metric event.  Count only the "remove" events.
    let sym_val: Val = symbol_short!("remove").into_val(&f.env);
    let count = f
        .env
        .events()
        .all()
        .iter()
        .filter(|(_, topics, _)| topics.first().map(|t| t.shallow_eq(&sym_val)) == Some(true))
        .count();
    assert_eq!(count, 1, "expected exactly one 'remove' event");
}

#[test]
fn remove_topic0_is_remove_symbol() {
    let f = setup();
    let registrant = Address::generate(&f.env);
    f.client.register_keys(&registrant, &FIXTURE_SCHEME_ID, &meta_address(&f.env));
    f.client.remove_keys(&registrant, &FIXTURE_SCHEME_ID);

    let event = find_event_by_sym(&f.env, symbol_short!("remove"));
    let got: soroban_sdk::Symbol = FromVal::from_val(&f.env, &event.1.get(0).unwrap());
    assert_eq!(got, symbol_short!("remove"));
}

#[test]
fn remove_full_topic_round_trip() {
    let f = setup();
    let registrant = Address::generate(&f.env);
    f.client.register_keys(&registrant, &FIXTURE_SCHEME_ID, &meta_address(&f.env));
    f.client.remove_keys(&registrant, &FIXTURE_SCHEME_ID);

    let event = find_event_by_sym(&f.env, symbol_short!("remove"));
    let expected: soroban_sdk::Vec<Val> = soroban_sdk::vec![
        &f.env,
        symbol_short!("remove").into_val(&f.env),
        registrant.into_val(&f.env),
        FIXTURE_SCHEME_ID.into_val(&f.env),
    ];
    assert_eq!(event.1, expected, "full remove topic tuple must match");
}

/// `remove_keys` data payload is unit `()`.
#[test]
fn remove_data_is_unit() {
    let f = setup();
    let registrant = Address::generate(&f.env);
    f.client.register_keys(&registrant, &FIXTURE_SCHEME_ID, &meta_address(&f.env));
    f.client.remove_keys(&registrant, &FIXTURE_SCHEME_ID);

    let event = find_event_by_sym(&f.env, symbol_short!("remove"));
    // unit() encodes to the void Val; from_val into () panics if the value is not void.
    let _unit: () = FromVal::from_val(&f.env, &event.2);
}

/// The "register" and "remove" event symbols are distinct.
///
/// Events are captured per-invocation since the soroban test env only retains
/// the most recent invocation's event log.
#[test]
fn register_and_remove_topic0_symbols_are_distinct() {
    let f = setup();
    let registrant = Address::generate(&f.env);

    f.client.register_keys(&registrant, &FIXTURE_SCHEME_ID, &meta_address(&f.env));
    let reg_event = find_event_by_sym(&f.env, symbol_short!("register"));
    let reg_sym: soroban_sdk::Symbol = FromVal::from_val(&f.env, &reg_event.1.get(0).unwrap());

    f.client.remove_keys(&registrant, &FIXTURE_SCHEME_ID);
    let rem_event = find_event_by_sym(&f.env, symbol_short!("remove"));
    let rem_sym: soroban_sdk::Symbol = FromVal::from_val(&f.env, &rem_event.1.get(0).unwrap());

    assert_ne!(reg_sym, rem_sym, "register and remove symbols must differ");
    assert_eq!(reg_sym, symbol_short!("register"));
    assert_eq!(rem_sym, symbol_short!("remove"));
}

/// Two different registrants each produce a `register` event carrying their
/// own address in topic[1].
///
/// Events are captured per-invocation since the soroban test env only retains
/// the most recent invocation's event log.
#[test]
fn multiple_registrants_emit_independent_events() {
    let f = setup();
    let alice = Address::generate(&f.env);
    let bob = Address::generate(&f.env);

    f.client.register_keys(&alice, &FIXTURE_SCHEME_ID, &meta_address(&f.env));
    let alice_event = find_event_by_sym(&f.env, symbol_short!("register"));
    let alice_from_event: Address =
        FromVal::from_val(&f.env, &alice_event.1.get(1).unwrap());

    f.client.register_keys(&bob, &FIXTURE_SCHEME_ID, &meta_address(&f.env));
    let bob_event = find_event_by_sym(&f.env, symbol_short!("register"));
    let bob_from_event: Address =
        FromVal::from_val(&f.env, &bob_event.1.get(1).unwrap());

    assert_eq!(alice_from_event, alice, "first event topic[1] must be alice");
    assert_eq!(bob_from_event, bob, "second event topic[1] must be bob");
    assert_ne!(alice_from_event, bob_from_event, "two registrants must produce distinct addresses");
}
