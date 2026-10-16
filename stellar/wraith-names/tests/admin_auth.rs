//! Pause admin and core-operation tests for the wraith-names contract.
//!
//! ## What wraith-names actually implements (as of this file)
//!
//! wraith-names has a **pause admin** (`DataKey::Admin`) that can pause and
//! unpause the contract.  It does NOT yet have an `upgrade` function, a
//! `renounce_admin` function, or an upgrade timelock.  The multisig machinery
//! (`MultisigSigners` / `MultisigThreshold` / `PendingRotation`) governs
//! rotation of the protocol signer set and the auction admin — it is
//! independent of any upgrade path.
//!
//! The upgrade / timelock / renunciation / social-recovery infrastructure is
//! tracked for future implementation; see GOVERNANCE.md §Per-Contract
//! Upgradability Decisions and the follow-up issue filed after #239.
//!
//! ## What these tests cover
//!
//! - Pause admin initialisation and access control (`init`, `pause`,
//!   `unpause`).
//! - That pausing does not break name resolution (read-only path stays up).
//! - That name registrations persist across arbitrary ledger advances
//!   (the property that upgrade state-preservation tests were trying to proxy).
//! - That normal CRUD operations always remain available (register / update /
//!   release / resolve).

#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    Address, Bytes, Env, String,
};
use wraith_names::{NamesError, WraithNamesContract, WraithNamesContractClient};

// ── helpers ──────────────────────────────────────────────────────────────────

fn setup_env() -> (Env, Address, WraithNamesContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    {
        let mut info = env.ledger().get();
        info.min_persistent_entry_ttl = 200_000;
        info.max_entry_ttl = 300_000;
        env.ledger().set(info);
    }
    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);
    (env, contract_id, client)
}

// ── pause-admin init & access control ────────────────────────────────────────

/// The admin set at `init` time can pause the contract.
#[test]
fn test_admin_can_pause() {
    let (env, _, client) = setup_env();
    let admin = Address::generate(&env);
    client.init(&admin);

    assert!(!client.is_paused());
    client.pause(&admin);
    assert!(client.is_paused());
}

/// Only the pause admin can pause; a non-admin caller panics.
#[test]
#[should_panic]
fn test_non_admin_cannot_pause() {
    let (env, _, client) = setup_env();
    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    client.init(&admin);

    client.pause(&non_admin);
}

/// The pause admin can unpause the contract after pausing it.
#[test]
fn test_admin_can_unpause() {
    let (env, _, client) = setup_env();
    let admin = Address::generate(&env);
    client.init(&admin);

    client.pause(&admin);
    assert!(client.is_paused());

    client.unpause(&admin);
    assert!(!client.is_paused());
}

/// Only the pause admin can unpause; a non-admin caller panics.
#[test]
#[should_panic]
fn test_non_admin_cannot_unpause() {
    let (env, _, client) = setup_env();
    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    client.init(&admin);

    client.pause(&admin);
    client.unpause(&non_admin); // should panic
}

/// `init` is idempotent — calling it a second time with a different address
/// does not replace the original admin.
#[test]
fn test_init_is_idempotent() {
    let (env, _, client) = setup_env();
    let original_admin = Address::generate(&env);
    let interloper = Address::generate(&env);

    client.init(&original_admin);
    client.init(&interloper); // second call must be a no-op

    // Original admin still works; interloper does not control pause.
    client.pause(&original_admin);
    assert!(client.is_paused());
}

// ── pause interaction with user operations ────────────────────────────────────

/// While paused, state-mutating operations (register, update, release,
/// extend_name_ttl) are all rejected with `Paused`.
#[test]
fn test_paused_contract_rejects_mutations() {
    let (env, _, client) = setup_env();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.init(&admin);

    // Register a name while unpaused.
    let name = String::from_str(&env, "alice");
    let meta = Bytes::from_slice(&env, &[1u8; 64]);
    client.register(&user, &name, &meta);

    client.pause(&admin);

    // register
    let name2 = String::from_str(&env, "bravo");
    let meta2 = Bytes::from_slice(&env, &[2u8; 64]);
    assert_eq!(
        client.try_register(&user, &name2, &meta2),
        Err(Ok(NamesError::Paused))
    );

    // update
    let new_meta = Bytes::from_slice(&env, &[3u8; 64]);
    assert_eq!(
        client.try_update(&user, &name, &new_meta),
        Err(Ok(NamesError::Paused))
    );

    // release
    assert_eq!(
        client.try_release(&user, &name),
        Err(Ok(NamesError::Paused))
    );

    // extend_name_ttl (advance ledger so extend_to_ledger is in the future)
    let future_ledger = env.ledger().sequence() + 100;
    assert_eq!(
        client.try_extend_name_ttl(&name, &future_ledger),
        Err(Ok(NamesError::Paused))
    );
}

