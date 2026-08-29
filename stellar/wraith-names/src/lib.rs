#![no_std]

extern crate alloc;

use core::convert::TryInto;

use soroban_sdk::xdr::{AccountId, PublicKey, ScAddress};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes, BytesN, Env,
    String, Vec,
};
use wraith_metrics::{contract_ids, emit_metric, metric_names};

pub mod auction;
mod multisig;

pub use auction::{Auction, AuctionConfig, AuctionError, SealedBid};
pub use multisig::{AdminRotationProposal, RotationProposal};

pub const WRAITH_NAMES_DOMAIN: &[u8] = b"wraith-names:v1";
const MIN_LABEL_LEN: usize = 3;
const MAX_NAME_LEN: usize = 32;
const MAX_SUBDOMAIN_DEPTH: usize = 1;
const BULK_LIMIT: u32 = 20;

/// Storage keys.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Maps name hash (BytesN<32>) to NameEntry.
    Name(BytesN<32>),
    /// Reverse lookup: meta-address hash (BytesN<32>) to name string.
    Reverse(BytesN<32>),
    /// Replay protection for signed on-behalf calls.
    Replay(BytesN<32>),
    /// Guardian config for a name.
    Guardians(BytesN<32>),
    /// Pending recovery proposal for a name.
    Recovery(BytesN<32>),
    /// Pause admin address.
    Admin,
    /// Whether the contract is paused.
    Paused,
    /// Protocol-level governance multisig signer set.
    MultisigSigners,
    /// Protocol-level governance multisig quorum threshold.
    MultisigThreshold,
    /// Pending protocol-level signer-rotation proposal, if any.
    PendingRotation,
    /// Pending auction-admin rotation proposal, if any.
    PendingAuctionAdminRotation,
}

/// A registered name entry.
#[contracttype]
#[derive(Clone)]
pub struct NameEntry {
    pub name: String,
    pub stealth_meta_address: Bytes,
    pub owner: Address,
    /// For a subdomain (`sub.parent`), the name hash of the parent label.
    /// `None` for a flat top-level name. Existing flat names register with
    /// `None`, so prior behaviour is preserved.
    pub parent: Option<BytesN<32>>,
}

/// Guardian configuration for a name.
#[contracttype]
#[derive(Clone)]
pub struct GuardianConfig {
    pub guardians: Vec<Address>,
    pub threshold: u32,
}

/// A pending recovery proposal.
#[contracttype]
#[derive(Clone)]
pub struct RecoveryProposal {
    pub new_owner: Address,
    pub new_meta_address: Bytes,
    pub proposed_at: u32,
    pub approvals: Vec<Address>,
}

/// Errors.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum NamesError {
    NameTaken = 1,
    NameTooShort = 2,
    NameTooLong = 3,
    InvalidNameCharacter = 4,
    InvalidMetaAddress = 5,
    NameNotFound = 6,
    NotOwner = 7,
    SignatureExpired = 8,
    SignatureReplay = 9,
    InvalidSigner = 10,
    NotGuardian = 11,
    NoProposal = 12,
    ProposalAlreadyExists = 13,
    AlreadyApproved = 14,
    DelayNotElapsed = 15,
    ThresholdNotMet = 16,
    TooManyGuardians = 17,
    InvalidThreshold = 18,
    InvalidExtendLedger = 19,
    ParentNotFound = 20,
    /// The contract is paused.
    Paused = 32,
    /// The protocol-level governance multisig has not been initialised.
    MultisigNotInitialized = 21,
    /// The protocol-level governance multisig has already been initialised.
    MultisigAlreadyInitialized = 22,
    /// The caller is not a current protocol-level governance signer.
    NotSigner = 23,
    /// A signer-rotation proposal is already pending.
    RotationAlreadyPending = 24,
    /// No signer-rotation proposal is pending.
    NoPendingRotation = 25,
    /// The caller has already approved the pending rotation.
    AlreadyApprovedRotation = 26,
    /// The pending rotation has not collected enough approvals yet.
    QuorumNotMet = 27,
    /// The rotation timelock has not elapsed yet.
    TimelockNotElapsed = 28,
    NameTooDeep = 29,
    BulkLimitExceeded = 30,
    /// The name is premium (<= 4 chars) and the auction window is active, so
    /// it can only be obtained through the sealed-bid auction.
    PremiumAuctionRequired = 31,
    /// The auction subsystem has not been initialised, so there is no auction
    /// admin to rotate.
    AuctionsNotInitialized = 1600,
    /// An auction has a revealed winner and has not settled yet, so the
    /// auction admin cannot be rotated out from under it.
    AuctionInProgress = 1601,
}

const TTL_THRESHOLD: u32 = 17280; // ~1 day
const TTL_EXTEND_TO: u32 = 518400; // ~30 days

#[contract]
pub struct WraithNamesContract;

#[contractimpl]
impl WraithNamesContract {
    /// Initialise the contract by storing the pause admin.
    ///
    /// Must be called before `pause` / `unpause`. Idempotent: calling
    /// more than once is a no-op (the first admin sticks).
    pub fn init(env: Env, admin: Address) -> Result<(), NamesError> {
        if !env.storage().instance().has(&DataKey::Admin) {
            env.storage().instance().set(&DataKey::Admin, &admin);
            Self::extend_instance_ttl(&env);
        }
        Ok(())
    }

