//! Storage rent and TTL boundary tests.
//!
//! Covers the persistent / instance storage paths of the registry, names,
//! vault and splitter contracts at their TTL boundaries:
//!
//! - entries are written with the expected TTL,
//! - renewal happens only once the remaining TTL is at or below the threshold,
//! - an entry is still live on its `live_until` ledger and archived one
//!   ledger later,
//! - renewing before expiry keeps an entry live past its original expiry,
//! - touching an entry after expiry fails (it must be restored first),
//! - missing entries surface the contract's "not found" error and cannot be
//!   extended.
//!
//! The Soroban test host panics with `Error(Storage, InternalError)` when an
//! archived entry is accessed; on a real network the transaction would be
//! rejected before the contract ran. Both mean the call cannot succeed.

use soroban_sdk::testutils::storage::{Instance as _, Persistent as _};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{token, vec, Address, Bytes, BytesN, Env, String, Vec};

use stealth_registry::{
    DataKey as RegistryKey, RegistryError, StealthRegistryContract, StealthRegistryContractClient,
};
use stealth_splitter::{
    Beneficiary, SplitterError, StealthSplitterContract, StealthSplitterContractClient,
};
use stealth_vault::{
    DataKey as VaultKey, StealthVaultContract, StealthVaultContractClient, VaultError,
};
use wraith_names::{
    DataKey as NamesKey, NamesError, WraithNamesContract, WraithNamesContractClient,
};

/// Mirrors the contracts' `TTL_THRESHOLD` (~1 day).
const TTL_THRESHOLD: u32 = 17_280;
/// Mirrors the contracts' `TTL_EXTEND_TO` (~30 days).
const TTL_EXTEND_TO: u32 = 518_400;
/// Network maximum entry TTL configured for these tests.
const MAX_ENTRY_TTL: u32 = 1_000_000;
/// Minimum TTL a freshly created persistent / instance entry receives.
const MIN_PERSISTENT_TTL: u32 = 100;

fn setup_env() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.sequence_number = 1_000;
        li.min_persistent_entry_ttl = MIN_PERSISTENT_TTL;
        li.min_temp_entry_ttl = 16;
        li.max_entry_ttl = MAX_ENTRY_TTL;
    });
    env
}

fn advance(env: &Env, ledgers: u32) {
    env.ledger().with_mut(|li| li.sequence_number += ledgers);
}

/// Keep a contract instance live for the rest of the test so that a
/// persistent entry's expiry can be observed in isolation.
fn pin_instance(env: &Env, contract: &Address) {
    env.as_contract(contract, || {
        env.storage()
            .instance()
            .extend_ttl(MAX_ENTRY_TTL - 1, MAX_ENTRY_TTL - 1)
    });
}

fn instance_ttl(env: &Env, contract: &Address) -> u32 {
    env.as_contract(contract, || env.storage().instance().get_ttl())
}

fn meta(env: &Env, fill: u8) -> Bytes {
    Bytes::from_slice(env, &[fill; 64])
}

fn sha256(env: &Env, data: &Bytes) -> BytesN<32> {
    BytesN::from_array(env, &env.crypto().sha256(data).to_array())
}

// ── Registry ────────────────────────────────────────────────────────────────

struct Registry {
    env: Env,
    id: Address,
    client: StealthRegistryContractClient<'static>,
    registrant: Address,
    key: RegistryKey,
}

impl Registry {
    fn new() -> Self {
        let env = setup_env();
        let id = env.register(StealthRegistryContract, ());
        let client = StealthRegistryContractClient::new(&env, &id);
        let registrant = Address::generate(&env);
        client.register_keys(&registrant, &1, &meta(&env, 1));
        let key = RegistryKey::MetaAddress(registrant.clone(), 1);
        Self {
            env,
            id,
            client,
            registrant,
            key,
        }
    }

    fn ttl(&self) -> u32 {
        self.env.as_contract(&self.id, || {
            self.env.storage().persistent().get_ttl(&self.key)
        })
    }

    fn lookup(&self) -> Result<Bytes, RegistryError> {
        self.client
            .try_stealth_meta_address_of(&self.registrant, &1)
            .map(|r| r.unwrap())
            .map_err(|e| e.unwrap())
    }
}

