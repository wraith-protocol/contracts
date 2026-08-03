//! Chaos integration tests for all Stellar Wraith contracts.
//!
//! These tests exercise the full contract lifecycle through the chaos harness.
//! When `WRAITHCHAOS_MODE=1` is set, every wrapped RPC-equivalent operation
//! has a configurable probability of injecting a simulated failure (HTTP 500,
//! timeout, wrong ledger, or empty response).
//!
//! Run in happy-path mode (default):
//!   cargo test --package integration-tests
//!
//! Run in chaos mode:
//!   WRAITHCHAOS_MODE=1 cargo test --package integration-tests
//!
//! Every wrapped op documents its retry / bail policy in `harness.rs`.

mod harness;

use harness::ChaosClient;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Bytes, BytesN, Env, String as SorobanString};

use stealth_announcer::{
    StealthAnnouncerContract, StealthAnnouncerContractClient, STELLAR_V2_SCHEME_ID,
};
use stealth_registry::{StealthRegistryContract, StealthRegistryContractClient};
use stealth_sender::{StealthSenderContract, StealthSenderContractClient};
use wraith_names::{WraithNamesContract, WraithNamesContractClient};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn env() -> Env {
    Env::default()
}

fn bytes(env: &Env, data: &[u8]) -> Bytes {
    Bytes::from_slice(env, data)
}

fn bytes32(env: &Env, data: &[u8]) -> BytesN<32> {
    let mut fixed = [0u8; 32];
    fixed.copy_from_slice(data);
    BytesN::from_array(env, &fixed)
}

/// Minimal announcer mock for sender tests.
mod mock_announcer {
    use soroban_sdk::{contract, contractimpl, symbol_short, Address, Bytes, BytesN, Env};

    #[contract]
    pub struct MockAnnouncer;

    #[contractimpl]
    impl MockAnnouncer {
        pub fn announce(
            env: Env,
            _scheme_id: u32,
            stealth_address: Address,
            _ephemeral_pub_key: BytesN<32>,
            _metadata: Bytes,
        ) {
            env.events()
                .publish((symbol_short!("announce"), stealth_address), ());
        }
    }
}

fn funded_token(env: &Env) -> (Address, Address) {
    use soroban_sdk::token::StellarAssetClient;
    let admin = Address::generate(env);
    let sender = Address::generate(env);
    let token = env.register_stellar_asset_contract_v2(admin).address();
    let asset = StellarAssetClient::new(env, &token);
    asset.mint(&sender, &1_000_000);
    (token, sender)
}

// ── Announcer tests ──────────────────────────────────────────────────────────

#[test]
fn announcer_announce_through_chaos() {
    let chaos = ChaosClient::from_env();
    let result = chaos.execute("announcer.announce", || {
        let env = env();
        let contract_id = env.register(StealthAnnouncerContract, ());
        let client = StealthAnnouncerContractClient::new(&env, &contract_id);
        let stealth_address = Address::generate(&env);
        let epk = bytes32(&env, &[1u8; 32]);
        let metadata = bytes(&env, &[42u8, 7u8]);
        client.announce(&STELLAR_V2_SCHEME_ID, &stealth_address, &epk, &metadata);
        Ok::<_, harness::ChaosError>(())
    });

    if chaos.is_chaos_enabled() {
        assert!(result.is_err(), "chaos should have injected a failure");
    } else {
        assert!(result.is_ok());
    }
}

#[test]
fn announcer_multiple_schemes_through_chaos() {
    let chaos = ChaosClient::from_env();
    let result = chaos.execute("announcer.multi", || {
        let env = env();
        let contract_id = env.register(StealthAnnouncerContract, ());
        let client = StealthAnnouncerContractClient::new(&env, &contract_id);
        let addr = Address::generate(&env);
        let epk = bytes32(&env, &[0xab; 32]);
        let meta = bytes(&env, &[0x01]);
        client.announce(&STELLAR_V2_SCHEME_ID, &addr, &epk, &meta);
        client.announce(&STELLAR_V2_SCHEME_ID, &addr, &epk, &bytes(&env, &[0x02]));
        Ok::<_, harness::ChaosError>(())
    });

    if chaos.is_chaos_enabled() {
        assert!(result.is_err());
    } else {
        assert!(result.is_ok());
    }
}

// ── Registry tests ───────────────────────────────────────────────────────────

#[test]
fn registry_register_and_lookup_through_chaos() {
    let chaos = ChaosClient::from_env();
    let result = chaos.execute("registry.register+lookup", || {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(StealthRegistryContract, ());
        let client = StealthRegistryContractClient::new(&env, &contract_id);
        let registrant = Address::generate(&env);
        let meta = bytes(&env, &[1u8; 64]);
        client.register_keys(&registrant, &1, &meta);
        let result = client.stealth_meta_address_of(&registrant, &1);
        assert_eq!(result, meta);
        Ok::<_, harness::ChaosError>(())
    });

    if chaos.is_chaos_enabled() {
        assert!(result.is_err());
    } else {
        assert!(result.is_ok());
    }
}