    /// Pause the contract — admin only.
    /// Prevents all registrations, updates, releases and TTL extensions
    /// while paused. Lookups (`resolve`, `name_of`) remain available.
    pub fn pause(env: Env, caller: Address) -> Result<(), NamesError> {
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
    pub fn unpause(env: Env, caller: Address) -> Result<(), NamesError> {
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

    /// Internal: require the contract is not paused.
    fn require_not_paused(env: &Env) -> Result<(), NamesError> {
        if env
            .storage()
            .instance()
            .get::<_, bool>(&DataKey::Paused)
            .unwrap_or(false)
        {
            return Err(NamesError::Paused);
        }
        Ok(())
    }

    /// Internal: extend instance TTL.
    fn extend_instance_ttl(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
    }

    /// Register a name mapped to a stealth meta-address.
    pub fn register(
        env: Env,
        owner: Address,
        name: String,
        stealth_meta_address: Bytes,
    ) -> Result<(), NamesError> {
        Self::require_not_paused(&env)?;
        owner.require_auth();
        Self::register_internal(&env, owner, name, stealth_meta_address, false)
    }

    /// Register a name on behalf of an owner using a signed authorization.
    pub fn register_on_behalf(
        env: Env,
        owner: Address,
        name: String,
        stealth_meta_address: Bytes,
        signature: BytesN<64>,
        expiry: u64,
    ) -> Result<(), NamesError> {
        Self::require_not_paused(&env)?;
        let replay_key = Self::verify_on_behalf_authorization(
            &env,
            &owner,
            b"wraith-names:register",
            &name,
            &stealth_meta_address,
            &signature,
            expiry,
        )?;
        Self::register_internal(&env, owner, name, stealth_meta_address, false)?;
        // Persist replay protection to prevent signature reuse
        env.storage()
            .persistent()
            .set(&DataKey::Replay(replay_key), &true);
        Ok(())
    }

    /// Update the meta-address for an existing name.
    /// Only the current owner can update.
    pub fn update(
        env: Env,
        owner: Address,
        name: String,
        new_meta_address: Bytes,
    ) -> Result<(), NamesError> {
        Self::require_not_paused(&env)?;
        owner.require_auth();
        Self::update_internal(&env, owner, name, new_meta_address)
    }

    /// Update a name on behalf of an owner using a signed authorization.
    pub fn update_on_behalf(
        env: Env,
        owner: Address,
        name: String,
        new_meta_address: Bytes,
        signature: BytesN<64>,
        expiry: u64,
    ) -> Result<(), NamesError> {
        Self::require_not_paused(&env)?;
        let replay_key = Self::verify_on_behalf_authorization(
            &env,
            &owner,
            b"wraith-names:update",
            &name,
            &new_meta_address,
            &signature,
            expiry,
        )?;
        Self::update_internal(&env, owner, name, new_meta_address)?;
        env.storage()
            .persistent()
            .set(&DataKey::Replay(replay_key), &true);
        Ok(())
    }

    /// Release a name, making it available again.
    pub fn release(env: Env, owner: Address, name: String) -> Result<(), NamesError> {
        Self::require_not_paused(&env)?;
        owner.require_auth();
        Self::release_internal(&env, owner, name)
    }

    /// Release a name on behalf of an owner using a signed authorization.
    pub fn release_on_behalf(
        env: Env,
        owner: Address,
        name: String,
        signature: BytesN<64>,
        expiry: u64,
    ) -> Result<(), NamesError> {
        Self::require_not_paused(&env)?;
        let empty_meta = Bytes::new(&env);
        let replay_key = Self::verify_on_behalf_authorization(
            &env,
            &owner,
            b"wraith-names:release",
            &name,
            &empty_meta,
            &signature,
            expiry,
        )?;
        Self::release_internal(&env, owner, name)?;
        env.storage()
            .persistent()
            .set(&DataKey::Replay(replay_key), &true);
        Ok(())
    }

    /// Register multiple names in a single atomic transaction.
    ///
    /// All names must be valid and not already taken. If any name fails,
    /// the entire operation reverts.
    pub fn bulk_register(
        env: Env,
        owner: Address,
        names: Vec<String>,
        meta_addresses: Vec<Bytes>,
    ) -> Result<(), NamesError> {
        owner.require_auth();

        let count = names.len();
        if count > BULK_LIMIT {
            return Err(NamesError::BulkLimitExceeded);
        }
        if meta_addresses.len() != count {
            return Err(NamesError::InvalidMetaAddress);
        }

        for i in 0..count {
            let name = names.get(i).unwrap();
            let meta = meta_addresses.get(i).unwrap();
            // Validate upfront so we fail early
            Self::validate_name(&env, &name)?;
            if meta.len() != 64 {
                return Err(NamesError::InvalidMetaAddress);
            }
            let name_hash = Self::hash_name(&env, &name);
            if env.storage().persistent().has(&DataKey::Name(name_hash)) {
                return Err(NamesError::NameTaken);
            }
        }

        let mut registered: Vec<BytesN<32>> = Vec::new(&env);
        for i in 0..count {
            let name = names.get(i).unwrap();
            let meta = meta_addresses.get(i).unwrap();
            Self::register_internal(&env, owner.clone(), name.clone(), meta, false)?;
            let name_hash = Self::hash_name(&env, &name);
            registered.push_back(name_hash);
        }

        env.events()
            .publish((symbol_short!("bulk_reg"), owner.clone()), registered);

        Ok(())
    }

    /// Renew (extend TTL for) multiple names in a single atomic transaction.
    ///
    /// All names must exist. If any name is not found, the entire operation
    /// reverts.
    pub fn bulk_renew(
        env: Env,
        names: Vec<String>,
        extend_to_ledger: u32,
    ) -> Result<(), NamesError> {
        let count = names.len();
        if count > BULK_LIMIT {
            return Err(NamesError::BulkLimitExceeded);
        }

        let current_ledger = env.ledger().sequence();
        if extend_to_ledger <= current_ledger {
            return Err(NamesError::InvalidExtendLedger);
        }

        for i in 0..count {
            let name = names.get(i).unwrap();
            let name_hash = Self::hash_name(&env, &name);
            let name_key = DataKey::Name(name_hash.clone());
            if !env.storage().persistent().has(&name_key) {
                return Err(NamesError::NameNotFound);
            }
        }

        for i in 0..count {
            let name = names.get(i).unwrap();
            let name_hash = Self::hash_name(&env, &name);
            let name_key = DataKey::Name(name_hash.clone());

            let entry: NameEntry = env.storage().persistent().get(&name_key).unwrap();
            let meta_hash = BytesN::from_array(
                &env,
                &env.crypto().sha256(&entry.stealth_meta_address).to_array(),
            );
            let reverse_key = DataKey::Reverse(meta_hash);

            env.storage()
                .persistent()
                .extend_ttl(&name_key, current_ledger, extend_to_ledger);
            env.storage()
                .persistent()
                .extend_ttl(&reverse_key, current_ledger, extend_to_ledger);

            env.events()
                .publish((symbol_short!("extend"), name_hash), extend_to_ledger);
        }

        let mut name_hashes: Vec<BytesN<32>> = Vec::new(&env);
        for ni in 0..count {
            let n = names.get(ni).unwrap();
            name_hashes.push_back(Self::hash_name(&env, &n));
        }
        env.events().publish(
            (symbol_short!("blk_renew"),),
            (name_hashes, extend_to_ledger),
        );

        // Emit metric event — one renewal per name in the batch.
        emit_metric(
            &env,
            contract_ids::WRAITH_NAMES,
            metric_names::RENEW_COUNT,
            count as i128,
            Vec::new(&env),
        );

        Ok(())
    }

    fn owner_public_key(env: &Env, owner: &Address) -> Result<BytesN<32>, NamesError> {
        let sc_address: ScAddress = owner.try_into().map_err(|_| NamesError::InvalidSigner)?;

        match sc_address {
            ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(public_key))) => {
                let public_key_bytes: [u8; 32] = public_key
                    .as_slice()
                    .try_into()
                    .map_err(|_| NamesError::InvalidSigner)?;
                Ok(BytesN::from_array(env, &public_key_bytes))
            }
            _ => Err(NamesError::InvalidSigner),
        }
    }

