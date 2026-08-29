#![no_std]

#[cfg(not(kani))]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, Bytes, BytesN, Env,
    IntoVal,
};
#[cfg(not(kani))]
use wraith_metrics::{contract_ids, dimension_names, emit_metric, metric_names};

/// Hand-rolled stand-in for `soroban_sdk` used only when compiling under Kani.
///
/// The real SDK is a host-call shim that the model checker cannot reason about,
/// so `[target.'cfg(kani)']` drops it from the dependency graph and this module
/// supplies the same surface backed by plain Rust data structures.
#[cfg(kani)]
pub mod mock_sdk;

#[cfg(kani)]
pub mod soroban_sdk {
    pub use crate::mock_sdk::*;
    pub use crate::mock_symbol_short as symbol_short;
    pub use crate::mock_vec as vec;
}

#[cfg(kani)]
pub mod wraith_metrics {
    pub use crate::mock_sdk::contract_ids;
    pub use crate::mock_sdk::dimension_names;
    pub use crate::mock_sdk::emit_metric;
    pub use crate::mock_sdk::metric_names;
}

#[cfg(kani)]
#[allow(unused_imports)]
use mock_sdk::{
    contract_ids, dimension_names, emit_metric, metric_names, token, Address, Bytes, BytesN, Env,
    IntoVal,
};

#[cfg(kani)]
mod proofs;

/// Default minimum number of ledgers that must separate `unlock_ledger` from
/// `refund_after`, and `refund_after` from the permissionless refund window.
///
/// Admins can retune this with `set_grace_period` without redeploying.
pub const DEFAULT_GRACE_PERIOD: u32 = 1000;

/// Scheme id the vault announces under.
///
/// Must match `stealth_announcer::STELLAR_V2_SCHEME_ID`; the announcer asserts
/// on it, so a mismatch reverts every `deposit`. `tests/announcer.rs` wires the
/// real announcer to keep the two in step.
const ANNOUNCE_SCHEME_ID: u32 = 2;

const TTL_THRESHOLD: u32 = 17280;
const TTL_EXTEND_TO: u32 = 518400;

#[cfg_attr(not(kani), contracttype)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DataKey {
    /// Maps a deposit id to its `DepositEntry`.
    Deposit(BytesN<32>),
    /// Address of the stealth announcer contract.
    Announcer,
    /// Pause admin address.
    Admin,
    /// Whether the contract is paused.
    Paused,
    /// Configurable grace period, in ledgers.
    GracePeriod,
}

#[cfg_attr(not(kani), contracttype)]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DepositEntry {
    pub sender: Address,
    pub recipient: Address,
    pub amount: i128,
    pub asset: Address,
    pub unlock_ledger: u32,
    pub refund_after: u32,
}

#[cfg_attr(not(kani), contracterror)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum VaultError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidWindow = 3,
    DepositNotFound = 4,
    NotYetUnlocked = 5,
    NotYetRefundable = 6,
    WrongRecipient = 7,
    /// The contract is paused; `deposit` is unavailable until an admin unpauses.
    Paused = 8,
    /// The permissionless refund window has not opened yet.
    NotYetPermissionless = 9,
    /// `set_grace_period` was called with zero.
    InvalidGracePeriod = 10,
}

mod announcer_client {
    // Nested modules do not see the crate-root `soroban_sdk` shim through a
    // bare path, so name it explicitly on the Kani side.
    #[cfg(kani)]
    #[allow(unused_imports)]
    use crate::soroban_sdk::{symbol_short, vec, Address, Bytes, BytesN, Env, IntoVal};
    #[cfg(not(kani))]
    use soroban_sdk::{symbol_short, vec, Address, Bytes, BytesN, Env, IntoVal};

    pub fn announce(
        env: &Env,
        announcer: &Address,
        scheme_id: u32,
        stealth_address: &Address,
        ephemeral_pub_key: &BytesN<32>,
        metadata: &Bytes,
    ) {
        let _: () = env.invoke_contract(
            announcer,
            &symbol_short!("announce"),
            vec![
                env,
                scheme_id.into_val(env),
                stealth_address.into_val(env),
                ephemeral_pub_key.into_val(env),
                metadata.into_val(env),
            ],
        );
    }
}

