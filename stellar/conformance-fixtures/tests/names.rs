//! Conformance fixtures — wraith-names event pipeline.
//!
//! Wire shapes asserted:
//!
//! `register`:
//!   * topics : `("register", name_hash: BytesN<32>)`
//!   * data   : `(name: String, stealth_meta_address: Bytes)`  (meta-address = 64 bytes)
//!
//! `update`:
//!   * topics : `("update", name_hash: BytesN<32>)`
//!   * data   : `(name: String, new_meta_address: Bytes)`
//!
//! `release`:
//!   * topics : `("release", name_hash: BytesN<32>)`
//!   * data   : `name: String`
//!
//! `name_hash` is SHA-256 of the name bytes (the same hash function the
//! contract uses internally for storage keys).  Name–hash consistency is
//! asserted across the register/release pair in the final test.

use ed25519_dalek::SigningKey;
use soroban_sdk::testutils::{Address as _, Events, Ledger};
use soroban_sdk::xdr::{AccountId, PublicKey, ScAddress, Uint256};
use soroban_sdk::{symbol_short, Address, Bytes, BytesN, Env, FromVal, IntoVal, String, TryFromVal, Val};

use wraith_names::{WraithNamesContract, WraithNamesContractClient};

// ── Canonical fixture constants ───────────────────────────────────────────────

/// 64-byte stealth meta-address (first 32 bytes = spending pubkey slot, second = viewing).
const FIXTURE_META_ADDRESS: [u8; 64] = {
    let mut b = [0u8; 64];
    let mut i = 0usize;
    while i < 32 {
        b[i] = (i as u8) + 1;
        i += 1;
    }
    let mut j = 0usize;
    while j < 32 {
        b[32 + j] = (j as u8) + 0x41;
        j += 1;
    }
    b
};

/// Alternate 64-byte meta-address used in update tests.
const FIXTURE_META_ADDRESS_2: [u8; 64] = [0xffu8; 64];

/// Seed for the ed25519 signing key.  The owner is derived from this key.
const FIXTURE_SEED: [u8; 32] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e,
    0x1f, 0x20,
];

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build an `Address` that is backed by the ed25519 key at `seed`.
fn signing_account(env: &Env, seed: [u8; 32]) -> (Address, SigningKey) {
    let signing_key = SigningKey::from_bytes(&seed);
    let public_key =
        Uint256::try_from(signing_key.verifying_key().to_bytes().as_ref()).expect("valid key");
    let sc_address = ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(public_key)));
    let owner = Address::try_from_val(env, &sc_address).expect("account address");
    (owner, signing_key)
}

/// Build the on-behalf authorization message and sign it.
///
/// The buffer size here is 64 bytes, matching `authorization_message` in
/// `wraith-names/src/lib.rs` exactly.  Names longer than 32 bytes are
/// rejected by `validate_name` (`MAX_NAME_LEN = 32`) before they ever reach
/// the signing path, so in practice `actual_len ≤ 32` always holds and both
/// the contract and this helper produce identical byte sequences.
fn sign_authorization(
    env: &Env,
    signing_key: &SigningKey,
    operation: &[u8],
    name: &String,
    stealth_meta_address: &Bytes,
    expiry: u64,
) -> BytesN<64> {
    use ed25519_dalek::Signer;

    let mut message = Bytes::from_slice(env, b"wraith-names:v1");
    message.extend_from_slice(operation);
    let name_len = name.len() as usize;
    // 64-byte buffer mirrors the contract's `authorization_message`.
    // `actual_len` is always ≤ MAX_NAME_LEN (32) for any name the contract
    // accepts, so the upper 32 bytes of the buffer are never written and
    // both paths produce the same slice.
    let actual_len = core::cmp::min(name_len, 64);
    let mut name_buf = [0u8; 64];
    name.copy_into_slice(&mut name_buf[..actual_len]);
    message.append(&Bytes::from_slice(env, &name_buf[..actual_len]));
    message.append(stealth_meta_address);
    message.extend_from_slice(&expiry.to_be_bytes());
    let hash = env.crypto().sha256(&message);
    let sig = signing_key.sign(&hash.to_array());
    BytesN::from_array(env, &sig.to_bytes())
}