    fn register_internal(
        env: &Env,
        owner: Address,
        name: String,
        stealth_meta_address: Bytes,
        via_auction: bool,
    ) -> Result<(), NamesError> {
        Self::validate_name(env, &name)?;
        if stealth_meta_address.len() != 64 {
            return Err(NamesError::InvalidMetaAddress);
        }

        // Parse subdomain
        let len = name.len() as usize;
        let mut name_buf = [0u8; MAX_NAME_LEN];
        name.copy_into_slice(&mut name_buf[..len]);

        let mut dot_count: u32 = 0;
        let mut last_dot: usize = 0;
        for i in 0..len {
            if name_buf[i] == b'.' {
                dot_count += 1;
                last_dot = i;
            }
        }

        if dot_count > MAX_SUBDOMAIN_DEPTH as u32 {
            return Err(NamesError::NameTooDeep);
        }

        let parent_hash = if dot_count > 0 {
            let mut parent_buf = [0u8; MAX_NAME_LEN];
            let parent_len = len - last_dot - 1;
            for i in 0..parent_len {
                parent_buf[i] = name_buf[last_dot + 1 + i];
            }
            let parent_str = String::from_str(
                env,
                core::str::from_utf8(&parent_buf[..parent_len]).unwrap(),
            );
            let ph = Self::hash_name(env, &parent_str);
            Some(ph)
        } else {
            None
        };

        // During the 90-day premium window, top-level names of 4 characters
        // or fewer can only be obtained through the sealed-bid auction.
        if !via_auction && parent_hash.is_none() && auction::premium_block_active(env, len) {
            return Err(NamesError::PremiumAuctionRequired);
        }

        let name_hash = Self::hash_name(env, &name);
        let name_key = DataKey::Name(name_hash.clone());

        // Check not taken
        if env.storage().persistent().has(&name_key) {
            return Err(NamesError::NameTaken);
        }

        // Subdomains require an existing parent and that `owner` is the parent
        // owner.
        if let Some(ref ph) = parent_hash {
            let parent: NameEntry = env
                .storage()
                .persistent()
                .get(&DataKey::Name(ph.clone()))
                .ok_or(NamesError::ParentNotFound)?;
            if parent.owner != owner {
                return Err(NamesError::NotOwner);
            }
        }

        let entry = NameEntry {
            name: name.clone(),
            stealth_meta_address: stealth_meta_address.clone(),
            owner,
            parent: parent_hash.clone(),
        };

        env.storage().persistent().set(&name_key, &entry);

        let meta_hash =
            BytesN::from_array(env, &env.crypto().sha256(&stealth_meta_address).to_array());
        let reverse_key = DataKey::Reverse(meta_hash);
        env.storage().persistent().set(&reverse_key, &name_hash);

        // Extend TTLs
        Self::extend_ttls(&env, &name_key, Some(&reverse_key));

        env.events().publish(
            (symbol_short!("register"), name_hash),
            (name, stealth_meta_address),
        );

        // Emit metric event.
        emit_metric(
            env,
            contract_ids::WRAITH_NAMES,
            metric_names::REGISTER_COUNT,
            1,
            Vec::new(env),
        );

        Ok(())
    }

    fn update_internal(
        env: &Env,
        owner: Address,
        name: String,
        new_meta_address: Bytes,
    ) -> Result<(), NamesError> {
        if new_meta_address.len() != 64 {
            return Err(NamesError::InvalidMetaAddress);
        }

        let name_hash = Self::hash_name(env, &name);
        let name_key = DataKey::Name(name_hash.clone());

        let entry: NameEntry = env
            .storage()
            .persistent()
            .get(&name_key)
            .ok_or(NamesError::NameNotFound)?;

        Self::require_manager(&env, &owner, &entry)?;

        let old_meta_hash = BytesN::from_array(
            env,
            &env.crypto().sha256(&entry.stealth_meta_address).to_array(),
        );
        env.storage()
            .persistent()
            .remove(&DataKey::Reverse(old_meta_hash));

        let new_entry = NameEntry {
            name: name.clone(),
            stealth_meta_address: new_meta_address.clone(),
            owner,
            parent: entry.parent,
        };
        env.storage().persistent().set(&name_key, &new_entry);

        let new_meta_hash =
            BytesN::from_array(env, &env.crypto().sha256(&new_meta_address).to_array());
        let reverse_key = DataKey::Reverse(new_meta_hash);
        env.storage().persistent().set(&reverse_key, &name_hash);

        // Extend TTLs
        Self::extend_ttls(&env, &name_key, Some(&reverse_key));

        env.events().publish(
            (symbol_short!("update"), name_hash),
            (name, new_meta_address),
        );

        Ok(())
    }

    fn release_internal(env: &Env, owner: Address, name: String) -> Result<(), NamesError> {
        let name_hash = Self::hash_name(env, &name);
        let name_key = DataKey::Name(name_hash.clone());

        let entry: NameEntry = env
            .storage()
            .persistent()
            .get(&name_key)
            .ok_or(NamesError::NameNotFound)?;

        Self::require_manager(&env, &owner, &entry)?;

        let meta_hash = BytesN::from_array(
            env,
            &env.crypto().sha256(&entry.stealth_meta_address).to_array(),
        );
        env.storage()
            .persistent()
            .remove(&DataKey::Reverse(meta_hash));

        // Remove name
        env.storage().persistent().remove(&name_key);

        env.events()
            .publish((symbol_short!("release"), name_hash), name);

        // Emit metric event.
        emit_metric(
            env,
            contract_ids::WRAITH_NAMES,
            metric_names::RELEASE_COUNT,
            1,
            Vec::new(env),
        );

        Ok(())
    }

    fn require_manager(env: &Env, caller: &Address, entry: &NameEntry) -> Result<(), NamesError> {
        if entry.owner == *caller {
            return Ok(());
        }
        // Check if caller is the parent owner
        if let Some(ref ph) = entry.parent {
            if let Some(parent_entry) = env
                .storage()
                .persistent()
                .get::<_, NameEntry>(&DataKey::Name(ph.clone()))
            {
                if parent_entry.owner == *caller {
                    return Ok(());
                }
            }
        }
        Err(NamesError::NotOwner)
    }

    fn verify_on_behalf_authorization(
        env: &Env,
        owner: &Address,
        operation: &[u8],
        name: &String,
        stealth_meta_address: &Bytes,
        signature: &BytesN<64>,
        expiry: u64,
    ) -> Result<BytesN<32>, NamesError> {
        let current_sequence = u64::from(env.ledger().sequence());
        if current_sequence >= expiry {
            return Err(NamesError::SignatureExpired);
        }

        let public_key = Self::owner_public_key(env, owner)?;
        let message =
            Self::authorization_message(env, operation, name, stealth_meta_address, expiry);
        let message_hash = env.crypto().sha256(&message);

        let replay_key: BytesN<32> = message_hash.clone().into();

        if env
            .storage()
            .persistent()
            .has(&DataKey::Replay(replay_key.clone()))
        {
            return Err(NamesError::SignatureReplay);
        }

        let digest = Bytes::from(message_hash.clone());
        env.crypto().ed25519_verify(&public_key, &digest, signature);

        Ok(replay_key)
    }

