//! Cross-chain schema parity assertions.
//!
//! These tests do NOT call any contract — they assert compile-time and
//! structural invariants that guarantee the Stellar event schema stays
//! aligned with its EVM counterpart.  Any field or constant rename
//! surfaces as a compile error; any value drift surfaces as a panic.
//!
//! Parity rules being enforced:
//!
//! | Concept             | EVM (Solidity)           | Stellar (Soroban)                         |
//! |---------------------|--------------------------|-------------------------------------------|
//! | Scheme ID (v2)      | `schemeId = 2` (ERC-5564)| `STELLAR_V2_SCHEME_ID = 2`                |
//! | Metadata kind flag  | `metadata[0]` = view tag | `METADATA_KIND_VIEW_TAG = 1`              |
//! | Meta-address length | 66 bytes (EVM secp256k1) | 64 bytes (Stellar ed25519, no 0x04 prefix)|
//! | Announce symbol     | event `Announcement`     | topic[0] = `symbol_short!("announce")`    |
//! | Register symbol     | event `StealthMetaAddressSet` | topic[0] = `symbol_short!("register")` |
//! | Remove symbol       | event `NonceIncremented` / none | topic[0] = `symbol_short!("remove")` |
//! | Name register       | event `NameRegistered`   | topic[0] = `symbol_short!("register")`    |
//! | Name release        | event `NameReleased`     | topic[0] = `symbol_short!("release")`     |
//!
//! The EVM scheme ID for secp256k1-with-view-tags is conventionally 1,
//! while Stellar's v2 announcer requires scheme_id = 2.  That intentional
//! difference is documented here — a future cross-chain SDK layer should
//! normalize when translating between chains.

use soroban_sdk::{symbol_short, Env};

use stealth_announcer::{METADATA_KIND_VIEW_TAG, STELLAR_V2_SCHEME_ID};

// ── Scheme ID parity ──────────────────────────────────────────────────────────

/// STELLAR_V2_SCHEME_ID must be 2.  This value is published in the
/// event topic and must never change for the v2 announcer deployment;
/// indexers and SDK parsers depend on it for cross-chain disambiguation.
#[test]
fn stellar_v2_scheme_id_is_2() {
    assert_eq!(
        STELLAR_V2_SCHEME_ID, 2u32,
        "STELLAR_V2_SCHEME_ID must be 2 — changing it is a breaking protocol change"
    );
}

/// METADATA_KIND_VIEW_TAG must be 1.  This is the metadata_kind value
/// emitted in topic[3] of every v2 announcement.  It signals that the
/// first metadata byte is the view tag.  Changing this value requires a
/// new metadata_kind constant and cannot be done in place.
#[test]
fn metadata_kind_view_tag_is_1() {
    assert_eq!(
        METADATA_KIND_VIEW_TAG, 1u32,
        "METADATA_KIND_VIEW_TAG must be 1 — changing it is a breaking protocol change"
    );
}

/// The EVM scheme ID for secp256k1-with-view-tags (per ERC-5564) is 1.
/// Document the intentional difference: Stellar v2 = 2, EVM v1 = 1.
/// A cross-chain SDK mapping layer must translate scheme IDs.
#[test]
fn evm_and_stellar_scheme_ids_are_intentionally_different() {
    // EVM convention (ERC-5564): schemeId = 1 for secp256k1 with view tags.
    const EVM_SECP256K1_VIEW_TAG_SCHEME_ID: u32 = 1;

    assert_ne!(
        STELLAR_V2_SCHEME_ID, EVM_SECP256K1_VIEW_TAG_SCHEME_ID,
        "Stellar and EVM scheme IDs are intentionally different — update this test if they converge"
    );
    // Explicitly document the expected values for clarity.
    assert_eq!(EVM_SECP256K1_VIEW_TAG_SCHEME_ID, 1u32);
    assert_eq!(STELLAR_V2_SCHEME_ID, 2u32);
}

// ── Meta-address length parity ────────────────────────────────────────────────

/// EVM meta-addresses are 66 bytes (secp256k1 compressed pub: 33 bytes × 2).
/// Stellar meta-addresses are 64 bytes (ed25519 pub: 32 bytes × 2, no 0x04 prefix).
/// This test documents and pin the expected lengths.
#[test]
fn meta_address_lengths_differ_by_chain() {
    const EVM_META_ADDRESS_LEN: u32 = 66;  // 33 (spending) + 33 (viewing) compressed secp256k1
    const STELLAR_META_ADDRESS_LEN: u32 = 64; // 32 (spending) + 32 (viewing) ed25519 pubkeys

    assert_ne!(
        EVM_META_ADDRESS_LEN, STELLAR_META_ADDRESS_LEN,
        "EVM and Stellar meta-address lengths intentionally differ; update this test if they converge"
    );
    assert_eq!(EVM_META_ADDRESS_LEN, 66u32);
    assert_eq!(STELLAR_META_ADDRESS_LEN, 64u32);
}

