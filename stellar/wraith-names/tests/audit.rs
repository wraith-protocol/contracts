use ed25519_dalek::SigningKey;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::xdr::{AccountId, PublicKey, ScAddress, Uint256};
use soroban_sdk::{Address, Bytes, BytesN, Env, String, TryFromVal};

use wraith_names::{NamesError, WraithNamesContract, WraithNamesContractClient};

fn signing_account(env: &Env, seed: [u8; 32]) -> (Address, SigningKey) {
    let signing_key = SigningKey::from_bytes(&seed);
    let public_key = Uint256::try_from(signing_key.verifying_key().to_bytes().as_ref())
        .expect("valid ed25519 key");
    let sc_address = ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(public_key)));
    let owner = Address::try_from_val(env, &sc_address).expect("account address");
    (owner, signing_key)
}

fn sign_authorization(
    env: &Env,
    signing_key: &SigningKey,
    operation: &[u8],
    name: &String,
    stealth_meta_address: &Bytes,
    expiry: u64,
) -> BytesN<64> {
    use ed25519_dalek::Signer;

    // Replicate authorization_message logic (private in contract)
    let mut message = Bytes::from_slice(env, b"wraith-names:v1");
    message.extend_from_slice(operation);
    let name_len = name.len() as usize;
    let mut name_buf = [0u8; 32];
    name.copy_into_slice(&mut name_buf[..name_len]);
    let name_bytes = Bytes::from_slice(env, &name_buf[..name_len]);
    message.append(&name_bytes);
    message.append(stealth_meta_address);
    message.extend_from_slice(&expiry.to_be_bytes());

    let message_hash = env.crypto().sha256(&message);
    let message_bytes = message_hash.to_array();
    let signature = signing_key.sign(&message_bytes);
    BytesN::from_array(env, &signature.to_bytes())
}

// ---------------------------------------------------------------------------
// Ownership & Authorization Tests
// ---------------------------------------------------------------------------

/// 1. Non-owner cannot update a registered name.
#[test]
fn adversarial_update_by_non_owner_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let attacker = Address::generate(&env);
    let name = String::from_str(&env, "alice");
    let meta = Bytes::from_slice(&env, &[42u8; 64]);
    let new_meta = Bytes::from_slice(&env, &[99u8; 64]);

    client.register(&owner, &name, &meta);

    // Attacker tries to update
    let result = client.try_update(&attacker, &name, &new_meta);
    assert_eq!(result, Err(Ok(NamesError::NotOwner)));
}

/// 2. Non-owner cannot release a registered name.
#[test]
fn adversarial_release_by_non_owner_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let attacker = Address::generate(&env);
    let name = String::from_str(&env, "bob");
    let meta = Bytes::from_slice(&env, &[1u8; 64]);

    client.register(&owner, &name, &meta);

    let result = client.try_release(&attacker, &name);
    assert_eq!(result, Err(Ok(NamesError::NotOwner)));
}

/// 3. Owner cannot update a name that does not exist.
#[test]
fn adversarial_update_nonexistent_name_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let name = String::from_str(&env, "ghost");
    let meta = Bytes::from_slice(&env, &[5u8; 64]);

    let result = client.try_update(&owner, &name, &meta);
    assert_eq!(result, Err(Ok(NamesError::NameNotFound)));
}

/// 4. Owner cannot release a name that does not exist.
#[test]
fn adversarial_release_nonexistent_name_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let name = String::from_str(&env, "ghost");

    let result = client.try_release(&owner, &name);
    assert_eq!(result, Err(Ok(NamesError::NameNotFound)));
}

// ---------------------------------------------------------------------------
// Name Validation Edge Cases
// ---------------------------------------------------------------------------

/// 5. Single character name rejected.
#[test]
fn adversarial_name_single_char_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let meta = Bytes::from_slice(&env, &[1u8; 64]);

    let result = client.try_register(&owner, &String::from_str(&env, "a"), &meta);
    assert_eq!(result, Err(Ok(NamesError::NameTooShort)));
}

/// 6. Empty name rejected.
#[test]
fn adversarial_name_empty_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let meta = Bytes::from_slice(&env, &[1u8; 64]);

    let result = client.try_register(&owner, &String::from_str(&env, ""), &meta);
    assert_eq!(result, Err(Ok(NamesError::NameTooShort)));
}

/// 7. Uppercase character rejected.
#[test]
fn adversarial_name_uppercase_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let meta = Bytes::from_slice(&env, &[1u8; 64]);

    let result = client.try_register(&owner, &String::from_str(&env, "Alice"), &meta);
    assert_eq!(result, Err(Ok(NamesError::InvalidNameCharacter)));
}

