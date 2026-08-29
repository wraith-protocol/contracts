use soroban_sdk::testutils::{Address as _, Events};
use soroban_sdk::{Address, Bytes, Env, String, Vec};
use wraith_names::{NamesError, WraithNamesContract, WraithNamesContractClient};

fn setup() -> (Env, WraithNamesContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);
    (env, client)
}

/// 1. Happy path: bulk_register registers all names atomically.
#[test]
fn test_bulk_register_all_succeed() {
    let (env, client) = setup();
    let owner = Address::generate(&env);

    let names = Vec::from_array(
        &env,
        [
            String::from_str(&env, "app"),
            String::from_str(&env, "docs"),
            String::from_str(&env, "pay"),
        ],
    );
    let metas = Vec::from_array(
        &env,
        [
            Bytes::from_slice(&env, &[1u8; 64]),
            Bytes::from_slice(&env, &[2u8; 64]),
            Bytes::from_slice(&env, &[3u8; 64]),
        ],
    );

    let result = client.try_bulk_register(&owner, &names, &metas);
    assert_eq!(result, Ok(Ok(())));

    assert_eq!(
        client.resolve(&String::from_str(&env, "app")),
        Bytes::from_slice(&env, &[1u8; 64])
    );
    assert_eq!(
        client.resolve(&String::from_str(&env, "docs")),
        Bytes::from_slice(&env, &[2u8; 64])
    );
    assert_eq!(
        client.resolve(&String::from_str(&env, "pay")),
        Bytes::from_slice(&env, &[3u8; 64])
    );
}

/// 2. Atomicity: if one name is taken, the entire bulk_register reverts.
#[test]
fn test_bulk_register_atomic_revert_on_taken() {
    let (env, client) = setup();
    let owner = Address::generate(&env);

    // Pre-register one name
    client.register(
        &owner,
        &String::from_str(&env, "taken"),
        &Bytes::from_slice(&env, &[1u8; 64]),
    );

    let names = Vec::from_array(
        &env,
        [
            String::from_str(&env, "free1"),
            String::from_str(&env, "taken"),
            String::from_str(&env, "free2"),
        ],
    );
    let metas = Vec::from_array(
        &env,
        [
            Bytes::from_slice(&env, &[1u8; 64]),
            Bytes::from_slice(&env, &[2u8; 64]),
            Bytes::from_slice(&env, &[3u8; 64]),
        ],
    );

    let result = client.try_bulk_register(&owner, &names, &metas);
    assert_eq!(result, Err(Ok(NamesError::NameTaken)));

    // Verify none of the free names were registered
    assert_eq!(
        client.try_resolve(&String::from_str(&env, "free1")),
        Err(Ok(NamesError::NameNotFound))
    );
    assert_eq!(
        client.try_resolve(&String::from_str(&env, "free2")),
        Err(Ok(NamesError::NameNotFound))
    );
}

/// 3. Atomicity: if one name has invalid meta, the entire operation reverts.
#[test]
fn test_bulk_register_atomic_revert_on_invalid_meta() {
    let (env, client) = setup();
    let owner = Address::generate(&env);

    let names = Vec::from_array(
        &env,
        [
            String::from_str(&env, "good1"),
            String::from_str(&env, "good2"),
        ],
    );
    let metas = Vec::from_array(
        &env,
        [
            Bytes::from_slice(&env, &[1u8; 64]),
            Bytes::from_slice(&env, &[2u8; 63]), // invalid length
        ],
    );

    let result = client.try_bulk_register(&owner, &names, &metas);
    assert_eq!(result, Err(Ok(NamesError::InvalidMetaAddress)));

    assert_eq!(
        client.try_resolve(&String::from_str(&env, "good1")),
        Err(Ok(NamesError::NameNotFound))
    );
}

/// 4. Size cap: bulk_register with more than 20 names is rejected.
#[test]
fn test_bulk_register_exceeds_limit() {
    let (env, client) = setup();
    let owner = Address::generate(&env);

    let mut names = Vec::new(&env);
    let mut metas = Vec::new(&env);
    for i in 0..21 {
        names.push_back(String::from_str(&env, &format!("n{}", i)));
        metas.push_back(Bytes::from_slice(&env, &[i as u8; 64]));
    }

    let result = client.try_bulk_register(&owner, &names, &metas);
    assert_eq!(result, Err(Ok(NamesError::BulkLimitExceeded)));
}

/// 5. Mismatched input lengths are caught.
#[test]
fn test_bulk_register_mismatched_lengths() {
    let (env, client) = setup();
    let owner = Address::generate(&env);

    let names = Vec::from_array(
        &env,
        [String::from_str(&env, "a"), String::from_str(&env, "b")],
    );
    let metas = Vec::from_array(&env, [Bytes::from_slice(&env, &[1u8; 64])]);

    let result = client.try_bulk_register(&owner, &names, &metas);
    assert_eq!(result, Err(Ok(NamesError::InvalidMetaAddress)));
}