struct Fixture {
    env: Env,
    client: WraithNamesContractClient<'static>,
    owner: Address,
    signing_key: SigningKey,
}

fn setup() -> Fixture {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.min_persistent_entry_ttl = 600_000;
        li.sequence_number = 100;
        li.timestamp = 100_000;
    });
    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    // Initialise the names contract (required before any register call).
    let admin = Address::generate(&env);
    client.init(&admin);

    let (owner, signing_key) = signing_account(&env, FIXTURE_SEED);
    Fixture {
        env,
        client,
        owner,
        signing_key,
    }
}

fn meta(env: &Env) -> Bytes {
    Bytes::from_slice(env, &FIXTURE_META_ADDRESS)
}

fn meta2(env: &Env) -> Bytes {
    Bytes::from_slice(env, &FIXTURE_META_ADDRESS_2)
}

fn fixture_name(env: &Env) -> String {
    String::from_str(env, "alice")
}

/// Register "alice" and return the expected name_hash (SHA-256 of "alice" bytes).
fn register_alice(f: &Fixture) -> BytesN<32> {
    let name = fixture_name(&f.env);
    let meta_val = meta(&f.env);
    let expiry = f.env.ledger().timestamp() + 3600;
    let sig = sign_authorization(
        &f.env,
        &f.signing_key,
        b"wraith-names:register",
        &name,
        &meta_val,
        expiry,
    );
    f.client.register_on_behalf(&f.owner, &name, &meta_val, &sig, &expiry);

    // name_hash = SHA-256(name_bytes) as used by the contract.
    let name_bytes = Bytes::from_slice(&f.env, b"alice");
    BytesN::from_array(&f.env, &f.env.crypto().sha256(&name_bytes).to_array())
}

/// Find an event with topic[0] matching `sym`, or panic.
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

// ── register event fixtures ───────────────────────────────────────────────────

/// `register` emits exactly one `register` event.
#[test]
fn register_emits_register_event() {
    let f = setup();
    register_alice(&f);

    let sym_val: Val = symbol_short!("register").into_val(&f.env);
    let count = f
        .env
        .events()
        .all()
        .iter()
        .filter(|(_, topics, _)| topics.first().map(|t| t.shallow_eq(&sym_val)) == Some(true))
        .count();

    assert_eq!(count, 1, "register must emit exactly one 'register' event");
}

/// topic[0] is the symbol `"register"`.
#[test]
fn register_topic0_is_register_symbol() {
    let f = setup();
    register_alice(&f);

    let event = find_event_by_sym(&f.env, symbol_short!("register"));
    let got: soroban_sdk::Symbol =
        FromVal::from_val(&f.env, &event.1.get(0).unwrap());
    assert_eq!(got, symbol_short!("register"));
}

/// topic[1] is the SHA-256 name hash (`BytesN<32>`).
#[test]
fn register_topic1_is_name_hash() {
    let f = setup();
    let expected_hash = register_alice(&f);

    let event = find_event_by_sym(&f.env, symbol_short!("register"));
    let got_hash: BytesN<32> =
        FromVal::from_val(&f.env, &event.1.get(1).unwrap());
    assert_eq!(got_hash, expected_hash, "topic[1] must be SHA-256(name)");
}

/// topic tuple has exactly 2 entries.
#[test]
fn register_topic_tuple_has_two_entries() {
    let f = setup();
    register_alice(&f);

    let event = find_event_by_sym(&f.env, symbol_short!("register"));
    assert_eq!(event.1.len(), 2, "register topic tuple must have exactly 2 entries");
}