    /// Resolve a name to its stealth meta-address.
    ///
    /// For a subdomain (`payments.alice`) resolution walks to the parent
    /// (`alice`): if the parent no longer exists the subdomain does not resolve.
    pub fn resolve(env: Env, name: String) -> Result<Bytes, NamesError> {
        let name_hash = Self::hash_name(&env, &name);
        let name_key = DataKey::Name(name_hash);
        let entry: NameEntry = match env.storage().persistent().get(&name_key) {
            Some(entry) => entry,
            None => {
                // Emit metric event. Published before the error is returned, so
                // it is captured whenever the enclosing transaction is applied.
                emit_metric(
                    &env,
                    contract_ids::WRAITH_NAMES,
                    metric_names::RESOLVE_MISS_COUNT,
                    1,
                    Vec::new(&env),
                );
                return Err(NamesError::NameNotFound);
            }
        };

        Self::extend_ttls(&env, &name_key, None);

        // Emit metric event.
        emit_metric(
            &env,
            contract_ids::WRAITH_NAMES,
            metric_names::RESOLVE_HIT_COUNT,
            1,
            Vec::new(&env),
        );

        Ok(entry.stealth_meta_address)
    }

    /// Reverse lookup: find the name for a given stealth meta-address.
    pub fn name_of(env: Env, stealth_meta_address: Bytes) -> Result<String, NamesError> {
        let meta_hash =
            BytesN::from_array(&env, &env.crypto().sha256(&stealth_meta_address).to_array());
        let reverse_key = DataKey::Reverse(meta_hash);
        let name_hash: BytesN<32> = env
            .storage()
            .persistent()
            .get(&reverse_key)
            .ok_or(NamesError::NameNotFound)?;
        let name_key = DataKey::Name(name_hash);
        let entry: NameEntry = env
            .storage()
            .persistent()
            .get(&name_key)
            .ok_or(NamesError::NameNotFound)?;

        Self::extend_ttls(&env, &name_key, Some(&reverse_key));

        Ok(entry.name)
    }

    /// Extend TTL for persistent storage entries only.
    /// Extend the TTL of a registered name to a future ledger.
    /// This is a permissionless function that anyone can call.
    /// Idempotent: calling twice in the same ledger has no additional effect.
    pub fn extend_name_ttl(
        env: Env,
        name: String,
        extend_to_ledger: u32,
    ) -> Result<(), NamesError> {
        Self::require_not_paused(&env)?;
        // Validate that extend_to_ledger is in the future
        let current_ledger = env.ledger().sequence();
        if extend_to_ledger <= current_ledger {
            return Err(NamesError::InvalidExtendLedger);
        }

        let name_hash = Self::hash_name(&env, &name);
        let name_key = DataKey::Name(name_hash.clone());

        // Check if name exists
        let entry: NameEntry = env
            .storage()
            .persistent()
            .get(&name_key)
            .ok_or(NamesError::NameNotFound)?;

        // Get the meta-address hash for reverse key
        let meta_hash = BytesN::from_array(
            &env,
            &env.crypto().sha256(&entry.stealth_meta_address).to_array(),
        );
        let reverse_key = DataKey::Reverse(meta_hash);

        // Extend TTLs to the specified ledger
        env.storage()
            .persistent()
            .extend_ttl(&name_key, current_ledger, extend_to_ledger);
        env.storage()
            .persistent()
            .extend_ttl(&reverse_key, current_ledger, extend_to_ledger);
        env.storage()
            .instance()
            .extend_ttl(current_ledger, extend_to_ledger);

        // Emit extend event for observability
        env.events()
            .publish((symbol_short!("extend"), name_hash), extend_to_ledger);

        // Emit metric event.
        emit_metric(
            &env,
            contract_ids::WRAITH_NAMES,
            metric_names::RENEW_COUNT,
            1,
            Vec::new(&env),
        );

        Ok(())
    }

    /// Private helper to extend TTLs for both the persistent entry and the contract instance.
    fn extend_ttls(env: &Env, name_key: &DataKey, reverse_key: Option<&DataKey>) {
        env.storage()
            .persistent()
            .extend_ttl(name_key, TTL_THRESHOLD, TTL_EXTEND_TO);
        if let Some(r_key) = reverse_key {
            env.storage()
                .persistent()
                .extend_ttl(r_key, TTL_THRESHOLD, TTL_EXTEND_TO);
        }
    }

    /// Hash a name string to BytesN<32> for use as storage key.
    fn hash_name(env: &Env, name: &String) -> BytesN<32> {
        let len = name.len() as usize;
        let mut buf = [0u8; MAX_NAME_LEN];
        if len > 0 {
            name.copy_into_slice(&mut buf[..len]);
        }
        let name_bytes = Bytes::from_slice(env, &buf[..len]);
        BytesN::from_array(env, &env.crypto().sha256(&name_bytes).to_array())
    }

    fn authorization_message(
        env: &Env,
        operation: &[u8],
        name: &String,
        stealth_meta_address: &Bytes,
        expiry: u64,
    ) -> Bytes {
        let mut message = Bytes::from_slice(env, WRAITH_NAMES_DOMAIN);
        message.extend_from_slice(operation);
        let len = name.len() as usize;
        let mut name_buf = [0u8; 64];
        if len > 64 {
            name.copy_into_slice(&mut name_buf);
        } else {
            name.copy_into_slice(&mut name_buf[..len]);
        }
        let actual_len = core::cmp::min(len, 64);
        let name_bytes = Bytes::from_slice(env, &name_buf[..actual_len]);
        message.append(&name_bytes);
        message.append(stealth_meta_address);
        message.extend_from_slice(&expiry.to_be_bytes());
        message
    }

    /// Validate name: 3-32 chars, lowercase alphanumeric only.
    fn validate_name(_env: &Env, name: &String) -> Result<(), NamesError> {
        let len = name.len() as usize;

        // The full name (including possible subdomain prefix) must be within limits
        if len < MIN_LABEL_LEN {
            return Err(NamesError::NameTooShort);
        }
        if len > MAX_NAME_LEN {
            return Err(NamesError::NameTooLong);
        }

        let mut name_buf = [0u8; MAX_NAME_LEN];
        name.copy_into_slice(&mut name_buf[..len]);

        for i in 0..len {
            let c = name_buf[i];
            if c == b'.' {
                continue;
            }
            if !(c >= b'a' && c <= b'z') && !(c >= b'0' && c <= b'9') {
                return Err(NamesError::InvalidNameCharacter);
            }
        }

        Ok(())
    }