#[test]
fn registry_register_sets_entry_and_instance_ttl() {
    let r = Registry::new();
    assert_eq!(r.ttl(), TTL_EXTEND_TO);
    assert_eq!(instance_ttl(&r.env, &r.id), TTL_EXTEND_TO);
}

#[test]
fn registry_read_above_threshold_does_not_renew() {
    let r = Registry::new();
    advance(&r.env, TTL_EXTEND_TO - TTL_THRESHOLD - 1);
    assert_eq!(r.ttl(), TTL_THRESHOLD + 1);

    assert!(r.lookup().is_ok());
    assert_eq!(r.ttl(), TTL_THRESHOLD + 1);
}

#[test]
fn registry_read_at_threshold_renews_entry_and_instance() {
    let r = Registry::new();
    advance(&r.env, TTL_EXTEND_TO - TTL_THRESHOLD);
    assert_eq!(r.ttl(), TTL_THRESHOLD);

    assert_eq!(r.lookup(), Ok(meta(&r.env, 1)));
    assert_eq!(r.ttl(), TTL_EXTEND_TO);
    assert_eq!(instance_ttl(&r.env, &r.id), TTL_EXTEND_TO);
}

#[test]
fn registry_entry_live_on_last_ledger_and_renewed_by_read() {
    let r = Registry::new();
    advance(&r.env, TTL_EXTEND_TO);
    assert_eq!(r.ttl(), 0);

    assert_eq!(r.lookup(), Ok(meta(&r.env, 1)));
    assert_eq!(r.ttl(), TTL_EXTEND_TO);
}

#[test]
fn registry_renewal_before_expiry_outlives_original_expiry() {
    let r = Registry::new();
    advance(&r.env, TTL_EXTEND_TO - 1);
    assert!(r.lookup().is_ok());

    // Well past the original live_until ledger.
    advance(&r.env, TTL_EXTEND_TO - 1);
    assert_eq!(r.lookup(), Ok(meta(&r.env, 1)));
}

#[test]
fn registry_update_before_expiry_renews_entry() {
    let r = Registry::new();
    advance(&r.env, TTL_EXTEND_TO - 10);
    r.client.register_keys(&r.registrant, &1, &meta(&r.env, 2));
    assert_eq!(r.ttl(), TTL_EXTEND_TO);
    assert_eq!(r.lookup(), Ok(meta(&r.env, 2)));
}

#[test]
#[should_panic(expected = "Error(Storage, InternalError)")]
fn registry_read_after_expiry_fails() {
    let r = Registry::new();
    pin_instance(&r.env, &r.id);
    advance(&r.env, TTL_EXTEND_TO + 1);
    let _ = r.lookup();
}

#[test]
#[should_panic(expected = "Error(Storage, InternalError)")]
fn registry_renewal_after_expiry_fails() {
    let r = Registry::new();
    pin_instance(&r.env, &r.id);
    advance(&r.env, TTL_EXTEND_TO + 1);
    r.client.register_keys(&r.registrant, &1, &meta(&r.env, 2));
}

#[test]
fn registry_missing_entry_is_not_registered_and_not_extended() {
    let r = Registry::new();
    r.client.remove_keys(&r.registrant, &1);
    assert_eq!(r.lookup(), Err(RegistryError::NotRegistered));

    let stranger = Address::generate(&r.env);
    assert_eq!(
        r.client.try_stealth_meta_address_of(&stranger, &1),
        Err(Ok(RegistryError::NotRegistered))
    );
    let missing = RegistryKey::MetaAddress(stranger, 1);
    r.env.as_contract(&r.id, || {
        assert!(!r.env.storage().persistent().has(&missing));
    });
}

#[test]
#[should_panic(expected = "HostError")]
fn registry_missing_entry_cannot_be_extended() {
    let r = Registry::new();
    let missing = RegistryKey::MetaAddress(Address::generate(&r.env), 1);
    r.env.as_contract(&r.id, || {
        r.env
            .storage()
            .persistent()
            .extend_ttl(&missing, TTL_THRESHOLD, TTL_EXTEND_TO)
    });
}

// ── Names ───────────────────────────────────────────────────────────────────

struct Names {
    env: Env,
    id: Address,
    client: WraithNamesContractClient<'static>,
    owner: Address,
    name: String,
    meta: Bytes,
    name_key: NamesKey,
    reverse_key: NamesKey,
}