/// 8. Underscore character rejected.
#[test]
fn adversarial_name_underscore_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let meta = Bytes::from_slice(&env, &[1u8; 64]);

    let result = client.try_register(&owner, &String::from_str(&env, "user_name"), &meta);
    assert_eq!(result, Err(Ok(NamesError::InvalidNameCharacter)));
}

/// 9. Hyphen character rejected.
#[test]
fn adversarial_name_hyphen_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let meta = Bytes::from_slice(&env, &[1u8; 64]);

    let result = client.try_register(&owner, &String::from_str(&env, "user-name"), &meta);
    assert_eq!(result, Err(Ok(NamesError::InvalidNameCharacter)));
}

/// 10. Space character rejected.
#[test]
fn adversarial_name_space_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let meta = Bytes::from_slice(&env, &[1u8; 64]);

    let result = client.try_register(&owner, &String::from_str(&env, "user name"), &meta);
    assert_eq!(result, Err(Ok(NamesError::InvalidNameCharacter)));
}

/// 11. 33-character name (over limit) rejected.
#[test]
fn adversarial_name_33_chars_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let meta = Bytes::from_slice(&env, &[1u8; 64]);

    let result =
        client.try_register(&owner, &String::from_str(&env, "abcdefghijklmnopqrstuvwxyzaaaaaaa"), &meta);
    assert_eq!(result, Err(Ok(NamesError::NameTooLong)));
}

/// 12. 32-character name (max valid) accepted.
#[test]
fn adversarial_name_32_chars_accepted() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let meta = Bytes::from_slice(&env, &[1u8; 64]);

    client.register(&owner, &String::from_str(&env, "abcdefghijklmnopqrstuvwxyzaaaa"), &meta);
    let resolved = client.resolve(&String::from_str(&env, "abcdefghijklmnopqrstuvwxyzaaaa"));
    assert_eq!(resolved, meta);
}

/// 13. Numbers-only name accepted.
#[test]
fn adversarial_name_numbers_only_accepted() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let meta = Bytes::from_slice(&env, &[1u8; 64]);

    client.register(&owner, &String::from_str(&env, "123"), &meta);
    let resolved = client.resolve(&String::from_str(&env, "123"));
    assert_eq!(resolved, meta);
}

/// 14. Mixed alphanumeric accepted.
#[test]
fn adversarial_name_mixed_alphanumeric_accepted() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let meta = Bytes::from_slice(&env, &[1u8; 64]);

    client.register(&owner, &String::from_str(&env, "abc123xyz"), &meta);
    let resolved = client.resolve(&String::from_str(&env, "abc123xyz"));
    assert_eq!(resolved, meta);
}

/// 15. Name starting with digit accepted.
#[test]
fn adversarial_name_starts_with_digit_accepted() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let meta = Bytes::from_slice(&env, &[1u8; 64]);

    client.register(&owner, &String::from_str(&env, "9user"), &meta);
    let resolved = client.resolve(&String::from_str(&env, "9user"));
    assert_eq!(resolved, meta);
}

// ---------------------------------------------------------------------------
// Release & Re-Register Flows
// ---------------------------------------------------------------------------

/// 16. Name can be immediately re-registered by a different user after release.
#[test]
fn adversarial_release_then_immediate_reregister() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner1 = Address::generate(&env);
    let owner2 = Address::generate(&env);
    let name = String::from_str(&env, "hotname");
    let meta1 = Bytes::from_slice(&env, &[1u8; 64]);
    let meta2 = Bytes::from_slice(&env, &[2u8; 64]);

    client.register(&owner1, &name, &meta1);
    assert_eq!(client.resolve(&name), meta1);

    client.release(&owner1, &name);

    // Verify name is gone
    let result = client.try_resolve(&name);
    assert_eq!(result, Err(Ok(NamesError::NameNotFound)));

    // Verify reverse lookup is gone
    let result = client.try_name_of(&meta1);
    assert_eq!(result, Err(Ok(NamesError::NameNotFound)));

    // Different user registers immediately
    client.register(&owner2, &name, &meta2);
    assert_eq!(client.resolve(&name), meta2);
    assert_eq!(client.name_of(&meta2), name);
}

/// 17. Released name's old meta-address no longer resolves via reverse lookup.
#[test]
fn adversarial_reverse_lookup_cleared_on_release() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let name = String::from_str(&env, "temp");
    let meta = Bytes::from_slice(&env, &[55u8; 64]);

    client.register(&owner, &name, &meta);
    assert_eq!(client.name_of(&meta), name);

    client.release(&owner, &name);

    let result = client.try_name_of(&meta);
    assert_eq!(result, Err(Ok(NamesError::NameNotFound)));
}

// ---------------------------------------------------------------------------
// Reverse Lookup Integrity
// ---------------------------------------------------------------------------