    /// One-time setup of the protocol-level governance signer set used to
    /// authorise signer rotations.
    pub fn init_multisig(
        env: Env,
        signers: Vec<Address>,
        threshold: u32,
    ) -> Result<(), NamesError> {
        multisig::init(&env, signers, threshold)
    }

    /// Current protocol-level governance signer set.
    pub fn signers(env: Env) -> Vec<Address> {
        multisig::signers(&env)
    }

    /// Current protocol-level governance quorum threshold.
    pub fn threshold(env: Env) -> u32 {
        multisig::threshold(&env)
    }

    /// The pending signer-rotation proposal, if any.
    pub fn pending_rotation(env: Env) -> Option<RotationProposal> {
        multisig::pending_rotation(&env)
    }

    /// Propose a new signer set + threshold behind the rotation timelock.
    /// `caller` must be a current signer; the proposal is auto-approved by
    /// `caller`. Rejects thresholds that could never reach quorum.
    pub fn propose_rotate_signers(
        env: Env,
        caller: Address,
        new_signers: Vec<Address>,
        new_threshold: u32,
    ) -> Result<(), NamesError> {
        multisig::propose_rotate_signers(&env, caller, new_signers, new_threshold)
    }

    /// Approve the pending signer-rotation proposal.
    pub fn approve_rotate_signers(env: Env, caller: Address) -> Result<(), NamesError> {
        multisig::approve_rotate_signers(&env, caller)
    }

    /// Execute the pending rotation once quorum is met and the timelock has
    /// elapsed. Emits `SignersRotated`.
    pub fn execute_rotate_signers(env: Env, caller: Address) -> Result<(), NamesError> {
        multisig::execute_rotate_signers(&env, caller)
    }

    /// Cancel the pending rotation, clearing all of its state.
    pub fn cancel_rotate_signers(env: Env, caller: Address) -> Result<(), NamesError> {
        multisig::cancel_rotate_signers(&env, caller)
    }

    // ── auction-admin rotation ───────────────────────────────────────────────

    /// The pending auction-admin rotation proposal, if any.
    pub fn pending_auction_admin_rotation(env: Env) -> Option<AdminRotationProposal> {
        multisig::pending_auction_admin_rotation(&env)
    }

    /// Propose a new premium-auction admin behind the 7-day rotation
    /// timelock, gated by the same governance signer set as
    /// `propose_rotate_signers`. `caller` must be a current signer; the
    /// proposal is auto-approved by `caller`.
    pub fn propose_rotate_auction_admin(
        env: Env,
        caller: Address,
        new_admin: Address,
    ) -> Result<(), NamesError> {
        multisig::propose_rotate_auction_admin(&env, caller, new_admin)
    }

    /// Approve the pending auction-admin rotation proposal.
    pub fn approve_rotate_auction_admin(env: Env, caller: Address) -> Result<(), NamesError> {
        multisig::approve_rotate_auction_admin(&env, caller)
    }

    /// Execute the pending auction-admin rotation once quorum is met and the
    /// timelock has elapsed. Emits `AuctionAdminRotated(old, new)`.
    ///
    /// Fails with `AuctionInProgress` while any auction is in its reveal or
    /// settle phase, leaving the proposal intact so it can be retried after
    /// settlement.
    pub fn execute_rotate_auction_admin(env: Env, caller: Address) -> Result<(), NamesError> {
        multisig::execute_rotate_auction_admin(&env, caller)
    }

    /// Cancel the pending auction-admin rotation, clearing all of its state.
    pub fn cancel_rotate_auction_admin(env: Env, caller: Address) -> Result<(), NamesError> {
        multisig::cancel_rotate_auction_admin(&env, caller)
    }

    /// Auctions with a revealed winner that have not settled yet. While this
    /// is non-zero, `execute_rotate_auction_admin` is blocked.
    pub fn auctions_pending_settlement(env: Env) -> u32 {
        auction::pending_settlements(&env)
    }

    // ── premium name auctions ────────────────────────────────────────────────

    /// One-time initialization of the premium-name auction system.
    ///
    /// `admin` operates settlements per the runbook, `treasury` receives
    /// winning bids, `token` is the payment asset (native XLM SAC on mainnet),
    /// `reserve_price` is the minimum bid, and `commit_secs` / `reveal_secs`
    /// are the phase durations for each auction. The 90-day premium window
    /// starts at the ledger timestamp of this call.
    pub fn init_auctions(
        env: Env,
        admin: Address,
        treasury: Address,
        token: Address,
        reserve_price: i128,
        commit_secs: u64,
        reveal_secs: u64,
    ) -> Result<(), AuctionError> {
        auction::init(
            &env,
            admin,
            treasury,
            token,
            reserve_price,
            commit_secs,
            reveal_secs,
        )
    }

    /// Start a sealed-bid auction for a premium name (<= 4 chars, top-level).
    /// Permissionless: anyone may open the auction for an eligible name.
    pub fn start_auction(env: Env, name: String) -> Result<(), AuctionError> {
        Self::validate_name(&env, &name).map_err(|_| AuctionError::NotPremiumName)?;

        let len = name.len() as usize;
        if len > auction::PREMIUM_NAME_MAX_LEN {
            return Err(AuctionError::NotPremiumName);
        }
        // Only top-level names are auctioned; subdomains are gated by parent
        // ownership instead.
        let mut buf = [0u8; MAX_NAME_LEN];
        name.copy_into_slice(&mut buf[..len]);
        for i in 0..len {
            if buf[i] == b'.' {
                return Err(AuctionError::NotPremiumName);
            }
        }

        let name_hash = Self::hash_name(&env, &name);
        if env
            .storage()
            .persistent()
            .has(&DataKey::Name(name_hash.clone()))
        {
            return Err(AuctionError::NameAlreadyRegistered);
        }
        auction::start(&env, name_hash, name)
    }

    /// Commit a sealed bid. `commitment` hides the bid amount; `deposit` is
    /// transferred to the contract and must cover the bid revealed later.
    pub fn commit_bid(
        env: Env,
        bidder: Address,
        name: String,
        commitment: BytesN<32>,
        deposit: i128,
    ) -> Result<(), AuctionError> {
        let name_hash = Self::hash_name(&env, &name);
        auction::commit(&env, bidder, name_hash, commitment, deposit)
    }

    /// Reveal a previously committed bid by disclosing the amount and salt.
    pub fn reveal_bid(
        env: Env,
        bidder: Address,
        name: String,
        amount: i128,
        salt: BytesN<32>,
    ) -> Result<(), AuctionError> {
        let name_hash = Self::hash_name(&env, &name);
        auction::reveal(&env, bidder, name_hash, amount, salt)
    }

    /// Settle an auction after the reveal phase: pays the winning bid to the
    /// treasury and refunds the winner's excess deposit. Permissionless so
    /// funds can never be trapped, operated by the admin per the runbook.
    pub fn settle_auction(env: Env, name: String) -> Result<(), AuctionError> {
        let name_hash = Self::hash_name(&env, &name);
        auction::settle(&env, name_hash)
    }