impl Names {
    fn new() -> Self {
        let env = setup_env();
        let id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &id);
        client.init(&Address::generate(&env));
        let owner = Address::generate(&env);
        let name = String::from_str(&env, "alice");
        let meta = meta(&env, 1);
        client.register(&owner, &name, &meta);
        let name_key = NamesKey::Name(sha256(&env, &Bytes::from_slice(&env, b"alice")));
        let reverse_key = NamesKey::Reverse(sha256(&env, &meta));
        Self {
            env,
            id,
            client,
            owner,
            name,
            meta,
            name_key,
            reverse_key,
        }
    }

    fn ttl(&self, key: &NamesKey) -> u32 {
        self.env
            .as_contract(&self.id, || self.env.storage().persistent().get_ttl(key))
    }

    fn seq(&self) -> u32 {
        self.env.ledger().sequence()
    }
}

#[test]
fn names_register_sets_name_and_reverse_ttl() {
    let n = Names::new();
    assert_eq!(n.ttl(&n.name_key), TTL_EXTEND_TO);
    assert_eq!(n.ttl(&n.reverse_key), TTL_EXTEND_TO);
}

#[test]
fn names_resolve_above_threshold_does_not_renew() {
    let n = Names::new();
    advance(&n.env, TTL_EXTEND_TO - TTL_THRESHOLD - 1);
    assert_eq!(n.client.resolve(&n.name), n.meta);
    assert_eq!(n.ttl(&n.name_key), TTL_THRESHOLD + 1);
}

#[test]
fn names_resolve_at_threshold_renews_forward_entry_only() {
    let n = Names::new();
    advance(&n.env, TTL_EXTEND_TO - TTL_THRESHOLD);
    assert_eq!(n.client.resolve(&n.name), n.meta);
    assert_eq!(n.ttl(&n.name_key), TTL_EXTEND_TO);
    assert_eq!(n.ttl(&n.reverse_key), TTL_THRESHOLD);
}

#[test]
fn names_reverse_lookup_at_threshold_renews_both_entries() {
    let n = Names::new();
    advance(&n.env, TTL_EXTEND_TO - TTL_THRESHOLD);
    assert_eq!(n.client.name_of(&n.meta), n.name);
    assert_eq!(n.ttl(&n.name_key), TTL_EXTEND_TO);
    assert_eq!(n.ttl(&n.reverse_key), TTL_EXTEND_TO);
}

#[test]
fn names_entries_live_on_last_ledger() {
    let n = Names::new();
    advance(&n.env, TTL_EXTEND_TO);
    assert_eq!(n.ttl(&n.name_key), 0);
    assert_eq!(n.ttl(&n.reverse_key), 0);
    assert_eq!(n.client.name_of(&n.meta), n.name);
    assert_eq!(n.ttl(&n.name_key), TTL_EXTEND_TO);
    assert_eq!(n.ttl(&n.reverse_key), TTL_EXTEND_TO);
}

#[test]
fn names_extend_name_ttl_before_expiry_outlives_original_expiry() {
    let n = Names::new();
    advance(&n.env, TTL_EXTEND_TO - 1);
    let extend_to = n.seq() + 1;
    n.client.extend_name_ttl(&n.name, &extend_to);
    assert!(n.ttl(&n.name_key) >= TTL_EXTEND_TO);
    assert!(n.ttl(&n.reverse_key) >= TTL_EXTEND_TO);

    advance(&n.env, TTL_EXTEND_TO - 1);
    assert_eq!(n.client.resolve(&n.name), n.meta);
    assert_eq!(n.client.name_of(&n.meta), n.name);
}

#[test]
fn names_extend_name_ttl_is_noop_while_ttl_above_current_ledger() {
    // extend_name_ttl uses the current ledger sequence as its threshold, so
    // an entry whose remaining TTL exceeds that number is left untouched.
    let n = Names::new();
    assert!(n.ttl(&n.name_key) > n.seq());
    n.client
        .extend_name_ttl(&n.name, &(n.seq() + TTL_EXTEND_TO * 2));
    assert_eq!(n.ttl(&n.name_key), TTL_EXTEND_TO);
    assert_eq!(n.ttl(&n.reverse_key), TTL_EXTEND_TO);
}