/// 6. Invalid name character causes revert.
#[test]
fn test_bulk_register_invalid_name_reverts() {
    let (env, client) = setup();
    let owner = Address::generate(&env);

    let names = Vec::from_array(
        &env,
        [
            String::from_str(&env, "valid"),
            String::from_str(&env, "BAD"),
        ],
    );
    let metas = Vec::from_array(
        &env,
        [
            Bytes::from_slice(&env, &[1u8; 64]),
            Bytes::from_slice(&env, &[2u8; 64]),
        ],
    );

    let result = client.try_bulk_register(&owner, &names, &metas);
    assert_eq!(result, Err(Ok(NamesError::InvalidNameCharacter)));

    assert_eq!(
        client.try_resolve(&String::from_str(&env, "valid")),
        Err(Ok(NamesError::NameNotFound))
    );
}

/// 7. Per-name events are emitted for each registration in a bulk_register.
#[test]
fn test_bulk_register_emits_per_name_events() {
    let (env, client) = setup();
    let owner = Address::generate(&env);

    let names = Vec::from_array(
        &env,
        [
            String::from_str(&env, "app"),
            String::from_str(&env, "docs"),
        ],
    );
    let metas = Vec::from_array(
        &env,
        [
            Bytes::from_slice(&env, &[1u8; 64]),
            Bytes::from_slice(&env, &[2u8; 64]),
        ],
    );

    client.bulk_register(&owner, &names, &metas);

    // Should have 2 per-name register events + 2 per-name register metrics
    // + 1 BulkRegistered event
    let events = env.events().all();
    // Each event has 3 components: topics, data
    // We check total count
    assert_eq!(
        events.len(),
        5,
        "expected 2 register + 2 register metrics + 1 bulk_reg events"
    );
}

/// 8. Happy path: bulk_renew extends TTL for multiple names.
#[test]
fn test_bulk_renew_all_succeed() {
    let (env, client) = setup();
    let owner = Address::generate(&env);

    client.register(
        &owner,
        &String::from_str(&env, "alpha"),
        &Bytes::from_slice(&env, &[1u8; 64]),
    );
    client.register(
        &owner,
        &String::from_str(&env, "beta"),
        &Bytes::from_slice(&env, &[2u8; 64]),
    );

    let names = Vec::from_array(
        &env,
        [
            String::from_str(&env, "alpha"),
            String::from_str(&env, "beta"),
        ],
    );
    let extend_to = env.ledger().sequence() + 10000;

    let result = client.try_bulk_renew(&names, &extend_to);
    assert_eq!(result, Ok(Ok(())));

    // Names still resolve after renew
    assert_eq!(
        client.resolve(&String::from_str(&env, "alpha")),
        Bytes::from_slice(&env, &[1u8; 64])
    );
}

/// 9. Atomicity: bulk_renew reverts if any name does not exist.
#[test]
fn test_bulk_renew_atomic_revert_on_missing() {
    let (env, client) = setup();
    let owner = Address::generate(&env);

    client.register(
        &owner,
        &String::from_str(&env, "exists"),
        &Bytes::from_slice(&env, &[1u8; 64]),
    );

    let names = Vec::from_array(
        &env,
        [
            String::from_str(&env, "exists"),
            String::from_str(&env, "ghost"),
        ],
    );
    let extend_to = env.ledger().sequence() + 10000;

    let result = client.try_bulk_renew(&names, &extend_to);
    assert_eq!(result, Err(Ok(NamesError::NameNotFound)));
}

/// 10. Size cap: bulk_renew with more than 20 names is rejected.
#[test]
fn test_bulk_renew_exceeds_limit() {
    let (env, client) = setup();

    let mut names = Vec::new(&env);
    for i in 0..21 {
        names.push_back(String::from_str(&env, &format!("n{}", i)));
    }
    let extend_to = env.ledger().sequence() + 10000;

    let result = client.try_bulk_renew(&names, &extend_to);
    assert_eq!(result, Err(Ok(NamesError::BulkLimitExceeded)));
}

/// 11. bulk_renew with invalid extend_to_ledger (past) is rejected.
#[test]
fn test_bulk_renew_invalid_extend_ledger() {
    let (env, client) = setup();
    let owner = Address::generate(&env);

    client.register(
        &owner,
        &String::from_str(&env, "test"),
        &Bytes::from_slice(&env, &[1u8; 64]),
    );

    let names = Vec::from_array(&env, [String::from_str(&env, "test")]);
    let current = env.ledger().sequence();
    let result = client.try_bulk_renew(&names, &current);
    assert_eq!(result, Err(Ok(NamesError::InvalidExtendLedger)));
}

/// 12. Per-name extend events are emitted in bulk_renew.
#[test]
fn test_bulk_renew_emits_per_name_events() {
    let (env, client) = setup();
    let owner = Address::generate(&env);

    client.register(
        &owner,
        &String::from_str(&env, "alpha"),
        &Bytes::from_slice(&env, &[1u8; 64]),
    );
    client.register(
        &owner,
        &String::from_str(&env, "beta"),
        &Bytes::from_slice(&env, &[2u8; 64]),
    );

    let names = Vec::from_array(
        &env,
        [
            String::from_str(&env, "alpha"),
            String::from_str(&env, "beta"),
        ],
    );
    let extend_to = env.ledger().sequence() + 10000;
    client.bulk_renew(&names, &extend_to);

    let events = env.events().all();
    // 2 per-name extend events + 1 bulk_renew event + 1 batch renew metric
    assert_eq!(
        events.len(),
        4,
        "expected 2 extend + 1 blk_renew + 1 renew metric events"
    );
}
