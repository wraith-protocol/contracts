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
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{Address, Bytes, BytesN, Env, String as SorobanString};

use governance::{GovernanceContract, GovernanceContractClient};
use stealth_announcer::{
    StealthAnnouncerContract, StealthAnnouncerContractClient, STELLAR_V2_SCHEME_ID,
};
use stealth_batch_sender::{StealthBatchSender, StealthBatchSenderClient, Transfer};
use stealth_registry::{StealthRegistryContract, StealthRegistryContractClient};
use stealth_sender::{StealthSenderContract, StealthSenderContractClient};
use stealth_splitter::{Beneficiary, StealthSplitterContract, StealthSplitterContractClient};
use stealth_vault::{StealthVaultContract, StealthVaultContractClient};
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

/// Minimal target mock for governance execution tests.
mod mock_gov_target {
    use soroban_sdk::{contract, contractimpl, symbol_short, Bytes, Env};

    #[contract]
    pub struct MockGovTarget;

    #[contractimpl]
    impl MockGovTarget {
        pub fn set_value(env: Env, value: Bytes) {
            env.storage()
                .instance()
                .set(&symbol_short!("value"), &value);
        }

        pub fn get_value(env: Env) -> Bytes {
            env.storage()
                .instance()
                .get(&symbol_short!("value"))
                .unwrap_or(Bytes::new(&env))
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

// ── Vault tests ──────────────────────────────────────────────────────────────

/// ChaosClient wrappers for stealth-vault entrypoints.
///
/// # Retry / bail policy (per entrypoint)
///
/// | Entrypoint | Http500 | Timeout | WrongLedger | EmptyResponse |
/// |---|---|---|---|---|
/// | `vault.deposit` | retry ×3, bail `MaxRetriesExceeded` | retry ×1, bail `MaxRetriesExceeded` | no retry, bail `WrongLedger` | retry ×1, bail `EmptyResponse` |
/// | `vault.claim`   | retry ×3, bail `MaxRetriesExceeded` | retry ×1, bail `MaxRetriesExceeded` | no retry, bail `WrongLedger` | retry ×1, bail `EmptyResponse` |
/// | `vault.refund`  | retry ×3, bail `MaxRetriesExceeded` | retry ×1, bail `MaxRetriesExceeded` | no retry, bail `WrongLedger` | retry ×1, bail `EmptyResponse` |
///
/// All three entrypoints share the standard policy defined in `ChaosClient::execute`.
/// `deposit` is the only entrypoint blocked by pause; `claim` and `refund` remain
/// callable even when the contract is paused so recipients can always exit.

#[test]
fn vault_deposit_and_claim_through_chaos() {
    let chaos = ChaosClient::from_env();
    let result = chaos.execute("vault.deposit+claim", || {
        let env = env();
        env.mock_all_auths();

        // Deploy a mock announcer that vault can cross-call.
        let announcer_id = env.register(mock_announcer::MockAnnouncer, ());

        let vault_id = env.register(StealthVaultContract, ());
        let vault = StealthVaultContractClient::new(&env, &vault_id);

        let admin = Address::generate(&env);
        vault.init(&admin, &announcer_id);

        // Mint tokens to sender.
        let (token, sender) = funded_token(&env);

        let recipient = Address::generate(&env);
        let epk = bytes32(&env, &[0xcc; 32]);

        // deposit: unlock at ledger 10, refund after 2010 (> 10 + DEFAULT_GRACE_PERIOD 1000)
        let deposit_id = vault.deposit(&sender, &recipient, &500, &token, &10u32, &2010u32, &epk);

        // Advance ledger past unlock.
        env.ledger().with_mut(|li| li.sequence_number = 10);

        // claim
        vault.claim(&deposit_id, &recipient);

        let token_client = soroban_sdk::token::Client::new(&env, &token);
        assert_eq!(token_client.balance(&recipient), 500);

        Ok::<_, harness::ChaosError>(())
    });

    if chaos.is_chaos_enabled() {
        assert!(result.is_err());
    } else {
        assert!(result.is_ok());
    }
}

#[test]
fn vault_deposit_and_refund_through_chaos() {
    let chaos = ChaosClient::from_env();
    let result = chaos.execute("vault.deposit+refund", || {
        let env = env();
        env.mock_all_auths();

        let announcer_id = env.register(mock_announcer::MockAnnouncer, ());
        let vault_id = env.register(StealthVaultContract, ());
        let vault = StealthVaultContractClient::new(&env, &vault_id);

        let admin = Address::generate(&env);
        vault.init(&admin, &announcer_id);

        let (token, sender) = funded_token(&env);
        let recipient = Address::generate(&env);
        let epk = bytes32(&env, &[0xdd; 32]);

        let deposit_id = vault.deposit(&sender, &recipient, &300, &token, &10u32, &2010u32, &epk);

        let token_client = soroban_sdk::token::Client::new(&env, &token);
        let balance_after_deposit = token_client.balance(&sender);

        // Advance ledger past refund_after.
        env.ledger().with_mut(|li| li.sequence_number = 2010);

        vault.refund(&deposit_id);

        assert_eq!(token_client.balance(&sender), balance_after_deposit + 300);

        Ok::<_, harness::ChaosError>(())
    });

    if chaos.is_chaos_enabled() {
        assert!(result.is_err());
    } else {
        assert!(result.is_ok());
    }
}

// ── Splitter tests ───────────────────────────────────────────────────────────

/// ChaosClient wrappers for stealth-splitter entrypoints.
///
/// # Retry / bail policy (per entrypoint)
///
/// | Entrypoint | Http500 | Timeout | WrongLedger | EmptyResponse |
/// |---|---|---|---|---|
/// | `splitter.create_split` | retry ×3, bail `MaxRetriesExceeded` | retry ×1, bail `MaxRetriesExceeded` | no retry, bail `WrongLedger` | retry ×1, bail `EmptyResponse` |
/// | `splitter.fund_split`   | retry ×3, bail `MaxRetriesExceeded` | retry ×1, bail `MaxRetriesExceeded` | no retry, bail `WrongLedger` | retry ×1, bail `EmptyResponse` |
///
/// `create_split` is idempotent on the same inputs (same split_id returned for same
/// beneficiaries + salt), so Http500 retries are safe. `fund_split` is NOT idempotent;
/// callers must guard against duplicate funding on retry by checking `get_split`
/// `total_funded` before re-submitting.

#[test]
fn splitter_create_and_fund_through_chaos() {
    let chaos = ChaosClient::from_env();
    let result = chaos.execute("splitter.create+fund", || {
        let env = env();
        env.mock_all_auths();

        let announcer_id = env.register(mock_announcer::MockAnnouncer, ());
        let splitter_id = env.register(StealthSplitterContract, ());
        let splitter = StealthSplitterContractClient::new(&env, &splitter_id);

        splitter.init(&announcer_id);

        let creator = Address::generate(&env);
        let (token, funder) = funded_token(&env);

        // Build two beneficiaries with 64-byte meta-addresses.
        let mut beneficiaries = soroban_sdk::vec![&env];
        beneficiaries.push_back(Beneficiary {
            meta_address: bytes(&env, &[0x11u8; 64]),
            weight: 1,
        });
        beneficiaries.push_back(Beneficiary {
            meta_address: bytes(&env, &[0x22u8; 64]),
            weight: 1,
        });

        let salt = bytes(&env, b"chaos-salt");
        let split_id = splitter.create_split(&creator, &beneficiaries, &token, &salt);

        // Prepare per-beneficiary stealth addresses and ephemeral keys.
        let stealth1 = Address::generate(&env);
        let stealth2 = Address::generate(&env);
        let stealth_addrs = soroban_sdk::vec![&env, stealth1.clone(), stealth2.clone()];
        let epks = soroban_sdk::vec![
            &env,
            bytes32(&env, &[0x01u8; 32]),
            bytes32(&env, &[0x02u8; 32])
        ];
        let metadatas = soroban_sdk::vec![&env, bytes(&env, &[0x01]), bytes(&env, &[0x02])];

        splitter.fund_split(
            &funder,
            &split_id,
            &1000,
            &1u32,
            &stealth_addrs,
            &epks,
            &metadatas,
        );

        let details = splitter.get_split(&split_id);
        assert_eq!(details.total_funded, 1000);

        let token_client = soroban_sdk::token::Client::new(&env, &token);
        // Splitter distributes to each beneficiary: first absorbs dust (amount - already_distributed),
        // subsequent beneficiaries get proportional shares. With 2 equal-weight beneficiaries:
        // i=0 (dust): 1000 - 0 = 1000; i=1: 1000 * 1/2 = 500. Both addresses receive tokens.
        assert!(token_client.balance(&stealth1) > 0);
        assert!(token_client.balance(&stealth2) > 0);

        Ok::<_, harness::ChaosError>(())
    });

    if chaos.is_chaos_enabled() {
        assert!(result.is_err());
    } else {
        assert!(result.is_ok());
    }
}

// ── Batch-sender tests ───────────────────────────────────────────────────────

/// ChaosClient wrappers for stealth-batch-sender entrypoints.
///
/// # Retry / bail policy (per entrypoint)
///
/// | Entrypoint | Http500 | Timeout | WrongLedger | EmptyResponse |
/// |---|---|---|---|---|
/// | `batch_sender.batch_send` | retry ×3, bail `MaxRetriesExceeded` | retry ×1, bail `MaxRetriesExceeded` | no retry, bail `WrongLedger` | retry ×1, bail `EmptyResponse` |
///
/// `batch_send` has all-or-nothing semantics at the Soroban transaction level.
/// A WrongLedger bail is final — the caller must re-build the batch and resubmit
/// after confirming the current ledger state. An Http500 or Timeout retry is safe
/// only if the transaction was not yet included; callers should check recipient
/// balances before retrying to avoid double-sends.

#[test]
fn batch_sender_batch_send_through_chaos() {
    let chaos = ChaosClient::from_env();
    let result = chaos.execute("batch_sender.batch_send", || {
        let env = env();
        env.mock_all_auths();

        let batch_sender_id = env.register(StealthBatchSender, ());
        let client = StealthBatchSenderClient::new(&env, &batch_sender_id);

        // The production-hardening pass (issue #155) added a one-time init
        // flow — batch_send now returns NotInitialized until init() is called.
        let admin = Address::generate(&env);
        let announcer_id = env.register(StealthAnnouncerContract, ());
        client.init(&admin, &announcer_id, &None);

        let (token, from) = funded_token(&env);

        let stealth1 = Address::generate(&env);
        let stealth2 = Address::generate(&env);
        let stealth3 = Address::generate(&env);

        let mut transfers = soroban_sdk::vec![&env];
        transfers.push_back(Transfer {
            stealth_address: stealth1.clone(),
            ephemeral_pub_key: bytes(&env, &[0x01u8; 32]),
            amount: 100,
            metadata: bytes(&env, &[0x01]),
        });
        transfers.push_back(Transfer {
            stealth_address: stealth2.clone(),
            ephemeral_pub_key: bytes(&env, &[0x02u8; 32]),
            amount: 200,
            metadata: bytes(&env, &[0x02]),
        });
        transfers.push_back(Transfer {
            stealth_address: stealth3.clone(),
            ephemeral_pub_key: bytes(&env, &[0x03u8; 32]),
            amount: 300,
            metadata: bytes(&env, &[0x03]),
        });

        client.batch_send(&from, &transfers, &token);

        let token_client = soroban_sdk::token::Client::new(&env, &token);
        assert_eq!(token_client.balance(&stealth1), 100);
        assert_eq!(token_client.balance(&stealth2), 200);
        assert_eq!(token_client.balance(&stealth3), 300);

        Ok::<_, harness::ChaosError>(())
    });

    if chaos.is_chaos_enabled() {
        assert!(result.is_err());
    } else {
        assert!(result.is_ok());
    }
}

// ── Governance tests ─────────────────────────────────────────────────────────

/// ChaosClient wrappers for governance entrypoints.
///
/// # Retry / bail policy (per entrypoint)
///
/// | Entrypoint | Http500 | Timeout | WrongLedger | EmptyResponse |
/// |---|---|---|---|---|
/// | `governance.propose` | retry ×3, bail `MaxRetriesExceeded` | retry ×1, bail `MaxRetriesExceeded` | no retry, bail `WrongLedger` | retry ×1, bail `EmptyResponse` |
/// | `governance.vote`    | retry ×3, bail `MaxRetriesExceeded` | retry ×1, bail `MaxRetriesExceeded` | no retry, bail `WrongLedger` | retry ×1, bail `EmptyResponse` |
/// | `governance.execute` | retry ×3, bail `MaxRetriesExceeded` | retry ×1, bail `MaxRetriesExceeded` | no retry, bail `WrongLedger` | retry ×1, bail `EmptyResponse` |
///
/// `propose` is not idempotent: each retry creates a new proposal.  Callers should
/// query `get_proposal` before retrying to confirm the first attempt did not land.
/// `vote` is idempotent for a given (proposal_id, voter) pair; the contract rejects
/// duplicate votes with `AlreadyVoted`, so Http500 retries are safe.
/// `execute` is idempotent; the contract rejects re-execution with `AlreadyExecuted`.

#[test]
fn governance_propose_vote_execute_through_chaos() {
    let chaos = ChaosClient::from_env();
    let result = chaos.execute("governance.propose+vote+execute", || {
        let env = env();
        env.mock_all_auths();

        // Deploy governance and dependencies.
        let gov_id = env.register(GovernanceContract, ());
        let gov = GovernanceContractClient::new(&env, &gov_id);

        let target_id = env.register(mock_gov_target::MockGovTarget, ());

        let admin = Address::generate(&env);
        let token_admin = Address::generate(&env);
        let token_id = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();

        // quorum=100, voting_period=50, timelock=10
        gov.init(&admin, &token_id, &100i128, &50u32, &10u32);

        // Mint voting power.
        let voter = Address::generate(&env);
        soroban_sdk::token::StellarAssetClient::new(&env, &token_id).mint(&voter, &200);

        let proposer = Address::generate(&env);
        let function = soroban_sdk::symbol_short!("set_value");
        let args = bytes(&env, b"chaos-gov");
        let description = soroban_sdk::String::from_str(&env, "chaos governance proposal");

        let pid = gov.propose(&proposer, &target_id, &function, &args, &description);

        gov.vote(&voter, &pid, &true);

        let proposal = gov.get_proposal(&pid);

        // Advance past end_ledger + timelock.
        env.ledger().with_mut(|li| {
            li.sequence_number = proposal.end_ledger + 20;
        });

        gov.execute(&pid);

        let p2 = gov.get_proposal(&pid);
        assert!(p2.executed);

        Ok::<_, harness::ChaosError>(())
    });

    if chaos.is_chaos_enabled() {
        assert!(result.is_err());
    } else {
        assert!(result.is_ok());
    }
}