#[test]
fn registry_wrong_length_rejected_through_chaos() {
    let chaos = ChaosClient::from_env();
    let result = chaos.execute("registry.bad_length", || {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(StealthRegistryContract, ());
        let client = StealthRegistryContractClient::new(&env, &contract_id);
        let registrant = Address::generate(&env);
        let bad_meta = bytes(&env, &[1u8; 32]);
        let res = client.try_register_keys(&registrant, &1, &bad_meta);
        assert_eq!(
            res,
            Err(Ok(
                stealth_registry::RegistryError::InvalidMetaAddressLength
            ))
        );
        Ok::<_, harness::ChaosError>(())
    });

    if chaos.is_chaos_enabled() {
        assert!(result.is_err());
    } else {
        assert!(result.is_ok());
    }
}

// ── Sender tests ─────────────────────────────────────────────────────────────

#[test]
fn sender_send_eth_through_chaos() {
    let chaos = ChaosClient::from_env();
    let result = chaos.execute("sender.send", || {
        let env = env();
        env.mock_all_auths();
        let announcer_id = env.register(mock_announcer::MockAnnouncer, ());
        let sender_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(&env, &sender_id);
        let admin = Address::generate(&env);
        client.init(&announcer_id, &None, &None, &0, &admin);
        let (token, sender_addr) = funded_token(&env);
        let stealth = Address::generate(&env);
        let epk = bytes32(&env, &[0xab; 32]);
        let meta = bytes(&env, &[0x01]);
        client.send(&sender_addr, &token, &500, &1, &stealth, &epk, &meta);
        let token_client = soroban_sdk::token::Client::new(&env, &token);
        assert_eq!(token_client.balance(&stealth), 500);
        Ok::<_, harness::ChaosError>(())
    });

    if chaos.is_chaos_enabled() {
        assert!(result.is_err());
    } else {
        assert!(result.is_ok());
    }
}

#[test]
fn sender_batch_send_through_chaos() {
    let chaos = ChaosClient::from_env();
    let result = chaos.execute("sender.batch_send", || {
        let env = env();
        env.mock_all_auths();
        let announcer_id = env.register(mock_announcer::MockAnnouncer, ());
        let sender_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(&env, &sender_id);
        let admin = Address::generate(&env);
        client.init(&announcer_id, &None, &None, &0, &admin);
        let (token, sender_addr) = funded_token(&env);
        let stealth1 = Address::generate(&env);
        let stealth2 = Address::generate(&env);
        let epk1 = bytes32(&env, &[1u8; 32]);
        let epk2 = bytes32(&env, &[2u8; 32]);
        let meta1 = bytes(&env, &[10u8]);
        let meta2 = bytes(&env, &[20u8]);
        let addresses = soroban_sdk::vec![&env, stealth1.clone(), stealth2.clone()];
        let epks = soroban_sdk::vec![&env, epk1, epk2];
        let metadatas = soroban_sdk::vec![&env, meta1, meta2];
        let amounts = soroban_sdk::vec![&env, 300, 400];
        client.batch_send(
            &sender_addr,
            &token,
            &1,
            &addresses,
            &epks,
            &metadatas,
            &amounts,
        );
        let token_client = soroban_sdk::token::Client::new(&env, &token);
        assert_eq!(token_client.balance(&stealth1), 300);
        assert_eq!(token_client.balance(&stealth2), 400);
        Ok::<_, harness::ChaosError>(())
    });

    if chaos.is_chaos_enabled() {
        assert!(result.is_err());
    } else {
        assert!(result.is_ok());
    }
}

// ── Names tests ──────────────────────────────────────────────────────────────

#[test]
fn names_register_and_resolve_through_chaos() {
    let chaos = ChaosClient::from_env();
    let result = chaos.execute("names.register+resolve", || {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let name = SorobanString::from_str(&env, "alice");
        let meta = bytes(&env, &[1u8; 64]);
        client.register(&owner, &name, &meta);
        let resolved = client.resolve(&name);
        assert_eq!(resolved, meta);
        Ok::<_, harness::ChaosError>(())
    });

    if chaos.is_chaos_enabled() {
        assert!(result.is_err());
    } else {
        assert!(result.is_ok());
    }
}

#[test]
fn names_update_through_chaos() {
    let chaos = ChaosClient::from_env();
    let result = chaos.execute("names.update", || {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let name = SorobanString::from_str(&env, "bob");
        let meta1 = bytes(&env, &[1u8; 64]);
        let meta2 = bytes(&env, &[2u8; 64]);
        client.register(&owner, &name, &meta1);
        assert_eq!(client.resolve(&name), meta1);
        client.update(&owner, &name, &meta2);
        assert_eq!(client.resolve(&name), meta2);
        Ok::<_, harness::ChaosError>(())
    });

    if chaos.is_chaos_enabled() {
        assert!(result.is_err());
    } else {
        assert!(result.is_ok());
    }
}