#[cfg_attr(not(kani), contract)]
pub struct StealthVaultContract;

#[cfg_attr(not(kani), contractimpl)]
impl StealthVaultContract {
    /// Initialise the vault with its pause admin and announcer address.
    ///
    /// Must be called exactly once before any `deposit`. The grace period is
    /// seeded to `DEFAULT_GRACE_PERIOD` and can be retuned by the admin.
    pub fn init(env: Env, admin: Address, announcer: Address) -> Result<(), VaultError> {
        if env.storage().instance().has(&DataKey::Announcer) {
            return Err(VaultError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::Announcer, &announcer);
        env.storage()
            .instance()
            .set(&DataKey::GracePeriod, &DEFAULT_GRACE_PERIOD);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        Ok(())
    }

    /// Returns the pause admin.
    pub fn admin(env: Env) -> Result<Address, VaultError> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(VaultError::NotInitialized)
    }

    /// Pause the contract — admin only.
    ///
    /// Blocks new deposits. `claim`, `refund`, and `refund_permissionless`
    /// stay callable so depositors and recipients can always exit.
    pub fn pause(env: Env, caller: Address) -> Result<(), VaultError> {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("admin not set — call init first");
        if caller != admin {
            panic!("unauthorized: only admin can pause");
        }
        env.storage().instance().set(&DataKey::Paused, &true);
        env.events()
            .publish((soroban_sdk::symbol_short!("paused"),), (caller,));
        Ok(())
    }

    /// Unpause the contract — admin only.
    pub fn unpause(env: Env, caller: Address) -> Result<(), VaultError> {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("admin not set — call init first");
        if caller != admin {
            panic!("unauthorized: only admin can unpause");
        }
        env.storage().instance().set(&DataKey::Paused, &false);
        env.events()
            .publish((soroban_sdk::symbol_short!("unpaused"),), (caller,));
        Ok(())
    }

    /// Returns true if the contract is paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    /// Returns the configured grace period in ledgers.
    pub fn grace_period(env: Env) -> u32 {
        Self::grace_period_value(&env)
    }

    /// Retune the grace period — admin only.
    ///
    /// Applies to deposits made after the change; already-stored deposits keep
    /// the absolute `unlock_ledger` / `refund_after` they were created with,
    /// but their permissionless refund window moves with the new value.
    pub fn set_grace_period(
        env: Env,
        caller: Address,
        grace_period: u32,
    ) -> Result<(), VaultError> {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("admin not set — call init first");
        if caller != admin {
            panic!("unauthorized: only admin can set the grace period");
        }
        if grace_period == 0 {
            return Err(VaultError::InvalidGracePeriod);
        }
        env.storage()
            .instance()
            .set(&DataKey::GracePeriod, &grace_period);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        env.events().publish(
            (soroban_sdk::symbol_short!("grace"),),
            (caller, grace_period),
        );
        Ok(())
    }

    /// Look up a stored deposit without mutating it.
    pub fn get_deposit(env: Env, deposit_id: BytesN<32>) -> Result<DepositEntry, VaultError> {
        env.storage()
            .persistent()
            .get(&DataKey::Deposit(deposit_id))
            .ok_or(VaultError::DepositNotFound)
    }

    pub fn deposit(
        env: Env,
        sender: Address,
        recipient: Address,
        amount: i128,
        asset: Address,
        unlock_ledger: u32,
        refund_after: u32,
        ephemeral_pub_key: BytesN<32>,
    ) -> Result<BytesN<32>, VaultError> {
        Self::require_not_paused(&env)?;
        sender.require_auth();

        let grace = Self::grace_period_value(&env);
        if refund_after <= unlock_ledger.saturating_add(grace) {
            return Err(VaultError::InvalidWindow);
        }

        let announcer: Address = env
            .storage()
            .instance()
            .get(&DataKey::Announcer)
            .ok_or(VaultError::NotInitialized)?;

        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);