#[test]
fn names_extend_name_ttl_is_capped_at_max_entry_ttl() {
    let n = Names::new();
    advance(&n.env, TTL_EXTEND_TO - 10);
    n.client.extend_name_ttl(&n.name, &(MAX_ENTRY_TTL * 2));
    assert!(n.ttl(&n.name_key) < MAX_ENTRY_TTL);
    assert!(n.ttl(&n.name_key) > TTL_EXTEND_TO);
    assert_eq!(n.ttl(&n.name_key), n.ttl(&n.reverse_key));
}

#[test]
fn names_extend_name_ttl_rejects_non_future_ledger() {
    let n = Names::new();
    let seq = n.seq();
    assert_eq!(
        n.client.try_extend_name_ttl(&n.name, &seq),
        Err(Ok(NamesError::InvalidExtendLedger))
    );
    assert_eq!(
        n.client.try_extend_name_ttl(&n.name, &(seq - 1)),
        Err(Ok(NamesError::InvalidExtendLedger))
    );
    assert_eq!(
        n.client.try_bulk_renew(&vec![&n.env, n.name.clone()], &seq),
        Err(Ok(NamesError::InvalidExtendLedger))
    );
    assert_eq!(n.ttl(&n.name_key), TTL_EXTEND_TO);
}

#[test]
fn names_missing_entry_cannot_be_extended() {
    let n = Names::new();
    let ghost = String::from_str(&n.env, "ghost");
    let extend_to = n.seq() + TTL_EXTEND_TO;
    assert_eq!(
        n.client.try_extend_name_ttl(&ghost, &extend_to),
        Err(Ok(NamesError::NameNotFound))
    );
    assert_eq!(
        n.client.try_resolve(&ghost),
        Err(Ok(NamesError::NameNotFound))
    );
    assert_eq!(
        n.client.try_name_of(&meta(&n.env, 9)),
        Err(Ok(NamesError::NameNotFound))
    );
}

#[test]
fn names_bulk_renew_with_missing_entry_renews_nothing() {
    let n = Names::new();
    advance(&n.env, TTL_EXTEND_TO - 10);
    let names: Vec<String> = vec![&n.env, n.name.clone(), String::from_str(&n.env, "ghost")];
    assert_eq!(
        n.client.try_bulk_renew(&names, &(n.seq() + 1)),
        Err(Ok(NamesError::NameNotFound))
    );
    assert_eq!(n.ttl(&n.name_key), 10);
    assert_eq!(n.ttl(&n.reverse_key), 10);
}

#[test]
fn names_released_entry_is_missing() {
    let n = Names::new();
    n.client.release(&n.owner, &n.name);
    assert_eq!(
        n.client.try_resolve(&n.name),
        Err(Ok(NamesError::NameNotFound))
    );
    assert_eq!(
        n.client.try_extend_name_ttl(&n.name, &(n.seq() + 1)),
        Err(Ok(NamesError::NameNotFound))
    );
}

#[test]
#[should_panic(expected = "Error(Storage, InternalError)")]
fn names_resolve_after_expiry_fails() {
    let n = Names::new();
    pin_instance(&n.env, &n.id);
    advance(&n.env, TTL_EXTEND_TO + 1);
    let _ = n.client.try_resolve(&n.name);
}

#[test]
#[should_panic(expected = "Error(Storage, InternalError)")]
fn names_renewal_after_expiry_fails() {
    let n = Names::new();
    pin_instance(&n.env, &n.id);
    advance(&n.env, TTL_EXTEND_TO + 1);
    let _ = n.client.try_extend_name_ttl(&n.name, &(n.seq() + 1));
}

// ── Vault ───────────────────────────────────────────────────────────────────

mod mock_announcer {
    use soroban_sdk::{contract, contractimpl, Address, Bytes, BytesN, Env};

    #[contract]
    pub struct MockAnnouncer;

    #[contractimpl]
    impl MockAnnouncer {
        pub fn announce(
            _env: Env,
            _scheme_id: u32,
            _stealth_address: Address,
            _ephemeral_pub_key: BytesN<32>,
            _metadata: Bytes,
        ) {
        }
    }
}