/// 18. Update correctly swaps reverse lookup from old to new meta-address.
#[test]
fn adversarial_reverse_lookup_updated_correctly() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let name = String::from_str(&env, "updatee");
    let old_meta = Bytes::from_slice(&env, &[10u8; 64]);
    let new_meta = Bytes::from_slice(&env, &[20u8; 64]);

    client.register(&owner, &name, &old_meta);
    assert_eq!(client.name_of(&old_meta), name);

    // Verify new_meta has no name yet
    let result = client.try_name_of(&new_meta);
    assert_eq!(result, Err(Ok(NamesError::NameNotFound)));

    client.update(&owner, &name, &new_meta);

    // Old meta should no longer resolve
    let result = client.try_name_of(&old_meta);
    assert_eq!(result, Err(Ok(NamesError::NameNotFound)));

    // New meta should resolve to the name
    assert_eq!(client.name_of(&new_meta), name);
}

/// 19. Two names with different meta-addresses maintain independent reverse lookups.
#[test]
fn adversarial_multiple_names_independent_reverse() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner1 = Address::generate(&env);
    let owner2 = Address::generate(&env);
    let name1 = String::from_str(&env, "alpha");
    let name2 = String::from_str(&env, "beta");
    let meta1 = Bytes::from_slice(&env, &[1u8; 64]);
    let meta2 = Bytes::from_slice(&env, &[2u8; 64]);

    client.register(&owner1, &name1, &meta1);
    client.register(&owner2, &name2, &meta2);

    assert_eq!(client.name_of(&meta1), name1);
    assert_eq!(client.name_of(&meta2), name2);

    // Release one, check the other is unaffected
    client.release(&owner1, &name1);
    assert_eq!(client.name_of(&meta2), name2);

    let result = client.try_name_of(&meta1);
    assert_eq!(result, Err(Ok(NamesError::NameNotFound)));
}

/// 20. Non-existent meta-address returns NameNotFound (not empty string).
#[test]
fn adversarial_name_of_nonexistent_returns_error() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let nonexistent_meta = Bytes::from_slice(&env, &[255u8; 64]);
    let result = client.try_name_of(&nonexistent_meta);
    assert_eq!(result, Err(Ok(NamesError::NameNotFound)));
}

// ---------------------------------------------------------------------------
// Meta-Address Validation
// ---------------------------------------------------------------------------

/// 21. Meta-address of 63 bytes rejected on register.
#[test]
fn adversarial_meta_address_63_bytes_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let name = String::from_str(&env, "valid");
    let bad_meta = Bytes::from_slice(&env, &[1u8; 63]);

    let result = client.try_register(&owner, &name, &bad_meta);
    assert_eq!(result, Err(Ok(NamesError::InvalidMetaAddress)));
}

/// 22. Meta-address of 65 bytes rejected on register.
#[test]
fn adversarial_meta_address_65_bytes_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let name = String::from_str(&env, "valid");
    let bad_meta = Bytes::from_slice(&env, &[1u8; 65]);

    let result = client.try_register(&owner, &name, &bad_meta);
    assert_eq!(result, Err(Ok(NamesError::InvalidMetaAddress)));
}

/// 23. Meta-address of 0 bytes rejected on register.
#[test]
fn adversarial_meta_address_empty_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let name = String::from_str(&env, "valid");
    let bad_meta = Bytes::new(&env);

    let result = client.try_register(&owner, &name, &bad_meta);
    assert_eq!(result, Err(Ok(NamesError::InvalidMetaAddress)));
}

/// 24. Invalid meta-address (63 bytes) rejected on update.
#[test]
fn adversarial_update_invalid_meta_address_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let name = String::from_str(&env, "testname");
    let valid_meta = Bytes::from_slice(&env, &[1u8; 64]);
    let bad_meta = Bytes::from_slice(&env, &[1u8; 63]);

    client.register(&owner, &name, &valid_meta);

    let result = client.try_update(&owner, &name, &bad_meta);
    assert_eq!(result, Err(Ok(NamesError::InvalidMetaAddress)));

    // Verify original meta is unchanged
    assert_eq!(client.resolve(&name), valid_meta);
}

// ---------------------------------------------------------------------------
// On-Behalf Authorization Edge Cases
// ---------------------------------------------------------------------------

/// 25. On-behalf register with wrong signer rejected.
#[test]
fn adversarial_on_behalf_wrong_signer_rejected() {
    let env = Env::default();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let (owner, _) = signing_account(&env, [11u8; 32]);
    let (_, wrong_signing_key) = signing_account(&env, [22u8; 32]);
    let name = String::from_str(&env, "mallory");
    let meta = Bytes::from_slice(&env, &[5u8; 64]);
    let expiry = u64::from(env.ledger().sequence()) + 10;
    let signature = sign_authorization(
        &env,
        &wrong_signing_key,
        b"wraith-names:register",
        &name,
        &meta,
        expiry,
    );

    let result = client.try_register_on_behalf(&owner, &name, &meta, &signature, &expiry);
    assert!(result.is_err());
}