        // Build deposit_id from key fields
        let seq = env.ledger().sequence();
        let mut id_bytes = Bytes::new(&env);
        id_bytes.append(&Bytes::from_slice(&env, &amount.to_be_bytes()));
        id_bytes.append(&Bytes::from_slice(&env, &unlock_ledger.to_be_bytes()));
        id_bytes.append(&Bytes::from_slice(&env, &refund_after.to_be_bytes()));
        id_bytes.append(&ephemeral_pub_key.clone().into());
        id_bytes.append(&Bytes::from_slice(&env, &seq.to_be_bytes()));
        let deposit_id: BytesN<32> = env.crypto().sha256(&id_bytes).into();

        // Transfer tokens from sender to this contract
        token::Client::new(&env, &asset).transfer(
            &sender,
            &env.current_contract_address(),
            &amount,
        );

        // Store deposit
        let entry = DepositEntry {
            sender: sender.clone(),
            recipient: recipient.clone(),
            amount,
            asset: asset.clone(),
            unlock_ledger,
            refund_after,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Deposit(deposit_id.clone()), &entry);
        env.storage().persistent().extend_ttl(
            &DataKey::Deposit(deposit_id.clone()),
            TTL_THRESHOLD,
            TTL_EXTEND_TO,
        );

        // Emit announcement so recipient finds it during normal scan
        // metadata = [view_tag] where view_tag = first byte of ephemeral_pub_key
        let view_tag = ephemeral_pub_key.get(0).unwrap_or(0);
        let metadata = Bytes::from_slice(&env, &[view_tag]);
        announcer_client::announce(
            &env,
            &announcer,
            ANNOUNCE_SCHEME_ID,
            &recipient,
            &ephemeral_pub_key,
            &metadata,
        );

        // Emit deposit event
        env.events().publish(
            (soroban_sdk::symbol_short!("deposit"), deposit_id.clone()),
            (sender, amount, asset.clone(), unlock_ledger),
        );

        // Emit metric events.
        let dimensions =
            soroban_sdk::vec![&env, (dimension_names::ASSET_ADDRESS, asset.into_val(&env))];
        emit_metric(
            &env,
            contract_ids::STEALTH_VAULT,
            metric_names::DEPOSIT_COUNT,
            1,
            dimensions.clone(),
        );
        emit_metric(
            &env,
            contract_ids::STEALTH_VAULT,
            metric_names::DEPOSIT_VOLUME,
            amount,
            dimensions,
        );

