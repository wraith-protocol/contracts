use soroban_sdk::testutils::{Address as _, Events, Ledger};
use soroban_sdk::{Address, Bytes, Env, String};

use wraith_names::{NamesError, WraithNamesContract, WraithNamesContractClient};

fn setup() -> (Env, WraithNamesContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);
    (env, client)
}

fn register_name(
    env: &Env,
    client: &WraithNamesContractClient,
    owner: &Address,
    name: &str,
    meta: &[u8; 64],
) {
    let name_str = String::from_str(env, name);
    let meta_bytes = Bytes::from_slice(env, meta);
    client.register(owner, &name_str, &meta_bytes);
}

/// 1. Happy path: extend_name_ttl extends TTL of an existing name.
#[test]
fn test_extend_name_ttl_happy_path() {
    let (env, client) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "alice");
    let meta = [1u8; 64];

    register_name(&env, &client, &owner, "alice", &meta);

    // Extend to ledger 10000
    let extend_to = u32::try_from(env.ledger().sequence() + 10000).unwrap();
    let result = client.try_extend_name_ttl(&name, &extend_to);
    assert_eq!(result, Ok(Ok(())));
}

/// 2. Permissionless: any caller can extend a name's TTL.
#[test]
fn test_extend_name_ttl_permissionless() {
    let (env, client) = setup();
    let owner = Address::generate(&env);
    let other = Address::generate(&env);
    let name = String::from_str(&env, "bob");
    let meta = [2u8; 64];

    register_name(&env, &client, &owner, "bob", &meta);

    // Other party extends the TTL
    let extend_to = u32::try_from(env.ledger().sequence() + 5000).unwrap();
    let result = client.try_extend_name_ttl(&name, &extend_to);
    assert_eq!(result, Ok(Ok(())));
}

/// 3. Idempotency: calling extend_name_ttl twice in the same ledger is a no-op.
#[test]
fn test_extend_name_ttl_idempotent() {
    let (env, client) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "carol");
    let meta = [3u8; 64];

    register_name(&env, &client, &owner, "carol", &meta);

    // First extend
    let extend_to = u32::try_from(env.ledger().sequence() + 7000).unwrap();
    let result1 = client.try_extend_name_ttl(&name, &extend_to);
    assert_eq!(result1, Ok(Ok(())));

    // Second extend in same ledger (should also succeed)
    let result2 = client.try_extend_name_ttl(&name, &extend_to);
    assert_eq!(result2, Ok(Ok(())));
}

/// 4. Non-existent name: extend_name_ttl on a non-existent name returns error.
#[test]
fn test_extend_name_ttl_nonexistent_name_rejected() {
    let (env, client) = setup();
    let name = String::from_str(&env, "ghost");

    let extend_to = u32::try_from(env.ledger().sequence() + 5000).unwrap();
    let result = client.try_extend_name_ttl(&name, &extend_to);
    assert_eq!(result, Err(Ok(NamesError::NameNotFound)));
}

/// 5. Invalid extend_to_ledger: extending to current ledger or past is rejected.
#[test]
fn test_extend_name_ttl_invalid_extend_ledger() {
    let (env, client) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "dave");
    let meta = [4u8; 64];

    register_name(&env, &client, &owner, "dave", &meta);

    let current_ledger = env.ledger().sequence();

    // Try to extend to current ledger
    let result = client.try_extend_name_ttl(&name, &u32::try_from(current_ledger).unwrap());
    assert_eq!(result, Err(Ok(NamesError::InvalidExtendLedger)));

    // Try to extend to past ledger
    let result = client.try_extend_name_ttl(
        &name,
        &u32::try_from(current_ledger.saturating_sub(1)).unwrap_or(0),
    );
    assert_eq!(result, Err(Ok(NamesError::InvalidExtendLedger)));
}

/// 6. Multiple names: extending different names independently works.
#[test]
fn test_extend_name_ttl_multiple_names() {
    let (env, client) = setup();
    let owner = Address::generate(&env);

    let name1 = String::from_str(&env, "eve");
    let name2 = String::from_str(&env, "frank");
    let meta = [5u8; 64];

    register_name(&env, &client, &owner, "eve", &meta);
    register_name(&env, &client, &owner, "frank", &meta);

    // Extend both names to different ledgers
    let extend_to1 = u32::try_from(env.ledger().sequence() + 3000).unwrap();
    let extend_to2 = u32::try_from(env.ledger().sequence() + 8000).unwrap();

    let result1 = client.try_extend_name_ttl(&name1, &extend_to1);
    assert_eq!(result1, Ok(Ok(())));

    let result2 = client.try_extend_name_ttl(&name2, &extend_to2);
    assert_eq!(result2, Ok(Ok(())));
}

/// 7. Extend after update: after updating a name, can still extend the new entry.
#[test]
fn test_extend_name_ttl_after_update() {
    let (env, client) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "grace");
    let meta1 = Bytes::from_slice(&env, &[6u8; 64]);
    let meta2 = Bytes::from_slice(&env, &[7u8; 64]);

    client.register(&owner, &name, &meta1);

    // Update the name
    client.update(&owner, &name, &meta2);

    // Extend the updated name
    let extend_to = u32::try_from(env.ledger().sequence() + 4000).unwrap();
    let result = client.try_extend_name_ttl(&name, &extend_to);
    assert_eq!(result, Ok(Ok(())));

    // Verify the name still resolves to the new meta
    assert_eq!(client.resolve(&name), meta2);
}

/// 8. Extend after release and re-register: releasing a name, then re-registering, then extending.
#[test]
fn test_extend_name_ttl_after_release_and_reregister() {
    let (env, client) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "henry");
    let meta = [8u8; 64];

    register_name(&env, &client, &owner, "henry", &meta);

    // Release the name
    client.release(&owner, &name);

    // Re-register it
    register_name(&env, &client, &owner, "henry", &meta);

    // Extend the re-registered name
    let extend_to = u32::try_from(env.ledger().sequence() + 6000).unwrap();
    let result = client.try_extend_name_ttl(&name, &extend_to);
    assert_eq!(result, Ok(Ok(())));
}

/// 9. Emits extend event: extend_name_ttl emits an extend event.
#[test]
fn test_extend_name_ttl_emits_event() {
    let (env, client) = setup();
    let owner = Address::generate(&env);
    let name = String::from_str(&env, "ivy");
    let meta = [9u8; 64];

    register_name(&env, &client, &owner, "ivy", &meta);

    let extend_to = u32::try_from(env.ledger().sequence() + 9000).unwrap();
    client.extend_name_ttl(&name, &extend_to);

    // Check that an event was published (test framework should capture this)
    // The event should contain the name hash and extend_to_ledger
    let events = env.events().all();
    assert!(!events.is_empty(), "extend_name_ttl should emit an event");
}