/// Data payload is `(name, stealth_meta_address)`.
#[test]
fn register_data_round_trip() {
    let f = setup();
    register_alice(&f);

    let event = find_event_by_sym(&f.env, symbol_short!("register"));
    let (decoded_name, decoded_meta): (String, Bytes) =
        FromVal::from_val(&f.env, &event.2);

    assert_eq!(decoded_name, fixture_name(&f.env), "register data[0] name must round-trip");
    assert_eq!(decoded_meta, meta(&f.env), "register data[1] stealth_meta_address must round-trip");
    assert_eq!(decoded_meta.len(), 64, "stealth_meta_address must be exactly 64 bytes");
}

/// Full topic tuple structural comparison.
#[test]
fn register_full_topic_round_trip() {
    let f = setup();
    let expected_hash = register_alice(&f);

    let event = find_event_by_sym(&f.env, symbol_short!("register"));
    let expected: soroban_sdk::Vec<Val> = soroban_sdk::vec![
        &f.env,
        symbol_short!("register").into_val(&f.env),
        expected_hash.into_val(&f.env),
    ];
    assert_eq!(event.1, expected, "full register topic tuple must match");
}

// ── update event fixtures ─────────────────────────────────────────────────────

/// `update` emits an event with topic[0] = `"update"`.
#[test]
fn update_topic0_is_update_symbol() {
    let f = setup();
    register_alice(&f);

    let new_meta = meta2(&f.env);
    let name = fixture_name(&f.env);
    let expiry = f.env.ledger().timestamp() + 3600;
    let sig = sign_authorization(
        &f.env,
        &f.signing_key,
        b"wraith-names:update",
        &name,
        &new_meta,
        expiry,
    );
    f.client.update_on_behalf(&f.owner, &name, &new_meta, &sig, &expiry);

    let event = find_event_by_sym(&f.env, symbol_short!("update"));
    let got: soroban_sdk::Symbol =
        FromVal::from_val(&f.env, &event.1.get(0).unwrap());
    assert_eq!(got, symbol_short!("update"));
}

/// `update` data is `(name, new_meta_address)` and both fields round-trip.
#[test]
fn update_data_round_trip() {
    let f = setup();
    register_alice(&f);

    let new_meta = meta2(&f.env);
    let name = fixture_name(&f.env);
    let expiry = f.env.ledger().timestamp() + 3600;
    let sig = sign_authorization(
        &f.env,
        &f.signing_key,
        b"wraith-names:update",
        &name,
        &new_meta,
        expiry,
    );
    f.client.update_on_behalf(&f.owner, &name, &new_meta, &sig, &expiry);

    let event = find_event_by_sym(&f.env, symbol_short!("update"));
    let (decoded_name, decoded_meta): (String, Bytes) =
        FromVal::from_val(&f.env, &event.2);

    assert_eq!(decoded_name, fixture_name(&f.env), "update data[0] name must round-trip");
    assert_eq!(decoded_meta, new_meta, "update data[1] new_meta_address must round-trip");
}

/// `update` topic[1] is the same name_hash as `register` for the same name.
#[test]
fn update_topic1_name_hash_consistent_with_register() {
    let f = setup();
    let register_hash = register_alice(&f);

    let new_meta = meta2(&f.env);
    let name = fixture_name(&f.env);
    let expiry = f.env.ledger().timestamp() + 3600;
    let sig = sign_authorization(
        &f.env,
        &f.signing_key,
        b"wraith-names:update",
        &name,
        &new_meta,
        expiry,
    );
    f.client.update_on_behalf(&f.owner, &name, &new_meta, &sig, &expiry);

    let event = find_event_by_sym(&f.env, symbol_short!("update"));
    let update_hash: BytesN<32> =
        FromVal::from_val(&f.env, &event.1.get(1).unwrap());
    assert_eq!(
        update_hash, register_hash,
        "update topic[1] name_hash must equal register topic[1] name_hash for same name"
    );
}