        Ok(deposit_id)
    }

    /// Claim a deposit. Callable while paused so recipients can always exit.
    pub fn claim(env: Env, deposit_id: BytesN<32>, recipient: Address) -> Result<(), VaultError> {
        let entry: DepositEntry = env
            .storage()
            .persistent()
            .get(&DataKey::Deposit(deposit_id.clone()))
            .ok_or(VaultError::DepositNotFound)?;

        if env.ledger().sequence() < entry.unlock_ledger {
            return Err(VaultError::NotYetUnlocked);
        }

        recipient.require_auth();

        if recipient != entry.recipient {
            return Err(VaultError::WrongRecipient);
        }

        token::Client::new(&env, &entry.asset).transfer(
            &env.current_contract_address(),
            &recipient,
            &entry.amount,
        );

        env.storage()
            .persistent()
            .remove(&DataKey::Deposit(deposit_id.clone()));

        env.events().publish(
            (soroban_sdk::symbol_short!("claim"), deposit_id),
            (recipient, entry.amount),
        );

        // Emit metric event.
        emit_metric(
            &env,
            contract_ids::STEALTH_VAULT,
            metric_names::CLAIM_COUNT,
            1,
            soroban_sdk::vec![
                &env,
                (dimension_names::ASSET_ADDRESS, entry.asset.into_val(&env))
            ],
        );

        Ok(())
    }

    /// Refund an unclaimed deposit to its depositor. Callable while paused.
    ///
    /// Requires the depositor's authorisation and opens at `refund_after`.
    pub fn refund(env: Env, deposit_id: BytesN<32>) -> Result<(), VaultError> {
        let entry: DepositEntry = env
            .storage()
            .persistent()
            .get(&DataKey::Deposit(deposit_id.clone()))
            .ok_or(VaultError::DepositNotFound)?;

        if env.ledger().sequence() < entry.refund_after {
            return Err(VaultError::NotYetRefundable);
        }

        entry.sender.require_auth();

        Self::settle_refund(&env, deposit_id, entry);

        Ok(())
    }

    /// Refund an unclaimed deposit to its depositor without the depositor's
    /// signature. Callable while paused.
    ///
    /// Opens one grace period after `refund_after`, so a depositor who has lost
    /// access to their key cannot strand funds in the vault forever. Funds
    /// always go to the recorded depositor — `caller` only pays the fee and is
    /// authorised so the invocation is attributable.
    pub fn refund_permissionless(
        env: Env,
        caller: Address,
        deposit_id: BytesN<32>,
    ) -> Result<(), VaultError> {
        caller.require_auth();

        let entry: DepositEntry = env
            .storage()
            .persistent()
            .get(&DataKey::Deposit(deposit_id.clone()))
            .ok_or(VaultError::DepositNotFound)?;

        let grace = Self::grace_period_value(&env);
        if env.ledger().sequence() < entry.refund_after.saturating_add(grace) {
            return Err(VaultError::NotYetPermissionless);
        }

        Self::settle_refund(&env, deposit_id, entry);

        Ok(())
    }

    /// Internal: pay a deposit back to its depositor, clear it, and report.
    ///
    /// Shared by `refund` and `refund_permissionless` so both paths emit the
    /// same `refund` event and `refund_count` metric that indexers already read.
    fn settle_refund(env: &Env, deposit_id: BytesN<32>, entry: DepositEntry) {
        token::Client::new(env, &entry.asset).transfer(
            &env.current_contract_address(),
            &entry.sender,
            &entry.amount,
        );

        env.storage()
            .persistent()
            .remove(&DataKey::Deposit(deposit_id.clone()));

        env.events().publish(
            (soroban_sdk::symbol_short!("refund"), deposit_id),
            (entry.sender, entry.amount),
        );

        // Emit metric event.
        emit_metric(
            env,
            contract_ids::STEALTH_VAULT,
            metric_names::REFUND_COUNT,
            1,
            soroban_sdk::vec![
                env,
                (dimension_names::ASSET_ADDRESS, entry.asset.into_val(env))
            ],
        );
    }

    /// Internal: require the contract is not paused.
    fn require_not_paused(env: &Env) -> Result<(), VaultError> {
        if env
            .storage()
            .instance()
            .get::<_, bool>(&DataKey::Paused)
            .unwrap_or(false)
        {
            return Err(VaultError::Paused);
        }
        Ok(())
    }

    /// Internal: read the configured grace period, falling back to the default
    /// for vaults deployed before the key existed.
    fn grace_period_value(env: &Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::GracePeriod)
            .unwrap_or(DEFAULT_GRACE_PERIOD)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Events, Ledger};
    use soroban_sdk::{Bytes, BytesN, Env};

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

    struct Fixture {
        env: Env,
        client: StealthVaultContractClient<'static>,
        admin: Address,
        sender: Address,
        recipient: Address,
        token_id: Address,
    }

    fn setup() -> (
        Env,
        StealthVaultContractClient<'static>,
        Address,
        Address,
        Address,
    ) {
        let f = fixture();
        (f.env, f.client, f.sender, f.recipient, f.token_id)
    }

    fn fixture() -> Fixture {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().with_mut(|li| {
            li.min_persistent_entry_ttl = 600000;
        });

        let announcer_id = env.register(MockAnnouncer, ());
        let vault_id = env.register(StealthVaultContract, ());
        let client = StealthVaultContractClient::new(&env, &vault_id);

        let admin = Address::generate(&env);
        client.init(&admin, &announcer_id);

        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);

        let token_admin = Address::generate(&env);
        let token_id = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();
        let token_admin_client = token::StellarAssetClient::new(&env, &token_id);
        token_admin_client.mint(&sender, &10000);

        Fixture {
            env,
            client,
            admin,
            sender,
            recipient,
            token_id,
        }
    }

    #[test]
    fn test_deposit_and_claim() {
        let (env, client, sender, recipient, token_id) = setup();
        let epk = BytesN::from_array(&env, &[1u8; 32]);

        let deposit_id = client.deposit(&sender, &recipient, &1000, &token_id, &100, &2000, &epk);

        env.ledger().with_mut(|li| li.sequence_number = 100);

        let token_client = token::Client::new(&env, &token_id);
        assert_eq!(token_client.balance(&recipient), 0);
        client.claim(&deposit_id, &recipient);
        assert_eq!(token_client.balance(&recipient), 1000);
    }

    #[test]
    fn test_claim_before_unlock_fails() {
        let (env, client, sender, recipient, token_id) = setup();
        let epk = BytesN::from_array(&env, &[2u8; 32]);

        let deposit_id = client.deposit(&sender, &recipient, &500, &token_id, &500, &2000, &epk);
        // sequence is 0 < 500
        let res = client.try_claim(&deposit_id, &recipient);
        assert_eq!(res, Err(Ok(VaultError::NotYetUnlocked)));
    }

    #[test]
    fn test_refund_after_window() {
        let (env, client, sender, recipient, token_id) = setup();
        let epk = BytesN::from_array(&env, &[3u8; 32]);
        let token_client = token::Client::new(&env, &token_id);

        let deposit_id = client.deposit(&sender, &recipient, &800, &token_id, &100, &2000, &epk);
        let balance_after_deposit = token_client.balance(&sender);

        env.ledger().with_mut(|li| li.sequence_number = 2000);
        client.refund(&deposit_id);
        assert_eq!(token_client.balance(&sender), balance_after_deposit + 800);
    }

    #[test]
    fn test_refund_before_window_fails() {
        let (env, client, sender, recipient, token_id) = setup();
        let epk = BytesN::from_array(&env, &[4u8; 32]);

        let deposit_id = client.deposit(&sender, &recipient, &300, &token_id, &100, &2000, &epk);
        // sequence = 0 < 2000
        let res = client.try_refund(&deposit_id);
        assert_eq!(res, Err(Ok(VaultError::NotYetRefundable)));
    }

    #[test]
    fn test_double_claim_fails() {
        let (env, client, sender, recipient, token_id) = setup();
        let epk = BytesN::from_array(&env, &[5u8; 32]);

        let deposit_id = client.deposit(&sender, &recipient, &200, &token_id, &0, &2000, &epk);
        client.claim(&deposit_id, &recipient);
        let res = client.try_claim(&deposit_id, &recipient);
        assert_eq!(res, Err(Ok(VaultError::DepositNotFound)));
    }

    #[test]
    fn test_wrong_recipient_cannot_claim() {
        let (env, client, sender, recipient, token_id) = setup();
        let epk = BytesN::from_array(&env, &[6u8; 32]);
        let wrong = Address::generate(&env);

        let deposit_id = client.deposit(&sender, &recipient, &100, &token_id, &0, &2000, &epk);
        let res = client.try_claim(&deposit_id, &wrong);
        assert_eq!(res, Err(Ok(VaultError::WrongRecipient)));
    }

    #[test]
    fn test_deposit_validates_refund_window() {
        let (env, client, sender, recipient, token_id) = setup();
        let epk = BytesN::from_array(&env, &[7u8; 32]);

        // refund_after = unlock_ledger + GRACE_PERIOD (not strictly greater)
        let res = client.try_deposit(&sender, &recipient, &100, &token_id, &100, &1100, &epk);
        assert_eq!(res, Err(Ok(VaultError::InvalidWindow)));
    }

    #[test]
    fn test_sender_cannot_refund_early() {
        let (env, client, sender, recipient, token_id) = setup();
        let epk = BytesN::from_array(&env, &[8u8; 32]);

        let deposit_id = client.deposit(&sender, &recipient, &150, &token_id, &100, &5000, &epk);
        env.ledger().with_mut(|li| li.sequence_number = 4999);
        let res = client.try_refund(&deposit_id);
        assert_eq!(res, Err(Ok(VaultError::NotYetRefundable)));
    }

    // ============ ADMIN / INIT ============

    #[test]
    fn test_init_is_one_shot() {
        let f = fixture();
        let other = Address::generate(&f.env);
        let announcer = f.env.register(MockAnnouncer, ());
        let res = f.client.try_init(&other, &announcer);
        assert_eq!(res, Err(Ok(VaultError::AlreadyInitialized)));
    }

    #[test]
    fn test_init_records_admin_and_default_grace_period() {
        let f = fixture();
        assert_eq!(f.client.admin(), f.admin);
        assert_eq!(f.client.grace_period(), DEFAULT_GRACE_PERIOD);
        assert!(!f.client.is_paused());
    }

    #[test]
    fn test_admin_can_set_grace_period() {
        let f = fixture();
        f.client.set_grace_period(&f.admin, &50);
        assert_eq!(f.client.grace_period(), 50);

        // The tightened window is what `deposit` now validates against.
        let epk = BytesN::from_array(&f.env, &[20u8; 32]);
        f.client
            .deposit(&f.sender, &f.recipient, &100, &f.token_id, &100, &151, &epk);
    }

    #[test]
    fn test_set_grace_period_rejects_zero() {
        let f = fixture();
        let res = f.client.try_set_grace_period(&f.admin, &0);
        assert_eq!(res, Err(Ok(VaultError::InvalidGracePeriod)));
    }

    #[test]
    #[should_panic(expected = "unauthorized: only admin can set the grace period")]
    fn test_non_admin_cannot_set_grace_period() {
        let f = fixture();
        let intruder = Address::generate(&f.env);
        f.client.set_grace_period(&intruder, &50);
    }

    // ============ PAUSE ============

    #[test]
    fn test_admin_can_pause_and_unpause() {
        let f = fixture();
        assert!(!f.client.is_paused());
        f.client.pause(&f.admin);
        assert!(f.client.is_paused());
        f.client.unpause(&f.admin);
        assert!(!f.client.is_paused());
    }

    #[test]
    #[should_panic(expected = "unauthorized: only admin can pause")]
    fn test_non_admin_cannot_pause() {
        let f = fixture();
        let intruder = Address::generate(&f.env);
        f.client.pause(&intruder);
    }

    #[test]
    #[should_panic(expected = "unauthorized: only admin can unpause")]
    fn test_non_admin_cannot_unpause() {
        let f = fixture();
        f.client.pause(&f.admin);
        let intruder = Address::generate(&f.env);
        f.client.unpause(&intruder);
    }

    #[test]
    fn test_deposit_blocked_while_paused() {
        let f = fixture();
        let epk = BytesN::from_array(&f.env, &[21u8; 32]);
        f.client.pause(&f.admin);

        let res = f.client.try_deposit(
            &f.sender,
            &f.recipient,
            &100,
            &f.token_id,
            &100,
            &2000,
            &epk,
        );
        assert_eq!(res, Err(Ok(VaultError::Paused)));

        f.client.unpause(&f.admin);
        f.client.deposit(
            &f.sender,
            &f.recipient,
            &100,
            &f.token_id,
            &100,
            &2000,
            &epk,
        );
    }

    #[test]
    fn test_claim_still_works_while_paused() {
        let f = fixture();
        let epk = BytesN::from_array(&f.env, &[22u8; 32]);
        let deposit_id = f.client.deposit(
            &f.sender,
            &f.recipient,
            &700,
            &f.token_id,
            &100,
            &2000,
            &epk,
        );

        f.client.pause(&f.admin);
        f.env.ledger().with_mut(|li| li.sequence_number = 100);
        f.client.claim(&deposit_id, &f.recipient);

        let token_client = token::Client::new(&f.env, &f.token_id);
        assert_eq!(token_client.balance(&f.recipient), 700);
    }

    #[test]
    fn test_refund_still_works_while_paused() {
        let f = fixture();
        let epk = BytesN::from_array(&f.env, &[23u8; 32]);
        let token_client = token::Client::new(&f.env, &f.token_id);
        let deposit_id = f.client.deposit(
            &f.sender,
            &f.recipient,
            &600,
            &f.token_id,
            &100,
            &2000,
            &epk,
        );
        let balance_after_deposit = token_client.balance(&f.sender);

        f.client.pause(&f.admin);
        f.env.ledger().with_mut(|li| li.sequence_number = 2000);
        f.client.refund(&deposit_id);

        assert_eq!(token_client.balance(&f.sender), balance_after_deposit + 600);
    }

    // ============ PERMISSIONLESS REFUND ============

    #[test]
    fn test_permissionless_refund_after_grace() {
        let f = fixture();
        let epk = BytesN::from_array(&f.env, &[24u8; 32]);
        let token_client = token::Client::new(&f.env, &f.token_id);
        let keeper = Address::generate(&f.env);

        let deposit_id = f.client.deposit(
            &f.sender,
            &f.recipient,
            &900,
            &f.token_id,
            &100,
            &2000,
            &epk,
        );
        let balance_after_deposit = token_client.balance(&f.sender);

        f.env.ledger().with_mut(|li| li.sequence_number = 3000);
        f.client.refund_permissionless(&keeper, &deposit_id);

        // Funds go back to the depositor, never to the caller.
        assert_eq!(token_client.balance(&f.sender), balance_after_deposit + 900);
        assert_eq!(token_client.balance(&keeper), 0);
        assert_eq!(
            f.client.try_get_deposit(&deposit_id),
            Err(Ok(VaultError::DepositNotFound))
        );
    }

    #[test]
    fn test_permissionless_refund_before_grace_fails() {
        let f = fixture();
        let epk = BytesN::from_array(&f.env, &[25u8; 32]);
        let keeper = Address::generate(&f.env);

        let deposit_id = f.client.deposit(
            &f.sender,
            &f.recipient,
            &400,
            &f.token_id,
            &100,
            &2000,
            &epk,
        );

        // refund_after has passed, but refund_after + grace_period has not.
        f.env.ledger().with_mut(|li| li.sequence_number = 2999);
        let res = f.client.try_refund_permissionless(&keeper, &deposit_id);
        assert_eq!(res, Err(Ok(VaultError::NotYetPermissionless)));
    }

    #[test]
    fn test_permissionless_refund_after_claim_fails() {
        let f = fixture();
        let epk = BytesN::from_array(&f.env, &[26u8; 32]);
        let keeper = Address::generate(&f.env);

        let deposit_id = f.client.deposit(
            &f.sender,
            &f.recipient,
            &400,
            &f.token_id,
            &100,
            &2000,
            &epk,
        );

        f.env.ledger().with_mut(|li| li.sequence_number = 100);
        f.client.claim(&deposit_id, &f.recipient);

        f.env.ledger().with_mut(|li| li.sequence_number = 3000);
        let res = f.client.try_refund_permissionless(&keeper, &deposit_id);
        assert_eq!(res, Err(Ok(VaultError::DepositNotFound)));
    }

    #[test]
    fn test_permissionless_refund_works_while_paused() {
        let f = fixture();
        let epk = BytesN::from_array(&f.env, &[27u8; 32]);
        let token_client = token::Client::new(&f.env, &f.token_id);
        let keeper = Address::generate(&f.env);

        let deposit_id = f.client.deposit(
            &f.sender,
            &f.recipient,
            &250,
            &f.token_id,
            &100,
            &2000,
            &epk,
        );
        let balance_after_deposit = token_client.balance(&f.sender);

        f.client.pause(&f.admin);
        f.env.ledger().with_mut(|li| li.sequence_number = 3000);
        f.client.refund_permissionless(&keeper, &deposit_id);

        assert_eq!(token_client.balance(&f.sender), balance_after_deposit + 250);
    }

    // ============ METRIC EVENT SHAPE ============

    /// Collect the `("metric", contract, name)` events currently in the buffer
    /// as `(metric_name, value, asset_address_dimension)` triples.
    fn metric_events(env: &Env) -> soroban_sdk::Vec<(soroban_sdk::Symbol, i128, Address)> {
        let metric_topic: soroban_sdk::Val = soroban_sdk::symbol_short!("metric").into_val(env);
        let mut collected = soroban_sdk::Vec::new(env);

        for (_, topics, data) in env.events().all().iter() {
            let first: Option<soroban_sdk::Val> = topics.first();
            if first.map(|t| t.shallow_eq(&metric_topic)) != Some(true) {
                continue;
            }

            assert_eq!(
                topics.len(),
                3,
                "metric topics are (metric, contract, name)"
            );
            let emitting: soroban_sdk::Symbol = topics.get(1).unwrap().into_val(env);
            assert_eq!(emitting, contract_ids::STEALTH_VAULT);

            let metric_name: soroban_sdk::Symbol = topics.get(2).unwrap().into_val(env);
            let (value, dimensions): (
                i128,
                soroban_sdk::Vec<(soroban_sdk::Symbol, soroban_sdk::Val)>,
            ) = data.into_val(env);

            assert_eq!(dimensions.len(), 1, "expected the asset_address dimension");
            let (dimension_name, dimension_value) = dimensions.get(0).unwrap();
            assert_eq!(dimension_name, dimension_names::ASSET_ADDRESS);
            let asset: Address = dimension_value.into_val(env);

            collected.push_back((metric_name, value, asset));
        }

        collected
    }

    #[test]
    fn test_deposit_emits_metric_events() {
        let (env, client, sender, recipient, token_id) = setup();
        let epk = BytesN::from_array(&env, &[9u8; 32]);

        client.deposit(&sender, &recipient, &1000, &token_id, &100, &2000, &epk);

        assert_eq!(
            metric_events(&env),
            soroban_sdk::vec![
                &env,
                (metric_names::DEPOSIT_COUNT, 1i128, token_id.clone()),
                (metric_names::DEPOSIT_VOLUME, 1000i128, token_id),
            ]
        );
    }

    #[test]
    fn test_claim_emits_metric_event() {
        let (env, client, sender, recipient, token_id) = setup();
        let epk = BytesN::from_array(&env, &[10u8; 32]);

        let deposit_id = client.deposit(&sender, &recipient, &1000, &token_id, &100, &2000, &epk);
        env.ledger().with_mut(|li| li.sequence_number = 100);
        client.claim(&deposit_id, &recipient);

        assert_eq!(
            metric_events(&env),
            soroban_sdk::vec![&env, (metric_names::CLAIM_COUNT, 1i128, token_id)]
        );
    }

    #[test]
    fn test_refund_emits_metric_event() {
        let (env, client, sender, recipient, token_id) = setup();
        let epk = BytesN::from_array(&env, &[11u8; 32]);

        let deposit_id = client.deposit(&sender, &recipient, &800, &token_id, &100, &2000, &epk);
        env.ledger().with_mut(|li| li.sequence_number = 2000);
        client.refund(&deposit_id);

        assert_eq!(
            metric_events(&env),
            soroban_sdk::vec![&env, (metric_names::REFUND_COUNT, 1i128, token_id)]
        );
    }

    #[test]
    fn test_permissionless_refund_emits_refund_metric() {
        let f = fixture();
        let epk = BytesN::from_array(&f.env, &[12u8; 32]);
        let keeper = Address::generate(&f.env);

        let deposit_id = f.client.deposit(
            &f.sender,
            &f.recipient,
            &800,
            &f.token_id,
            &100,
            &2000,
            &epk,
        );
        f.env.ledger().with_mut(|li| li.sequence_number = 3000);
        f.client.refund_permissionless(&keeper, &deposit_id);

        assert_eq!(
            metric_events(&f.env),
            soroban_sdk::vec![&f.env, (metric_names::REFUND_COUNT, 1i128, f.token_id)]
        );
    }
}