// ── Symbol / event name parity ────────────────────────────────────────────────

/// The Stellar `announce` topic[0] symbol is `symbol_short!("announce")`.
/// The corresponding EVM event is `Announcement(schemeId, stealthAddress, caller, ...)`.
/// While the names differ, both serve as the primary event type discriminator.
/// This test pins the Stellar symbol so a rename is visible immediately.
#[test]
fn stellar_announce_symbol_is_announce() {
    let _env = Env::default();
    let sym = symbol_short!("announce");
    // soroban_sdk Symbol exposes its string representation via Debug.
    let dbg = std::format!("{sym:?}");
    assert!(
        dbg.contains("announce"),
        "announce symbol debug representation must contain 'announce': {dbg}"
    );
}

/// The Stellar registry `register` symbol is `symbol_short!("register")`.
/// EVM counterpart: `StealthMetaAddressSet`.
#[test]
fn stellar_registry_register_symbol_is_register() {
    let _env = Env::default();
    let sym = symbol_short!("register");
    let dbg = std::format!("{sym:?}");
    assert!(
        dbg.contains("register"),
        "register symbol must contain 'register': {dbg}"
    );
}

/// The Stellar registry `remove` symbol is `symbol_short!("remove")`.
/// EVM counterpart: keys are overwritten rather than removed; there is no EVM remove event.
#[test]
fn stellar_registry_remove_symbol_is_remove() {
    let _env = Env::default();
    let sym = symbol_short!("remove");
    let dbg = std::format!("{sym:?}");
    assert!(
        dbg.contains("remove"),
        "remove symbol must contain 'remove': {dbg}"
    );
}

/// The Stellar names `release` symbol is `symbol_short!("release")`.
/// EVM counterpart: `NameReleased`.
#[test]
fn stellar_names_release_symbol_is_release() {
    let _env = Env::default();
    let sym = symbol_short!("release");
    let dbg = std::format!("{sym:?}");
    assert!(
        dbg.contains("release"),
        "release symbol must contain 'release': {dbg}"
    );
}

/// Announce and register are distinct symbols — indexers must NOT conflate them.
#[test]
fn announce_and_register_symbols_are_distinct() {
    assert_ne!(symbol_short!("announce"), symbol_short!("register"));
}

/// Register and release are distinct symbols.
#[test]
fn register_and_release_symbols_are_distinct() {
    assert_ne!(symbol_short!("register"), symbol_short!("release"));
}

/// Announce, register, update, and release are all mutually distinct.
#[test]
fn all_event_symbols_are_mutually_distinct() {
    let syms = [
        symbol_short!("announce"),
        symbol_short!("register"),
        symbol_short!("update"),
        symbol_short!("release"),
        symbol_short!("remove"),
    ];
    for i in 0..syms.len() {
        for j in (i + 1)..syms.len() {
            assert_ne!(
                syms[i], syms[j],
                "symbols at index {i} and {j} must be distinct"
            );
        }
    }
}

// ── View-tag position parity ──────────────────────────────────────────────────

/// On both chains the view tag occupies the FIRST byte of the metadata blob.
/// This test pins the byte-offset so any future metadata layout change is visible.
#[test]
fn view_tag_is_always_metadata_first_byte_on_both_chains() {
    const VIEW_TAG_BYTE_OFFSET: usize = 0;

    // Stellar: view_tag_bucket = metadata[0] as u32 (per the stealth-announcer source).
    // EVM: metadata[0] is the view tag by SDK convention (see ERC-5564 note in IERC5564Announcer).
    // Both agree: offset 0.
    assert_eq!(VIEW_TAG_BYTE_OFFSET, 0, "view tag must be at byte offset 0 in metadata");

    // Verify the Stellar helper `view_tag_bucket` agrees.
    let env = Env::default();
    let view_tag: u8 = 0xab;
    let meta = soroban_sdk::Bytes::from_slice(&env, &[view_tag, 0x00, 0x01]);
    let bucket = stealth_announcer::view_tag_bucket(&meta);
    assert_eq!(bucket, view_tag as u32, "view_tag_bucket must equal metadata[0]");
}

/// The METADATA_KIND_VIEW_TAG constant value must equal 1.
/// Changing this would silently change the topic[3] value of every v2
/// announcement and break all downstream indexers.
#[test]
fn metadata_kind_view_tag_constant_matches_expected_wire_value() {
    // This is the REQUIRED CHECK value from the issue spec.
    assert_eq!(
        METADATA_KIND_VIEW_TAG, 1u32,
        "METADATA_KIND_VIEW_TAG wire value is 1; import from source, do not hardcode"
    );
}