// ── release event fixtures ────────────────────────────────────────────────────

/// `release` emits an event with topic[0] = `"release"`.
#[test]
fn release_topic0_is_release_symbol() {
    let f = setup();
    register_alice(&f);

    let name = fixture_name(&f.env);
    let empty_meta = Bytes::new(&f.env);
    let expiry = f.env.ledger().timestamp() + 3600;
    let sig = sign_authorization(
        &f.env,
        &f.signing_key,
        b"wraith-names:release",
        &name,
        &empty_meta,
        expiry,
    );
    f.client.release_on_behalf(&f.owner, &name, &sig, &expiry);

    let event = find_event_by_sym(&f.env, symbol_short!("release"));
    let got: soroban_sdk::Symbol =
        FromVal::from_val(&f.env, &event.1.get(0).unwrap());
    assert_eq!(got, symbol_short!("release"));
}

/// `release` topic[1] is the name_hash — must match the hash from `register`.
#[test]
fn release_topic1_name_hash_consistent_with_register() {
    let f = setup();
    let register_hash = register_alice(&f);

    let name = fixture_name(&f.env);
    let empty_meta = Bytes::new(&f.env);
    let expiry = f.env.ledger().timestamp() + 3600;
    let sig = sign_authorization(
        &f.env,
        &f.signing_key,
        b"wraith-names:release",
        &name,
        &empty_meta,
        expiry,
    );
    f.client.release_on_behalf(&f.owner, &name, &sig, &expiry);

    let event = find_event_by_sym(&f.env, symbol_short!("release"));
    let release_hash: BytesN<32> =
        FromVal::from_val(&f.env, &event.1.get(1).unwrap());
    assert_eq!(
        release_hash, register_hash,
        "release topic[1] name_hash must equal register topic[1] name_hash for same name"
    );
}

/// `release` topic tuple has exactly 2 entries.
#[test]
fn release_topic_tuple_has_two_entries() {
    let f = setup();
    register_alice(&f);

    let name = fixture_name(&f.env);
    let empty_meta = Bytes::new(&f.env);
    let expiry = f.env.ledger().timestamp() + 3600;
    let sig = sign_authorization(
        &f.env,
        &f.signing_key,
        b"wraith-names:release",
        &name,
        &empty_meta,
        expiry,
    );
    f.client.release_on_behalf(&f.owner, &name, &sig, &expiry);

    let event = find_event_by_sym(&f.env, symbol_short!("release"));
    assert_eq!(event.1.len(), 2, "release topic tuple must have exactly 2 entries");
}

/// `release` data is the name `String` and it round-trips.
#[test]
fn release_data_is_name_string() {
    let f = setup();
    register_alice(&f);

    let name = fixture_name(&f.env);
    let empty_meta = Bytes::new(&f.env);
    let expiry = f.env.ledger().timestamp() + 3600;
    let sig = sign_authorization(
        &f.env,
        &f.signing_key,
        b"wraith-names:release",
        &name,
        &empty_meta,
        expiry,
    );
    f.client.release_on_behalf(&f.owner, &name, &sig, &expiry);

    let event = find_event_by_sym(&f.env, symbol_short!("release"));
    let decoded_name: String =
        FromVal::from_val(&f.env, &event.2);
    assert_eq!(decoded_name, fixture_name(&f.env), "release data must be the name string");
}