/// 26. Expired on-behalf signature rejected.
#[test]
fn adversarial_on_behalf_expired_signature_rejected() {
    let env = Env::default();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let (owner, signing_key) = signing_account(&env, [33u8; 32]);
    let name = String::from_str(&env, "trent");
    let meta = Bytes::from_slice(&env, &[8u8; 64]);
    let expiry = u64::from(env.ledger().sequence());
    let signature = sign_authorization(
        &env,
        &signing_key,
        b"wraith-names:register",
        &name,
        &meta,
        expiry,
    );

    let result = client.try_register_on_behalf(&owner, &name, &meta, &signature, &expiry);
    assert_eq!(result, Err(Ok(NamesError::SignatureExpired)));
}

/// 27. Replay of on-behalf signature rejected.
#[test]
fn adversarial_on_behalf_replay_rejected() {
    let env = Env::default();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let (owner, signing_key) = signing_account(&env, [44u8; 32]);
    let name = String::from_str(&env, "victor");
    let meta = Bytes::from_slice(&env, &[9u8; 64]);
    let expiry = u64::from(env.ledger().sequence()) + 10;
    let signature = sign_authorization(
        &env,
        &signing_key,
        b"wraith-names:register",
        &name,
        &meta,
        expiry,
    );

    client.register_on_behalf(&owner, &name, &meta, &signature, &expiry);
    let result = client.try_register_on_behalf(&owner, &name, &meta, &signature, &expiry);
    assert_eq!(result, Err(Ok(NamesError::SignatureReplay)));
}

/// 28. On-behalf update with non-existent name rejected.
#[test]
fn adversarial_on_behalf_update_nonexistent_rejected() {
    let env = Env::default();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let (owner, signing_key) = signing_account(&env, [77u8; 32]);
    let name = String::from_str(&env, "ghost");
    let meta = Bytes::from_slice(&env, &[9u8; 64]);
    let expiry = u64::from(env.ledger().sequence()) + 10;
    let signature = sign_authorization(
        &env,
        &signing_key,
        b"wraith-names:update",
        &name,
        &meta,
        expiry,
    );

    let result = client.try_update_on_behalf(&owner, &name, &meta, &signature, &expiry);
    assert_eq!(result, Err(Ok(NamesError::NameNotFound)));
}

/// 29. On-behalf release with non-existent name rejected.
#[test]
fn adversarial_on_behalf_release_nonexistent_rejected() {
    let env = Env::default();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let (owner, signing_key) = signing_account(&env, [88u8; 32]);
    let name = String::from_str(&env, "ghost");
    let expiry = u64::from(env.ledger().sequence()) + 10;
    let signature = sign_authorization(
        &env,
        &signing_key,
        b"wraith-names:release",
        &name,
        &Bytes::new(&env),
        expiry,
    );

    let result = client.try_release_on_behalf(&owner, &name, &signature, &expiry);
    assert_eq!(result, Err(Ok(NamesError::NameNotFound)));
}

/// 30. On-behalf operations with distinct replay keys — same name, different operations don't conflict.
#[test]
fn adversarial_on_behalf_different_operations_distinct_replay_keys() {
    let env = Env::default();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let (owner, signing_key) = signing_account(&env, [99u8; 32]);
    let name = String::from_str(&env, "multi");
    let meta1 = Bytes::from_slice(&env, &[10u8; 64]);
    let meta2 = Bytes::from_slice(&env, &[20u8; 64]);
    let expiry = u64::from(env.ledger().sequence()) + 10;

    // Register
    let reg_sig = sign_authorization(
        &env,
        &signing_key,
        b"wraith-names:register",
        &name,
        &meta1,
        expiry,
    );
    client.register_on_behalf(&owner, &name, &meta1, &reg_sig, &expiry);
    assert_eq!(client.resolve(&name), meta1);

    // Update — should succeed (different operation = different replay key)
    let upd_sig = sign_authorization(
        &env,
        &signing_key,
        b"wraith-names:update",
        &name,
        &meta2,
        expiry,
    );
    client.update_on_behalf(&owner, &name, &meta2, &upd_sig, &expiry);
    assert_eq!(client.resolve(&name), meta2);

    // Release — should succeed (different operation)
    let rel_sig = sign_authorization(
        &env,
        &signing_key,
        b"wraith-names:release",
        &name,
        &Bytes::new(&env),
        expiry,
    );
    client.release_on_behalf(&owner, &name, &rel_sig, &expiry);
    let result = client.try_resolve(&name);
    assert_eq!(result, Err(Ok(NamesError::NameNotFound)));
}