struct Vault {
    env: Env,
    id: Address,
    client: StealthVaultContractClient<'static>,
    recipient: Address,
    deposit_id: BytesN<32>,
    key: VaultKey,
}

impl Vault {
    fn new() -> Self {
        let env = setup_env();
        let announcer = env.register(mock_announcer::MockAnnouncer, ());
        let id = env.register(StealthVaultContract, ());
        let client = StealthVaultContractClient::new(&env, &id);
        client.init(&Address::generate(&env), &announcer);

        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);
        let token_id = env
            .register_stellar_asset_contract_v2(Address::generate(&env))
            .address();
        token::StellarAssetClient::new(&env, &token_id).mint(&sender, &1_000);

        let seq = env.ledger().sequence();
        let deposit_id = client.deposit(
            &sender,
            &recipient,
            &1_000,
            &token_id,
            &(seq + 10),
            &(seq + 10 + client.grace_period() + 10),
            &BytesN::from_array(&env, &[7u8; 32]),
        );
        let key = VaultKey::Deposit(deposit_id.clone());
        Self {
            env,
            id,
            client,
            recipient,
            deposit_id,
            key,
        }
    }

    fn ttl(&self) -> u32 {
        self.env.as_contract(&self.id, || {
            self.env.storage().persistent().get_ttl(&self.key)
        })
    }

    /// Operator-side renewal (e.g. an `ExtendFootprintTTLOp`); the vault has
    /// no in-contract renewal path for deposits.
    fn renew(&self) {
        self.env.as_contract(&self.id, || {
            self.env
                .storage()
                .persistent()
                .extend_ttl(&self.key, TTL_THRESHOLD, TTL_EXTEND_TO)
        });
    }
}

#[test]
fn vault_deposit_sets_entry_ttl() {
    let v = Vault::new();
    assert_eq!(v.ttl(), TTL_EXTEND_TO);
    assert_eq!(instance_ttl(&v.env, &v.id), TTL_EXTEND_TO);
}

#[test]
fn vault_get_deposit_does_not_renew() {
    let v = Vault::new();
    advance(&v.env, TTL_EXTEND_TO - 1);
    assert_eq!(v.client.get_deposit(&v.deposit_id).amount, 1_000);
    assert_eq!(v.ttl(), 1);
}

#[test]
fn vault_deposit_live_on_last_ledger() {
    let v = Vault::new();
    pin_instance(&v.env, &v.id);
    advance(&v.env, TTL_EXTEND_TO);
    assert_eq!(v.ttl(), 0);
    assert_eq!(v.client.get_deposit(&v.deposit_id).recipient, v.recipient);
}

#[test]
fn vault_renewal_before_expiry_outlives_original_expiry() {
    let v = Vault::new();
    advance(&v.env, TTL_EXTEND_TO - TTL_THRESHOLD);
    pin_instance(&v.env, &v.id);
    v.renew();
    assert_eq!(v.ttl(), TTL_EXTEND_TO);

    advance(&v.env, TTL_EXTEND_TO - 1);
    assert_eq!(v.client.get_deposit(&v.deposit_id).amount, 1_000);
}

#[test]
fn vault_renewal_above_threshold_is_noop() {
    let v = Vault::new();
    advance(&v.env, TTL_EXTEND_TO - TTL_THRESHOLD - 1);
    v.renew();
    assert_eq!(v.ttl(), TTL_THRESHOLD + 1);
}

#[test]
#[should_panic(expected = "Error(Storage, InternalError)")]
fn vault_read_after_expiry_fails() {
    let v = Vault::new();
    pin_instance(&v.env, &v.id);
    advance(&v.env, TTL_EXTEND_TO + 1);
    let _ = v.client.try_get_deposit(&v.deposit_id);
}

#[test]
#[should_panic(expected = "Error(Storage, InternalError)")]
fn vault_claim_after_expiry_fails() {
    let v = Vault::new();
    pin_instance(&v.env, &v.id);
    advance(&v.env, TTL_EXTEND_TO + 1);
    let _ = v.client.try_claim(&v.deposit_id, &v.recipient);
}

#[test]
#[should_panic(expected = "Error(Storage, InternalError)")]
fn vault_renewal_after_expiry_fails() {
    let v = Vault::new();
    pin_instance(&v.env, &v.id);
    advance(&v.env, TTL_EXTEND_TO + 1);
    v.renew();
}