    /// Withdraw a losing (or unrevealed) bid deposit in full.
    pub fn withdraw_bid(env: Env, bidder: Address, name: String) -> Result<(), AuctionError> {
        let name_hash = Self::hash_name(&env, &name);
        auction::withdraw(&env, bidder, name_hash)
    }

    /// Claim a won auction: registers the name to the winner with their
    /// stealth meta-address.
    pub fn claim_name(
        env: Env,
        winner: Address,
        name: String,
        stealth_meta_address: Bytes,
    ) -> Result<(), AuctionError> {
        winner.require_auth();
        let name_hash = Self::hash_name(&env, &name);
        auction::verify_claim(&env, &winner, &name_hash)?;
        Self::register_internal(&env, winner, name, stealth_meta_address, true).map_err(|e| match e
        {
            NamesError::NameTaken => AuctionError::NameAlreadyRegistered,
            NamesError::InvalidMetaAddress => AuctionError::InvalidMetaAddress,
            _ => AuctionError::RegistrationFailed,
        })
    }

    /// Read the auction state for a name, if any.
    pub fn get_auction(env: Env, name: String) -> Option<Auction> {
        let name_hash = Self::hash_name(&env, &name);
        auction::load(&env, &name_hash)
    }

    /// Read the auction configuration, if initialized.
    pub fn auction_config(env: Env) -> Option<AuctionConfig> {
        auction::config(&env)
    }

    /// Compute the sealed-bid commitment for the given parameters.
    ///
    /// Intended for off-chain use (simulation only): calling this in a real
    /// transaction would leak the bid amount.
    pub fn compute_commitment(
        env: Env,
        name: String,
        bidder: Address,
        amount: i128,
        salt: BytesN<32>,
    ) -> BytesN<32> {
        auction::compute_commitment(&env, &name, &bidder, amount, &salt)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloc::format;
    use ed25519_dalek::SigningKey;
    use proptest::prelude::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::xdr::{AccountId, PublicKey, ScAddress, Uint256};
    use soroban_sdk::TryFromVal;
    use soroban_sdk::{Bytes, Env, String};

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

        let message = WraithNamesContract::authorization_message(
            env,
            operation,
            name,
            stealth_meta_address,
            expiry,
        );
        let message_hash = env.crypto().sha256(&message);
        let message_bytes = message_hash.to_array();
        let signature = signing_key.sign(&message_bytes);
        BytesN::from_array(env, &signature.to_bytes())
    }