#[test]
fn names_release_and_reregister_through_chaos() {
    let chaos = ChaosClient::from_env();
    let result = chaos.execute("names.release+reregister", || {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let name = SorobanString::from_str(&env, "carol");
        let meta = bytes(&env, &[1u8; 64]);
        client.register(&owner, &name, &meta);
        client.release(&owner, &name);
        let res = client.try_resolve(&name);
        assert_eq!(res, Err(Ok(wraith_names::NamesError::NameNotFound)));
        let meta2 = bytes(&env, &[3u8; 64]);
        client.register(&owner, &name, &meta2);
        assert_eq!(client.resolve(&name), meta2);
        Ok::<_, harness::ChaosError>(())
    });

    if chaos.is_chaos_enabled() {
        assert!(result.is_err());
    } else {
        assert!(result.is_ok());
    }
}

#[test]
fn names_duplicate_rejected_through_chaos() {
    let chaos = ChaosClient::from_env();
    let result = chaos.execute("names.duplicate", || {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let name = SorobanString::from_str(&env, "dave");
        let meta = bytes(&env, &[1u8; 64]);
        client.register(&owner, &name, &meta);
        let res = client.try_register(&owner, &name, &meta);
        assert_eq!(res, Err(Ok(wraith_names::NamesError::NameTaken)));
        Ok::<_, harness::ChaosError>(())
    });

    if chaos.is_chaos_enabled() {
        assert!(result.is_err());
    } else {
        assert!(result.is_ok());
    }
}

#[test]
fn names_reverse_lookup_through_chaos() {
    let chaos = ChaosClient::from_env();
    let result = chaos.execute("names.name_of", || {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let name = SorobanString::from_str(&env, "eve");
        let meta = bytes(&env, &[42u8; 64]);
        client.register(&owner, &name, &meta);
        let found = client.name_of(&meta);
        assert_eq!(found, name);
        Ok::<_, harness::ChaosError>(())
    });

    if chaos.is_chaos_enabled() {
        assert!(result.is_err());
    } else {
        assert!(result.is_ok());
    }
}

// ── Cross-contract sender + announcer through chaos ──────────────────────────

#[test]
fn sender_announcer_lifecycle_through_chaos() {
    let chaos = ChaosClient::from_env();
    let result = chaos.execute("sender.announcer.lifecycle", || {
        let env = env();
        env.mock_all_auths();
        let announcer_id = env.register(StealthAnnouncerContract, ());
        let sender_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(&env, &sender_id);
        let admin = Address::generate(&env);
        client.init(&announcer_id, &None, &None, &0, &admin);
        let (token, sender_addr) = funded_token(&env);
        let stealth = Address::generate(&env);
        let epk = bytes32(&env, &[0xff; 32]);
        let meta = bytes(&env, &[0x07]);
        client.send(
            &sender_addr,
            &token,
            &1000,
            &STELLAR_V2_SCHEME_ID,
            &stealth,
            &epk,
            &meta,
        );
        let token_client = soroban_sdk::token::Client::new(&env, &token);
        assert_eq!(token_client.balance(&stealth), 1000);
        Ok::<_, harness::ChaosError>(())
    });

    if chaos.is_chaos_enabled() {
        assert!(result.is_err());
    } else {
        assert!(result.is_ok());
    }
}

// ── Chaos-specific negative tests ────────────────────────────────────────────

#[test]
fn chaos_disabled_never_injects_failures() {
    let chaos = ChaosClient::new(false, 1.0, 42);
    for _ in 0..100 {
        let result = chaos.execute("noop", || Ok::<_, ()>(()));
        assert!(result.is_ok(), "chaos disabled should never fail");
    }
}

#[test]
fn chaos_zero_rate_never_injects_failures() {
    let chaos = ChaosClient::new(true, 0.0, 42);
    for _ in 0..100 {
        let result = chaos.execute("noop", || Ok::<_, ()>(()));
        assert!(result.is_ok(), "zero failure rate should never fail");
    }
}

#[test]
fn chaos_full_rate_always_injects_failures() {
    let chaos = ChaosClient::new(true, 1.0, 42);
    for _ in 0..20 {
        let result = chaos.execute("noop", || Ok::<_, ()>(()));
        assert!(result.is_err(), "full failure rate should always fail");
    }
}

#[test]
fn chaos_deterministic_across_seeds() {
    let c1 = ChaosClient::new(true, 0.5, 123);
    let c2 = ChaosClient::new(true, 0.5, 123);
    let mut r1 = std::vec::Vec::new();
    let mut r2 = std::vec::Vec::new();
    for _ in 0..200 {
        r1.push(c1.execute("t", || Ok::<_, ()>(1)).is_ok());
        r2.push(c2.execute("t", || Ok::<_, ()>(1)).is_ok());
    }
    assert_eq!(r1, r2, "same seed must produce same failure sequence");
}

#[test]
fn chaos_different_seeds_differ() {
    let c1 = ChaosClient::new(true, 0.5, 1);
    let c2 = ChaosClient::new(true, 0.5, 2);
    let mut r1 = std::vec::Vec::new();
    let mut r2 = std::vec::Vec::new();
    for _ in 0..200 {
        r1.push(c1.execute("t", || Ok::<_, ()>(1)).is_ok());
        r2.push(c2.execute("t", || Ok::<_, ()>(1)).is_ok());
    }
    assert_ne!(r1, r2, "different seeds should produce different sequences");
}