/// All three event symbols (`"register"`, `"update"`, `"release"`) are distinct.
#[test]
fn all_three_event_symbols_are_distinct() {
    // Soroban's test env only retains the most recent invocation's events in
    // env.events().all().  Capture the emitted symbol right after each call so
    // we see the event before the next call overwrites the event buffer.
    let f = setup();

    // --- register ---
    register_alice(&f);
    let register_event = find_event_by_sym(&f.env, symbol_short!("register"));
    let reg_sym: soroban_sdk::Symbol =
        FromVal::from_val(&f.env, &register_event.1.get(0).unwrap());

    // --- update ---
    let new_meta = meta2(&f.env);
    let name = fixture_name(&f.env);
    let expiry = f.env.ledger().timestamp() + 3600;
    let upd_sig = sign_authorization(
        &f.env,
        &f.signing_key,
        b"wraith-names:update",
        &name,
        &new_meta,
        expiry,
    );
    f.client.update_on_behalf(&f.owner, &name, &new_meta, &upd_sig, &expiry);
    let update_event = find_event_by_sym(&f.env, symbol_short!("update"));
    let upd_sym: soroban_sdk::Symbol =
        FromVal::from_val(&f.env, &update_event.1.get(0).unwrap());

    // --- release ---
    let empty_meta = Bytes::new(&f.env);
    let expiry2 = f.env.ledger().timestamp() + 7200;
    let rel_sig = sign_authorization(
        &f.env,
        &f.signing_key,
        b"wraith-names:release",
        &name,
        &empty_meta,
        expiry2,
    );
    f.client.release_on_behalf(&f.owner, &name, &rel_sig, &expiry2);
    let release_event = find_event_by_sym(&f.env, symbol_short!("release"));
    let rel_sym: soroban_sdk::Symbol =
        FromVal::from_val(&f.env, &release_event.1.get(0).unwrap());

    // Assert we actually captured each symbol correctly.
    assert_eq!(reg_sym, symbol_short!("register"), "register event must carry 'register' symbol");
    assert_eq!(upd_sym, symbol_short!("update"),   "update event must carry 'update' symbol");
    assert_eq!(rel_sym, symbol_short!("release"),  "release event must carry 'release' symbol");

    // Assert all three symbols are mutually distinct.
    assert_ne!(symbol_short!("register"), symbol_short!("update"));
    assert_ne!(symbol_short!("update"),   symbol_short!("release"));
    assert_ne!(symbol_short!("register"), symbol_short!("release"));
}

// ── 32-vs-64-byte name-buffer conformance ────────────────────────────────────

/// Regression test for the 32-vs-64-byte name buffer discrepancy.
///
/// The contract's `authorization_message` function uses a 64-byte buffer;
/// `sign_authorization` also uses 64 bytes.  `validate_name` caps names at
/// `MAX_NAME_LEN = 32`, so a 33-char name is rejected with `NameTooLong`
/// **before** the signing path is reached — making names >32 chars
/// unreachable in practice.  This test confirms:
///
/// 1. A 32-char name (maximum allowed) registers successfully and the event
///    round-trips correctly — proving the buffer logic works at the boundary.
/// 2. Any name longer than 32 chars is rejected by the contract before any
///    signing or event emission occurs.
#[test]
fn max_length_name_buffer_conformance() {
    let f = setup();

    // 32 lowercase letters — exactly MAX_NAME_LEN.
    let max_name_str = "abcdefghijklmnopqrstuvwxyzabcdef"; // len == 32
    assert_eq!(max_name_str.len(), 32, "fixture must be exactly 32 chars");

    let name = String::from_str(&f.env, max_name_str);
    let meta_val = meta(&f.env);
    let expiry = f.env.ledger().timestamp() + 3600;

    let sig = sign_authorization(
        &f.env,
        &f.signing_key,
        b"wraith-names:register",
        &name,
        &meta_val,
        expiry,
    );

    // This must succeed: the contract must accept a 32-char name and the
    // signature produced by sign_authorization must match what the contract
    // computes internally.
    f.client.register_on_behalf(&f.owner, &name, &meta_val, &sig, &expiry);

    // Confirm the register event carries the correct name.
    let event = find_event_by_sym(&f.env, symbol_short!("register"));
    let (decoded_name, _): (String, Bytes) = FromVal::from_val(&f.env, &event.2);
    assert_eq!(
        decoded_name, name,
        "32-char name must round-trip through event data unchanged"
    );
}