#[test]
fn vault_claimed_deposit_is_missing() {
    let v = Vault::new();
    advance(&v.env, 10);
    v.client.claim(&v.deposit_id, &v.recipient);
    assert_eq!(
        v.client.try_get_deposit(&v.deposit_id),
        Err(Ok(VaultError::DepositNotFound))
    );
    assert_eq!(
        v.client.try_claim(&v.deposit_id, &v.recipient),
        Err(Ok(VaultError::DepositNotFound))
    );
    assert_eq!(
        v.client.try_refund(&v.deposit_id),
        Err(Ok(VaultError::DepositNotFound))
    );
}

#[test]
#[should_panic(expected = "HostError")]
fn vault_missing_deposit_cannot_be_extended() {
    let v = Vault::new();
    advance(&v.env, 10);
    v.client.claim(&v.deposit_id, &v.recipient);
    v.renew();
}

// ── Splitter ────────────────────────────────────────────────────────────────

struct Splitter {
    env: Env,
    id: Address,
    client: StealthSplitterContractClient<'static>,
    split_id: BytesN<32>,
}

impl Splitter {
    fn new() -> Self {
        let env = setup_env();
        let announcer = env.register(mock_announcer::MockAnnouncer, ());
        let id = env.register(StealthSplitterContract, ());
        let client = StealthSplitterContractClient::new(&env, &id);
        client.init(&announcer);
        let beneficiaries = vec![
            &env,
            Beneficiary {
                meta_address: meta(&env, 1),
                weight: 1,
            },
        ];
        let split_id = client.create_split(
            &Address::generate(&env),
            &beneficiaries,
            &Address::generate(&env),
            &Bytes::from_slice(&env, b"salt"),
        );
        Self {
            env,
            id,
            client,
            split_id,
        }
    }

    fn ttl(&self) -> u32 {
        instance_ttl(&self.env, &self.id)
    }

    /// Operator-side instance renewal; the splitter never extends its own TTL.
    fn renew(&self) {
        self.env.as_contract(&self.id, || {
            self.env
                .storage()
                .instance()
                .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO)
        });
    }
}

#[test]
fn splitter_writes_and_reads_do_not_renew_instance() {
    let s = Splitter::new();
    let initial = s.ttl();
    assert!(initial >= MIN_PERSISTENT_TTL - 1);
    assert!(initial < TTL_THRESHOLD);

    advance(&s.env, initial);
    assert!(s.client.get_split(&s.split_id).total_funded == 0);
    assert_eq!(s.ttl(), 0);
}

#[test]
fn splitter_split_live_on_last_instance_ledger() {
    let s = Splitter::new();
    advance(&s.env, s.ttl());
    assert_eq!(s.client.get_split(&s.split_id).beneficiaries.len(), 1);
}

#[test]
fn splitter_renewal_before_expiry_outlives_original_expiry() {
    let s = Splitter::new();
    let initial = s.ttl();
    s.renew();
    assert_eq!(s.ttl(), TTL_EXTEND_TO);

    advance(&s.env, initial + 1);
    assert_eq!(s.client.get_split(&s.split_id).beneficiaries.len(), 1);

    advance(&s.env, TTL_EXTEND_TO - initial - 1);
    assert_eq!(s.ttl(), 0);
    assert_eq!(s.client.get_split(&s.split_id).beneficiaries.len(), 1);
}

#[test]
#[should_panic(expected = "Error(Storage, InternalError)")]
fn splitter_read_after_instance_expiry_fails() {
    let s = Splitter::new();
    advance(&s.env, s.ttl() + 1);
    let _ = s.client.try_get_split(&s.split_id);
}

#[test]
#[should_panic(expected = "Error(Storage, InternalError)")]
fn splitter_renewal_after_instance_expiry_fails() {
    let s = Splitter::new();
    advance(&s.env, s.ttl() + 1);
    s.renew();
}

#[test]
fn splitter_missing_split_is_not_found() {
    let s = Splitter::new();
    let ghost = BytesN::from_array(&s.env, &[0u8; 32]);
    assert!(matches!(
        s.client.try_get_split(&ghost),
        Err(Ok(SplitterError::SplitNotFound))
    ));
}