/// While paused, read-only operations (`resolve`, `name_of`) remain
/// available so users can always look up their names.
#[test]
fn test_paused_contract_allows_reads() {
    let (env, _, client) = setup_env();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.init(&admin);

    let name = String::from_str(&env, "carol");
    let meta = Bytes::from_slice(&env, &[42u8; 64]);
    client.register(&user, &name, &meta);

    client.pause(&admin);

    // resolve still works
    assert_eq!(client.resolve(&name), meta);
    // name_of still works
    assert_eq!(client.name_of(&meta), name);
}

/// After unpausing, all previously blocked operations work again.
#[test]
fn test_operations_resume_after_unpause() {
    let (env, _, client) = setup_env();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    client.init(&admin);

    client.pause(&admin);
    client.unpause(&admin);

    let name = String::from_str(&env, "dave");
    let meta = Bytes::from_slice(&env, &[5u8; 64]);
    client.register(&user, &name, &meta);
    assert_eq!(client.resolve(&name), meta);
}

// ── name persistence ──────────────────────────────────────────────────────────

/// Name registrations persist after arbitrary ledger advances.
///
/// This replaces the vacuous "post-upgrade state preserved" stub that was
/// present before: the real guarantee is that persistent storage survives
/// as long as TTLs are extended, which is independent of any upgrade.
#[test]
fn test_name_registrations_survive_ledger_advances() {
    let (env, _, client) = setup_env();

    let user1 = Address::generate(&env);
    let name1 = String::from_str(&env, "alice");
    let meta1 = Bytes::from_slice(&env, &[1u8; 64]);
    client.register(&user1, &name1, &meta1);

    let user2 = Address::generate(&env);
    let name2 = String::from_str(&env, "bob");
    let meta2 = Bytes::from_slice(&env, &[2u8; 64]);
    client.register(&user2, &name2, &meta2);

    let user3 = Address::generate(&env);
    let name3 = String::from_str(&env, "carol");
    let meta3 = Bytes::from_slice(&env, &[3u8; 64]);
    client.register(&user3, &name3, &meta3);

    // Advance the ledger by a significant amount (staying within TTL window).
    let new_seq = env.ledger().sequence() + 50_000;
    env.ledger().set_sequence_number(new_seq);

    // All names still resolve.
    assert_eq!(client.resolve(&name1), meta1);
    assert_eq!(client.resolve(&name2), meta2);
    assert_eq!(client.resolve(&name3), meta3);

    // Reverse lookups work too.
    assert_eq!(client.name_of(&meta1), name1);
    assert_eq!(client.name_of(&meta2), name2);
    assert_eq!(client.name_of(&meta3), name3);
}

// ── normal CRUD always available ──────────────────────────────────────────────

/// Register, update, release, and resolve all work in the unpaused state.
/// This test documents the baseline contract functionality independently of
/// any upgrade or admin lifecycle.
#[test]
fn test_core_operations_work_normally() {
    let (env, _, client) = setup_env();

    let user = Address::generate(&env);
    let name = String::from_str(&env, "frank");
    let meta = Bytes::from_slice(&env, &[7u8; 64]);

    // register
    client.register(&user, &name, &meta);
    assert_eq!(client.resolve(&name), meta);

    // update
    let new_meta = Bytes::from_slice(&env, &[8u8; 64]);
    client.update(&user, &name, &new_meta);
    assert_eq!(client.resolve(&name), new_meta);

    // release — name no longer resolvable
    client.release(&user, &name);
    assert!(client.try_resolve(&name).is_err());

    // re-register by a different user
    let user2 = Address::generate(&env);
    let meta3 = Bytes::from_slice(&env, &[9u8; 64]);
    client.register(&user2, &name, &meta3);
    assert_eq!(client.resolve(&name), meta3);
}

/// Pause state does not affect name ownership: after unpausing, only the
/// original owner can update or release their name.
#[test]
fn test_ownership_preserved_through_pause_cycle() {
    let (env, _, client) = setup_env();
    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    let attacker = Address::generate(&env);
    client.init(&admin);

    let name = String::from_str(&env, "grace");
    let meta = Bytes::from_slice(&env, &[10u8; 64]);
    client.register(&owner, &name, &meta);

    // Pause and unpause.
    client.pause(&admin);
    client.unpause(&admin);

    // Attacker still cannot update the name.
    let bad_meta = Bytes::from_slice(&env, &[99u8; 64]);
    assert_eq!(
        client.try_update(&attacker, &name, &bad_meta),
        Err(Ok(NamesError::NotOwner))
    );

    // Owner can still update normally.
    client.update(&owner, &name, &bad_meta);
    assert_eq!(client.resolve(&name), bad_meta);
}