    #[test]
    fn test_register_and_resolve() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "alice");
        let meta = Bytes::from_slice(&env, &[1u8; 64]);

        client.register(&owner, &name, &meta);
        assert_eq!(client.resolve(&name), meta);
    }

    #[test]
    fn test_register_on_behalf() {
        let env = Env::default();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let (owner, signing_key) = signing_account(&env, [7u8; 32]);
        let name = String::from_str(&env, "eve");
        let meta = Bytes::from_slice(&env, &[42u8; 64]);
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

        let resolved = client.resolve(&name);
        assert_eq!(resolved, meta);
    }

    #[test]
    #[should_panic]
    fn test_register_on_behalf_wrong_signer_panics() {
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

        client.register_on_behalf(&owner, &name, &meta, &signature, &expiry);
    }

    #[test]
    fn test_register_on_behalf_expired() {
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

    #[test]
    fn test_register_on_behalf_replay() {
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

    #[test]
    fn test_update_on_behalf_and_release_on_behalf() {
        let env = Env::default();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let (owner, signing_key) = signing_account(&env, [55u8; 32]);
        let name = String::from_str(&env, "wendy");
        let meta = Bytes::from_slice(&env, &[10u8; 64]);
        let expiry = u64::from(env.ledger().sequence()) + 10;
        let register_signature = sign_authorization(
            &env,
            &signing_key,
            b"wraith-names:register",
            &name,
            &meta,
            expiry,
        );

        client.register_on_behalf(&owner, &name, &meta, &register_signature, &expiry);

        let updated_meta = Bytes::from_slice(&env, &[11u8; 64]);
        let update_expiry = u64::from(env.ledger().sequence()) + 10;
        let update_signature = sign_authorization(
            &env,
            &signing_key,
            b"wraith-names:update",
            &name,
            &updated_meta,
            update_expiry,
        );
        client.update_on_behalf(
            &owner,
            &name,
            &updated_meta,
            &update_signature,
            &update_expiry,
        );
        assert_eq!(client.resolve(&name), updated_meta);

        let release_expiry = u64::from(env.ledger().sequence()) + 10;
        let release_signature = sign_authorization(
            &env,
            &signing_key,
            b"wraith-names:release",
            &name,
            &Bytes::new(&env),
            release_expiry,
        );
        client.release_on_behalf(&owner, &name, &release_signature, &release_expiry);

        let result = client.try_resolve(&name);
        assert_eq!(result, Err(Ok(NamesError::NameNotFound)));
    }

    #[test]
    fn test_on_behalf_malformed_inputs() {
        let env = Env::default();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let (owner, signing_key) = signing_account(&env, [66u8; 32]);
        let name = String::from_str(&env, "zoe");
        let invalid_meta = Bytes::from_slice(&env, &[1u8; 63]);
        let expiry = u64::from(env.ledger().sequence()) + 10;
        let signature = sign_authorization(
            &env,
            &signing_key,
            b"wraith-names:register",
            &name,
            &invalid_meta,
            expiry,
        );

        let result =
            client.try_register_on_behalf(&owner, &name, &invalid_meta, &signature, &expiry);
        assert_eq!(result, Err(Ok(NamesError::InvalidMetaAddress)));
    }

    proptest! {
        #[test]
        fn prop_register_on_behalf_roundtrip(
            seed in any::<[u8; 32]>(),
            meta_seed in any::<[u8; 64]>(),
            name in proptest::string::string_regex("[a-z0-9]{3,32}").expect("valid name strategy"),
        ) {
            let env = Env::default();

            let contract_id = env.register(WraithNamesContract, ());
            let client = WraithNamesContractClient::new(&env, &contract_id);

            let (owner, signing_key) = signing_account(&env, seed);
            let name = String::from_str(&env, &name);
            let meta = Bytes::from_slice(&env, &meta_seed);
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
            prop_assert_eq!(client.resolve(&name), meta);
        }
    }

    #[test]
    #[ignore] // subdomain flow not wired; enable when register_subdomain lands
    fn test_subdomain_register_and_resolve() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let parent = String::from_str(&env, "alice");
        let parent_meta = Bytes::from_slice(&env, &[42u8; 64]);
        client.register(&owner, &parent, &parent_meta);

        let sub = String::from_str(&env, "payments.alice");
        let sub_meta = Bytes::from_slice(&env, &[7u8; 64]);
        client.register(&owner, &sub, &sub_meta);

        // Subdomain resolves to its own record, parent is unchanged.
        assert_eq!(client.resolve(&sub), sub_meta);
        assert_eq!(client.resolve(&parent), parent_meta);
    }

    #[test]
    #[ignore] // subdomain flow not wired; enable when register_subdomain lands
    fn test_subdomain_requires_existing_parent() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let sub = String::from_str(&env, "team.ghost");
        let meta = Bytes::from_slice(&env, &[3u8; 64]);

        let result = client.try_register(&owner, &sub, &meta);
        assert_eq!(result, Err(Ok(NamesError::ParentNotFound)));
    }

    #[test]
    #[ignore] // subdomain flow not wired; enable when register_subdomain lands
    fn test_subdomain_permission_boundary() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let attacker = Address::generate(&env);
        let parent = String::from_str(&env, "alice");
        let parent_meta = Bytes::from_slice(&env, &[42u8; 64]);
        client.register(&owner, &parent, &parent_meta);

        // Non parent-owner cannot register a subdomain under alice.
        let sub = String::from_str(&env, "payments.alice");
        let sub_meta = Bytes::from_slice(&env, &[7u8; 64]);
        let result = client.try_register(&attacker, &sub, &sub_meta);
        assert_eq!(result, Err(Ok(NamesError::NotOwner)));

        // Parent owner registers it, attacker cannot update or release it.
        client.register(&owner, &sub, &sub_meta);
        let other_meta = Bytes::from_slice(&env, &[8u8; 64]);
        assert_eq!(
            client.try_update(&attacker, &sub, &other_meta),
            Err(Ok(NamesError::NotOwner))
        );
        assert_eq!(
            client.try_release(&attacker, &sub),
            Err(Ok(NamesError::NotOwner))
        );
    }

    #[test]
    #[ignore] // subdomain flow not wired; enable when register_subdomain lands
    fn test_subdomain_update_and_release_by_parent_owner() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let parent = String::from_str(&env, "alice");
        let parent_meta = Bytes::from_slice(&env, &[42u8; 64]);
        client.register(&owner, &parent, &parent_meta);

        let sub = String::from_str(&env, "payments.alice");
        let sub_meta = Bytes::from_slice(&env, &[7u8; 64]);
        client.register(&owner, &sub, &sub_meta);

        // Parent owner updates the subdomain.
        let new_meta = Bytes::from_slice(&env, &[9u8; 64]);
        client.update(&owner, &sub, &new_meta);
        assert_eq!(client.resolve(&sub), new_meta);

        // Parent owner releases the subdomain; it can be re-registered.
        client.release(&owner, &sub);
        assert_eq!(client.try_resolve(&sub), Err(Ok(NamesError::NameNotFound)));
        client.register(&owner, &sub, &sub_meta);
        assert_eq!(client.resolve(&sub), sub_meta);
    }

    #[test]
    #[ignore] // subdomain flow not wired; enable when register_subdomain lands
    fn test_subdomain_orphaned_when_parent_released() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let parent = String::from_str(&env, "alice");
        let parent_meta = Bytes::from_slice(&env, &[42u8; 64]);
        client.register(&owner, &parent, &parent_meta);

        let sub = String::from_str(&env, "payments.alice");
        let sub_meta = Bytes::from_slice(&env, &[7u8; 64]);
        client.register(&owner, &sub, &sub_meta);

        // Releasing the parent makes the subdomain stop resolving.
        client.release(&owner, &parent);
        assert_eq!(client.try_resolve(&sub), Err(Ok(NamesError::NameNotFound)));
    }

    #[test]
    fn test_name_too_deep() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let meta = Bytes::from_slice(&env, &[1u8; 64]);

        // Dotted names nested more than one level deep are rejected.
        let result = client.try_register(&owner, &String::from_str(&env, "a.b.alice"), &meta);
        assert_eq!(result, Err(Ok(NamesError::NameTooDeep)));
    }

    #[test]
    fn test_bulk_register_happy_path() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let names = soroban_sdk::vec![
            &env,
            String::from_str(&env, "app"),
            String::from_str(&env, "docs"),
            String::from_str(&env, "pay"),
        ];
        let metas = soroban_sdk::vec![
            &env,
            Bytes::from_slice(&env, &[1u8; 64]),
            Bytes::from_slice(&env, &[2u8; 64]),
            Bytes::from_slice(&env, &[3u8; 64]),
        ];

        client.bulk_register(&owner, &names, &metas);

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

    #[test]
    fn test_bulk_register_atomic_revert_on_taken() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        // Pre-register "taken"
        client.register(
            &owner,
            &String::from_str(&env, "taken"),
            &Bytes::from_slice(&env, &[1u8; 64]),
        );

        let names = soroban_sdk::vec![
            &env,
            String::from_str(&env, "free1"),
            String::from_str(&env, "taken"),
            String::from_str(&env, "free2"),
        ];
        let metas = soroban_sdk::vec![
            &env,
            Bytes::from_slice(&env, &[1u8; 64]),
            Bytes::from_slice(&env, &[2u8; 64]),
            Bytes::from_slice(&env, &[3u8; 64]),
        ];

        let result = client.try_bulk_register(&owner, &names, &metas);
        assert_eq!(result, Err(Ok(NamesError::NameTaken)));

        // Verify none of the names were registered
        assert_eq!(
            client.try_resolve(&String::from_str(&env, "free1")),
            Err(Ok(NamesError::NameNotFound))
        );
        assert_eq!(
            client.try_resolve(&String::from_str(&env, "free2")),
            Err(Ok(NamesError::NameNotFound))
        );
    }

    #[test]
    fn test_bulk_register_exceeds_limit() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let mut names_vec = soroban_sdk::Vec::new(&env);
        let mut metas_vec = soroban_sdk::Vec::new(&env);
        for i in 0..21 {
            let name_str = format!("name{}", i);
            names_vec.push_back(String::from_str(&env, &name_str));
            metas_vec.push_back(Bytes::from_slice(&env, &[i as u8; 64]));
        }

        let result = client.try_bulk_register(&owner, &names_vec, &metas_vec);
        assert_eq!(result, Err(Ok(NamesError::BulkLimitExceeded)));
    }

    #[test]
    fn test_bulk_renew_happy_path() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let name1 = String::from_str(&env, "alpha");
        let name2 = String::from_str(&env, "beta");
        client.register(&owner, &name1, &Bytes::from_slice(&env, &[1u8; 64]));
        client.register(&owner, &name2, &Bytes::from_slice(&env, &[2u8; 64]));

        let names = soroban_sdk::vec![&env, name1.clone(), name2.clone()];
        let extend_to = env.ledger().sequence() + 10000;
        let result = client.try_bulk_renew(&names, &extend_to);
        assert_eq!(result, Ok(Ok(())));
    }

    #[test]
    fn test_bulk_renew_atomic_revert_on_missing() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        client.register(
            &owner,
            &String::from_str(&env, "exists"),
            &Bytes::from_slice(&env, &[1u8; 64]),
        );

        let names = soroban_sdk::vec![
            &env,
            String::from_str(&env, "exists"),
            String::from_str(&env, "ghost"),
        ];
        let extend_to = env.ledger().sequence() + 10000;
        let result = client.try_bulk_renew(&names, &extend_to);
        assert_eq!(result, Err(Ok(NamesError::NameNotFound)));
    }

    #[test]
    fn test_bulk_renew_exceeds_limit() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let mut names_vec = soroban_sdk::Vec::new(&env);
        for i in 0..21 {
            names_vec.push_back(String::from_str(&env, &format!("name{}", i)));
        }
        let extend_to = env.ledger().sequence() + 10000;
        let result = client.try_bulk_renew(&names_vec, &extend_to);
        assert_eq!(result, Err(Ok(NamesError::BulkLimitExceeded)));
    }

    #[test]
    fn test_bulk_register_invalid_meta_length() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let names = soroban_sdk::vec![&env, String::from_str(&env, "test")];
        let metas = soroban_sdk::vec![&env, Bytes::from_slice(&env, &[1u8; 63])];

        let result = client.try_bulk_register(&owner, &names, &metas);
        assert_eq!(result, Err(Ok(NamesError::InvalidMetaAddress)));
    }

    #[test]
    fn test_bulk_register_mismatched_lengths() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let names = soroban_sdk::vec![
            &env,
            String::from_str(&env, "a"),
            String::from_str(&env, "b")
        ];
        let metas = soroban_sdk::vec![&env, Bytes::from_slice(&env, &[1u8; 64])];

        let result = client.try_bulk_register(&owner, &names, &metas);
        assert_eq!(result, Err(Ok(NamesError::InvalidMetaAddress)));
    }

    // ── Pause / unpause tests ──────────────────────────────────────────────

    #[test]
    fn test_pause_by_admin() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.init(&admin);

        // Initially not paused
        assert!(!client.is_paused());

        // Admin pauses
        client.pause(&admin);
        assert!(client.is_paused());

        // Admin unpauses
        client.unpause(&admin);
        assert!(!client.is_paused());
    }

    #[test]
    fn test_register_rejected_when_paused() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.init(&admin);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "alice");
        let meta = Bytes::from_slice(&env, &[1u8; 64]);

        // Pause
        client.pause(&admin);
        assert!(client.is_paused());

        // Register should be rejected
        let result = client.try_register(&owner, &name, &meta);
        assert_eq!(result, Err(Ok(NamesError::Paused)));
    }

    #[test]
    fn test_update_rejected_when_paused() {
        use soroban_sdk::testutils::Ledger;

        let env = Env::default();
        env.mock_all_auths();

        // Configure ledger for TTL
        {
            let mut info = env.ledger().get();
            info.min_persistent_entry_ttl = 200_000;
            env.ledger().set(info);
        }

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.init(&admin);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "bob");
        let meta = Bytes::from_slice(&env, &[2u8; 64]);
        // Register first
        client.register(&owner, &name, &meta);

        // Pause
        client.pause(&admin);

        // Update should be rejected
        let new_meta = Bytes::from_slice(&env, &[3u8; 64]);
        let result = client.try_update(&owner, &name, &new_meta);
        assert_eq!(result, Err(Ok(NamesError::Paused)));
    }

    #[test]
    fn test_release_rejected_when_paused() {
        use soroban_sdk::testutils::Ledger;

        let env = Env::default();
        env.mock_all_auths();

        {
            let mut info = env.ledger().get();
            info.min_persistent_entry_ttl = 200_000;
            env.ledger().set(info);
        }

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.init(&admin);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "carol");
        let meta = Bytes::from_slice(&env, &[4u8; 64]);
        // Register first
        client.register(&owner, &name, &meta);

        // Pause
        client.pause(&admin);

        // Release should be rejected
        let result = client.try_release(&owner, &name);
        assert_eq!(result, Err(Ok(NamesError::Paused)));
    }

    #[test]
    fn test_resolve_works_when_paused() {
        use soroban_sdk::testutils::Ledger;

        let env = Env::default();
        env.mock_all_auths();

        {
            let mut info = env.ledger().get();
            info.min_persistent_entry_ttl = 200_000;
            env.ledger().set(info);
        }

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.init(&admin);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "dave");
        let meta = Bytes::from_slice(&env, &[5u8; 64]);
        client.register(&owner, &name, &meta);

        // Pause
        client.pause(&admin);

        // Resolve still works while paused
        let resolved = client.resolve(&name);
        assert_eq!(resolved, meta);
    }

    #[test]
    fn test_name_of_works_when_paused() {
        use soroban_sdk::testutils::Ledger;

        let env = Env::default();
        env.mock_all_auths();

        {
            let mut info = env.ledger().get();
            info.min_persistent_entry_ttl = 200_000;
            env.ledger().set(info);
        }

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.init(&admin);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "eve");
        let meta = Bytes::from_slice(&env, &[6u8; 64]);
        client.register(&owner, &name, &meta);

        // Pause
        client.pause(&admin);

        // name_of (reverse lookup) still works while paused
        let resolved_name = client.name_of(&meta);
        assert_eq!(resolved_name, name);
    }

    #[test]
    fn test_extend_name_ttl_rejected_when_paused() {
        use soroban_sdk::testutils::Ledger;

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

        let admin = Address::generate(&env);
        client.init(&admin);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "frank");
        let meta = Bytes::from_slice(&env, &[7u8; 64]);
        client.register(&owner, &name, &meta);

        // Pause
        client.pause(&admin);

        // extend_name_ttl should be rejected
        let extend_to = env.ledger().sequence() + 1000;
        let result = client.try_extend_name_ttl(&name, &extend_to);
        assert_eq!(result, Err(Ok(NamesError::Paused)));
    }

    #[test]
    #[should_panic(expected = "HostError")]
    fn test_admin_only_can_pause_wraith_names() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let attacker = Address::generate(&env);
        client.init(&admin);

        // Non-admin cannot pause (panic expected)
        client.pause(&attacker);
    }

    #[test]
    fn test_register_allowed_after_unpause() {
        use soroban_sdk::testutils::Ledger;

        let env = Env::default();
        env.mock_all_auths();

        {
            let mut info = env.ledger().get();
            info.min_persistent_entry_ttl = 200_000;
            env.ledger().set(info);
        }

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.init(&admin);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "grace");
        let meta = Bytes::from_slice(&env, &[8u8; 64]);

        // Pause then unpause
        client.pause(&admin);
        client.unpause(&admin);
        assert!(!client.is_paused());

        // Register should succeed
        client.register(&owner, &name, &meta);
        assert_eq!(client.resolve(&name), meta);
    }
}
