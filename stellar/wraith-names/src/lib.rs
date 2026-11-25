#![no_std]

// Only the test module allocates. Linking `alloc` into the crate unconditionally
// makes the wasm32 cdylib require a global allocator it does not have.
#[cfg(test)]
extern crate alloc;

use core::convert::TryInto;

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
    /// Subdomain era of a top-level name (see `NameEntry::era`): created at 0
    /// when the name gets its first subdomain and incremented every time the
    /// name is released. It only ever increases, so an era is never reused.
    SubdomainEra(BytesN<32>),
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
    /// For a subdomain, the parent's `SubdomainEra` when the subdomain was
    /// registered; `None` for a top-level name. The subdomain is live only
    /// while the parent's era is unchanged: releasing the parent advances it,
    /// so every subdomain registered before the release is retired for good,
    /// even if the parent is registered again.
    pub era: Option<u64>,
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

/// An ed25519 account strkey (`G…`) is 56 unpadded base32 characters over
/// `[version byte, 32-byte public key, 2-byte CRC16 checksum]`.
const ACCOUNT_STRKEY_CHARS: usize = 56;
const ACCOUNT_STRKEY_BYTES: usize = 35;
/// Version 6 left-shifted into the high five bits, which is what makes the
/// strkey start with `G`.
const ACCOUNT_VERSION_BYTE: u8 = 6 << 3;

fn base32_quintet(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'2'..=b'7' => Some(26 + byte - b'2'),
        _ => None,
    }
}

fn crc16_xmodem(bytes: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for &byte in bytes {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            let overflowed = crc & 0x8000 != 0;
            crc = crc.wrapping_shl(1);
            if overflowed {
                crc ^= 0x1021;
            }
        }
    }
    crc
}

/// Recovers the ed25519 public key from an account strkey.
///
/// Returns `None` for a contract address (`C…`), a muxed account (`M…`), or a
/// corrupt checksum, which the caller maps to `NamesError::InvalidSigner`.
fn account_public_key(strkey: &[u8]) -> Option<[u8; 32]> {
    if strkey.len() != ACCOUNT_STRKEY_CHARS {
        return None;
    }

    let mut decoded = [0u8; ACCOUNT_STRKEY_BYTES];
    let mut accumulator: u32 = 0;
    let mut bits: u32 = 0;
    let mut written: usize = 0;
    for &byte in strkey {
        let quintet = base32_quintet(byte)?;
        accumulator = (accumulator << 5) | quintet as u32;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            decoded[written] = ((accumulator >> bits) & 0xff) as u8;
            written += 1;
        }
    }

    if written != ACCOUNT_STRKEY_BYTES || decoded[0] != ACCOUNT_VERSION_BYTE {
        return None;
    }
    if u16::from_le_bytes([decoded[33], decoded[34]]) != crc16_xmodem(&decoded[..33]) {
        return None;
    }

    let mut public_key = [0u8; 32];
    public_key.copy_from_slice(&decoded[1..33]);
    Some(public_key)
}

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

    /// Register the subdomain `label.parent` (e.g. `payments` under `alice` is
    /// `payments.alice`), equivalent to `register(owner, "label.parent", ..)`.
    ///
    /// `parent` must be a live top-level name and `owner` its current owner.
    /// `label` follows the usual label rules (3+ lowercase alphanumeric
    /// characters) and the full name must fit in 32 characters. The subdomain
    /// is managed by whoever owns `parent` from then on, and is retired when
    /// `parent` is released.
    pub fn register_subdomain(
        env: Env,
        owner: Address,
        parent: String,
        label: String,
        stealth_meta_address: Bytes,
    ) -> Result<(), NamesError> {
        Self::require_not_paused(&env)?;
        owner.require_auth();
        let name = Self::join_labels(&env, &label, &parent)?;
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
            if Self::is_taken(&env, &DataKey::Name(name_hash)) {
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
            // A retired subdomain fails the whole (atomic) batch.
            let parent = Self::live_parent(&env, &entry)?;
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
            if let Some(parent) = parent {
                Self::extend_parent_to(&env, &entry, &parent, current_ledger, extend_to_ledger);
            }

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
        // soroban-sdk converts an `Address` to an `xdr::ScAddress` only on the
        // host (`cfg(not(target_family = "wasm"))`), so the deployed wasm has to
        // recover the owner's key from the strkey instead.
        let strkey = owner.to_string();
        if strkey.len() as usize != ACCOUNT_STRKEY_CHARS {
            return Err(NamesError::InvalidSigner);
        }
        let mut strkey_bytes = [0u8; ACCOUNT_STRKEY_CHARS];
        strkey.copy_into_slice(&mut strkey_bytes);
        let public_key = account_public_key(&strkey_bytes).ok_or(NamesError::InvalidSigner)?;
        Ok(BytesN::from_array(env, &public_key))
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

        let parent_hash = Self::parent_label(env, &name).map(|p| Self::hash_name(env, &p));

        // During the 90-day premium window, top-level names of 4 characters
        // or fewer can only be obtained through the sealed-bid auction.
        let len = name.len() as usize;
        if !via_auction && parent_hash.is_none() && auction::premium_block_active(env, len) {
            return Err(NamesError::PremiumAuctionRequired);
        }

        let name_hash = Self::hash_name(env, &name);
        let name_key = DataKey::Name(name_hash.clone());

        // A live name is taken. A subdomain left behind by a released parent is
        // dead: drop its reverse entry and let the new registration replace it.
        if let Some(existing) = env.storage().persistent().get::<_, NameEntry>(&name_key) {
            if Self::live_parent(env, &existing).is_ok() {
                return Err(NamesError::NameTaken);
            }
            Self::clear_reverse(env, &existing.stealth_meta_address, &name_hash);
        }

        // Subdomains require an existing parent and that `owner` is the parent
        // owner. They record the parent's current subdomain era.
        let era = match parent_hash {
            Some(ref ph) => {
                let parent: NameEntry = env
                    .storage()
                    .persistent()
                    .get(&DataKey::Name(ph.clone()))
                    .ok_or(NamesError::ParentNotFound)?;
                if parent.owner != owner {
                    return Err(NamesError::NotOwner);
                }
                Some(Self::subdomain_era(env, ph).unwrap_or_else(|| {
                    Self::set_subdomain_era(env, ph, 0);
                    0
                }))
            }
            None => None,
        };

        let entry = NameEntry {
            name: name.clone(),
            stealth_meta_address: stealth_meta_address.clone(),
            owner,
            parent: parent_hash.clone(),
            era,
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
            era: entry.era,
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

        // Retire every subdomain registered under this name. Names that never
        // had a subdomain have no era to advance.
        if entry.parent.is_none() {
            if let Some(era) = Self::subdomain_era(env, &name_hash) {
                let next = era.checked_add(1).expect("subdomain era overflow");
                Self::set_subdomain_era(env, &name_hash, next);
            }
        }

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

    /// Who may update or release `entry`: the owner of a top-level name, or the
    /// current owner of a subdomain's live parent. Subdomain control is re-read
    /// from the parent on every call, so it moves with the parent's ownership;
    /// a subdomain of a released parent is `NameNotFound`.
    fn require_manager(env: &Env, caller: &Address, entry: &NameEntry) -> Result<(), NamesError> {
        let manager = match Self::live_parent(env, entry)? {
            Some(parent) => parent.owner,
            None => entry.owner.clone(),
        };
        if manager == *caller {
            Ok(())
        } else {
            Err(NamesError::NotOwner)
        }
    }

    /// The parent of a subdomain, if it is still live: it must exist and its
    /// subdomain era must still be the one the subdomain was registered under.
    /// Top-level names have no parent (`Ok(None)`). A subdomain whose parent
    /// was released, or released and registered again, is dead and reports
    /// `NameNotFound`; eras never repeat, so it cannot come back.
    ///
    /// If the parent's entry has expired (been archived), reading it fails: a
    /// subdomain cannot be used while its parent is archived.
    fn live_parent(env: &Env, entry: &NameEntry) -> Result<Option<NameEntry>, NamesError> {
        let Some(ref parent_hash) = entry.parent else {
            return Ok(None);
        };
        let parent = env
            .storage()
            .persistent()
            .get::<_, NameEntry>(&DataKey::Name(parent_hash.clone()))
            .ok_or(NamesError::NameNotFound)?;
        if entry.era.is_none() || Self::subdomain_era(env, parent_hash) != entry.era {
            return Err(NamesError::NameNotFound);
        }
        Ok(Some(parent))
    }

    fn subdomain_era(env: &Env, parent_hash: &BytesN<32>) -> Option<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::SubdomainEra(parent_hash.clone()))
    }

    fn set_subdomain_era(env: &Env, parent_hash: &BytesN<32>, era: u64) {
        let key = DataKey::SubdomainEra(parent_hash.clone());
        env.storage().persistent().set(&key, &era);
        Self::extend_ttls(env, &key, None);
    }

    /// Whether `name_key` holds a live name (a dead subdomain does not count).
    fn is_taken(env: &Env, name_key: &DataKey) -> bool {
        env.storage()
            .persistent()
            .get::<_, NameEntry>(name_key)
            .is_some_and(|existing| Self::live_parent(env, &existing).is_ok())
    }

    /// Remove the reverse entry for `meta` if it still points at `name_hash`.
    fn clear_reverse(env: &Env, meta: &Bytes, name_hash: &BytesN<32>) {
        let reverse_key = DataKey::Reverse(BytesN::from_array(
            env,
            &env.crypto().sha256(meta).to_array(),
        ));
        let points_here = env
            .storage()
            .persistent()
            .get::<_, BytesN<32>>(&reverse_key)
            .is_some_and(|target| target == *name_hash);
        if points_here {
            env.storage().persistent().remove(&reverse_key);
        }
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
    /// (`alice`): if the parent has been released, or released and registered
    /// again, the subdomain does not resolve. Resolving a subdomain renews the
    /// parent's entry too.
    pub fn resolve(env: Env, name: String) -> Result<Bytes, NamesError> {
        let name_hash = Self::hash_name(&env, &name);
        let name_key = DataKey::Name(name_hash);
        let live = env
            .storage()
            .persistent()
            .get::<_, NameEntry>(&name_key)
            .ok_or(NamesError::NameNotFound)
            .and_then(|entry| Self::live_parent(&env, &entry).map(|_| entry));
        let entry = match live {
            Ok(entry) => entry,
            Err(err) => {
                // Emit metric event. Published before the error is returned, so
                // it is captured whenever the enclosing transaction is applied.
                emit_metric(
                    &env,
                    contract_ids::WRAITH_NAMES,
                    metric_names::RESOLVE_MISS_COUNT,
                    1,
                    Vec::new(&env),
                );
                return Err(err);
            }
        };

        Self::extend_ttls(&env, &name_key, None);
        if let Some(ref parent_hash) = entry.parent {
            Self::extend_ttls(&env, &DataKey::Name(parent_hash.clone()), None);
            Self::extend_ttls(&env, &DataKey::SubdomainEra(parent_hash.clone()), None);
        }

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
        Self::live_parent(&env, &entry)?;

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

        // Check if name exists (a subdomain also needs its live parent)
        let entry: NameEntry = env
            .storage()
            .persistent()
            .get(&name_key)
            .ok_or(NamesError::NameNotFound)?;
        let parent = Self::live_parent(&env, &entry)?;

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
        // A subdomain is only usable while its parent is, so renew both.
        if let Some(parent) = parent {
            Self::extend_parent_to(&env, &entry, &parent, current_ledger, extend_to_ledger);
        }
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

    /// Extend a subdomain's parent entry, its reverse entry and its subdomain
    /// era to `to`.
    fn extend_parent_to(env: &Env, entry: &NameEntry, parent: &NameEntry, current: u32, to: u32) {
        let storage = env.storage().persistent();
        if let Some(ref parent_hash) = entry.parent {
            storage.extend_ttl(&DataKey::Name(parent_hash.clone()), current, to);
            storage.extend_ttl(&DataKey::SubdomainEra(parent_hash.clone()), current, to);
        }
        let reverse_key = DataKey::Reverse(BytesN::from_array(
            env,
            &env.crypto().sha256(&parent.stealth_meta_address).to_array(),
        ));
        if storage.has(&reverse_key) {
            storage.extend_ttl(&reverse_key, current, to);
        }
    }

    /// The parent label of a validated subdomain (`alice` for `payments.alice`).
    fn parent_label(env: &Env, name: &String) -> Option<String> {
        let len = name.len() as usize;
        let mut buf = [0u8; MAX_NAME_LEN];
        name.copy_into_slice(&mut buf[..len]);
        let dot = buf[..len].iter().position(|&c| c == b'.')?;
        Some(String::from_bytes(env, &buf[dot + 1..len]))
    }

    /// Build `label.parent`, rejecting results longer than `MAX_NAME_LEN`.
    fn join_labels(env: &Env, label: &String, parent: &String) -> Result<String, NamesError> {
        let (label_len, parent_len) = (label.len() as usize, parent.len() as usize);
        let len = label_len + 1 + parent_len;
        if len > MAX_NAME_LEN {
            return Err(NamesError::NameTooLong);
        }
        let mut buf = [0u8; MAX_NAME_LEN];
        label.copy_into_slice(&mut buf[..label_len]);
        buf[label_len] = b'.';
        parent.copy_into_slice(&mut buf[label_len + 1..len]);
        Ok(String::from_bytes(env, &buf[..len]))
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

    /// Validate a name: a top-level label (`alice`) or one subdomain level
    /// (`payments.alice`). Every label is at least `MIN_LABEL_LEN` lowercase
    /// alphanumeric characters, and the full name is at most `MAX_NAME_LEN`.
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
        let name_bytes = &name_buf[..len];

        let mut dots = 0usize;
        for &c in name_bytes {
            if c == b'.' {
                dots += 1;
            } else if !c.is_ascii_lowercase() && !c.is_ascii_digit() {
                return Err(NamesError::InvalidNameCharacter);
            }
        }
        if dots > MAX_SUBDOMAIN_DEPTH {
            return Err(NamesError::NameTooDeep);
        }

        // Each label on its own, so `x.alice`, `.alice` and `alice.` are rejected.
        if name_bytes
            .split(|&c| c == b'.')
            .any(|label| label.len() < MIN_LABEL_LEN)
        {
            return Err(NamesError::NameTooShort);
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

    #[test]
    fn strkey_decode_recovers_the_account_public_key() {
        let env = Env::default();
        let seed = [7u8; 32];
        let (owner, signing_key) = signing_account(&env, seed);
        let strkey = owner.to_string();
        let mut bytes = [0u8; ACCOUNT_STRKEY_CHARS];
        strkey.copy_into_slice(&mut bytes);
        let decoded = account_public_key(&bytes).expect("account strkey decodes");
        assert_eq!(decoded, signing_key.verifying_key().to_bytes());
    }

    /// The all-zero ed25519 public key as encoded by the reference strkey
    /// encoder: version byte `6 << 3`, 32 zero bytes, then the CRC16 checksum
    /// in little-endian byte order.
    const ZERO_ACCOUNT_STRKEY: &[u8; ACCOUNT_STRKEY_CHARS] =
        b"GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";

    #[test]
    fn strkey_decode_matches_a_known_vector() {
        let decoded = account_public_key(ZERO_ACCOUNT_STRKEY).expect("valid strkey");
        assert_eq!(decoded, [0u8; 32]);
    }

    #[test]
    fn strkey_decode_rejects_a_corrupt_checksum() {
        let mut strkey = *ZERO_ACCOUNT_STRKEY;
        strkey[ACCOUNT_STRKEY_CHARS - 1] = b'A';
        assert_eq!(account_public_key(&strkey), None);
    }

    #[test]
    fn strkey_decode_rejects_non_account_addresses() {
        let env = Env::default();
        let contract = Address::generate(&env);
        let strkey = contract.to_string();
        let mut bytes = [0u8; ACCOUNT_STRKEY_CHARS];
        strkey.copy_into_slice(&mut bytes);
        assert_eq!(account_public_key(&bytes), None);
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

    fn subdomain_setup() -> (Env, WraithNamesContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        (env, client, contract_id)
    }

    fn meta(env: &Env, byte: u8) -> Bytes {
        Bytes::from_slice(env, &[byte; 64])
    }

    fn name_key(env: &Env, name: &str) -> DataKey {
        DataKey::Name(WraithNamesContract::hash_name(
            env,
            &String::from_str(env, name),
        ))
    }

    #[test]
    fn test_register_subdomain_entry_point() {
        let (env, client, _) = subdomain_setup();
        let owner = Address::generate(&env);
        let parent = String::from_str(&env, "alice");
        client.register(&owner, &parent, &meta(&env, 1));

        let label = String::from_str(&env, "payments");
        client.register_subdomain(&owner, &parent, &label, &meta(&env, 2));

        // The call is authorized by the parent owner.
        assert_eq!(env.auths()[0].0, owner);

        let sub = String::from_str(&env, "payments.alice");
        assert_eq!(client.resolve(&sub), meta(&env, 2));
        assert_eq!(client.name_of(&meta(&env, 2)), sub);

        // Same name, either entry point: it is taken.
        assert_eq!(
            client.try_register(&owner, &sub, &meta(&env, 3)),
            Err(Ok(NamesError::NameTaken))
        );
        assert_eq!(
            client.try_register_subdomain(&owner, &parent, &label, &meta(&env, 3)),
            Err(Ok(NamesError::NameTaken))
        );
    }

    #[test]
    fn test_register_subdomain_rejections() {
        let (env, client, _) = subdomain_setup();
        let owner = Address::generate(&env);
        let other = Address::generate(&env);
        let parent = String::from_str(&env, "alice");
        client.register(&owner, &parent, &meta(&env, 1));

        let try_sub = |caller: &Address, parent: &str, label: &str| {
            client.try_register_subdomain(
                caller,
                &String::from_str(&env, parent),
                &String::from_str(&env, label),
                &meta(&env, 2),
            )
        };

        assert_eq!(
            try_sub(&other, "alice", "payments"),
            Err(Ok(NamesError::NotOwner))
        );
        assert_eq!(
            try_sub(&owner, "nobody", "payments"),
            Err(Ok(NamesError::ParentNotFound))
        );
        assert_eq!(
            try_sub(&owner, "alice", "ab"),
            Err(Ok(NamesError::NameTooShort))
        );
        assert_eq!(
            try_sub(&owner, "alice", ""),
            Err(Ok(NamesError::NameTooShort))
        );
        assert_eq!(
            try_sub(&owner, "alice", "pay.ments"),
            Err(Ok(NamesError::NameTooDeep))
        );
        assert_eq!(
            try_sub(&owner, "pay.alice", "team"),
            Err(Ok(NamesError::NameTooDeep))
        );
        assert_eq!(
            try_sub(&owner, "alice", "Pay"),
            Err(Ok(NamesError::InvalidNameCharacter))
        );
        // 26 + 1 + 5 = 32 characters fits; one more does not.
        assert_eq!(
            try_sub(&owner, "alice", "abcdefghijklmnopqrstuvwxyz"),
            Ok(Ok(()))
        );
        assert_eq!(
            try_sub(&owner, "alice", "abcdefghijklmnopqrstuvwxyz0"),
            Err(Ok(NamesError::NameTooLong))
        );
    }

    #[test]
    fn test_subdomain_label_and_name_limits() {
        let (env, client, _) = subdomain_setup();
        let owner = Address::generate(&env);
        client.register(&owner, &String::from_str(&env, "alice"), &meta(&env, 1));

        let try_name =
            |name: &str| client.try_register(&owner, &String::from_str(&env, name), &meta(&env, 2));

        // Every label must be at least three characters.
        assert_eq!(try_name("ab.alice"), Err(Ok(NamesError::NameTooShort)));
        assert_eq!(try_name(".alice"), Err(Ok(NamesError::NameTooShort)));
        assert_eq!(try_name("alice."), Err(Ok(NamesError::NameTooShort)));
        assert_eq!(try_name("pay.ab"), Err(Ok(NamesError::NameTooShort)));
        // One level of nesting at most.
        assert_eq!(try_name("pay..alice"), Err(Ok(NamesError::NameTooDeep)));
        // The full name, dot included, is at most 32 characters.
        assert_eq!(
            try_name("abcdefghijklmnopqrstuvwxyz0.alice"),
            Err(Ok(NamesError::NameTooLong))
        );
        assert_eq!(try_name("abcdefghijklmnopqrstuvwxyz.alice"), Ok(Ok(())));
        assert_eq!(try_name("pay.alice"), Ok(Ok(())));
    }

    #[test]
    fn test_subdomain_not_revived_when_parent_registered_again() {
        let (env, client, _) = subdomain_setup();
        let first = Address::generate(&env);
        let second = Address::generate(&env);
        let parent = String::from_str(&env, "alice");
        let sub = String::from_str(&env, "payments.alice");

        client.register(&first, &parent, &meta(&env, 1));
        client.register(&first, &sub, &meta(&env, 2));
        client.release(&first, &parent);

        // Someone else registers the released parent: the old subdomain stays
        // retired, in both directions, and the previous holder cannot touch it.
        client.register(&second, &parent, &meta(&env, 3));
        assert_eq!(client.try_resolve(&sub), Err(Ok(NamesError::NameNotFound)));
        assert_eq!(
            client.try_name_of(&meta(&env, 2)),
            Err(Ok(NamesError::NameNotFound))
        );
        assert_eq!(
            client.try_update(&first, &sub, &meta(&env, 4)),
            Err(Ok(NamesError::NameNotFound))
        );
        assert_eq!(
            client.try_release(&first, &sub),
            Err(Ok(NamesError::NameNotFound))
        );

        // The new parent owner can claim the subdomain name afresh.
        client.register(&second, &sub, &meta(&env, 5));
        assert_eq!(client.resolve(&sub), meta(&env, 5));
        assert_eq!(client.name_of(&meta(&env, 5)), sub);
        assert_eq!(
            client.try_name_of(&meta(&env, 2)),
            Err(Ok(NamesError::NameNotFound))
        );
        assert_eq!(
            client.try_update(&first, &sub, &meta(&env, 4)),
            Err(Ok(NamesError::NotOwner))
        );
    }

    #[test]
    fn test_same_owner_reregistering_parent_starts_fresh() {
        let (env, client, _) = subdomain_setup();
        let owner = Address::generate(&env);
        let parent = String::from_str(&env, "alice");
        let sub = String::from_str(&env, "payments.alice");

        client.register(&owner, &parent, &meta(&env, 1));
        client.register(&owner, &sub, &meta(&env, 2));
        client.release(&owner, &parent);
        client.register(&owner, &parent, &meta(&env, 1));

        assert_eq!(client.try_resolve(&sub), Err(Ok(NamesError::NameNotFound)));
        client.register(&owner, &sub, &meta(&env, 2));
        assert_eq!(client.resolve(&sub), meta(&env, 2));
        assert_eq!(client.name_of(&meta(&env, 2)), sub);
    }

    #[test]
    fn test_subdomain_control_follows_parent_owner() {
        let (env, client, contract_id) = subdomain_setup();
        let old_owner = Address::generate(&env);
        let new_owner = Address::generate(&env);
        let parent = String::from_str(&env, "alice");
        let sub = String::from_str(&env, "payments.alice");

        client.register(&old_owner, &parent, &meta(&env, 1));
        client.register(&old_owner, &sub, &meta(&env, 2));

        // The parent changes hands while its registration stays in place (as a
        // guardian recovery would). There is no public transfer call yet, so the
        // test rewrites the owner directly.
        env.as_contract(&contract_id, || {
            let key = name_key(&env, "alice");
            let mut entry: NameEntry = env.storage().persistent().get(&key).unwrap();
            entry.owner = new_owner.clone();
            env.storage().persistent().set(&key, &entry);
        });

        // The subdomain keeps resolving, and only the new parent owner manages it.
        assert_eq!(client.resolve(&sub), meta(&env, 2));
        assert_eq!(
            client.try_update(&old_owner, &sub, &meta(&env, 3)),
            Err(Ok(NamesError::NotOwner))
        );
        assert_eq!(
            client.try_release(&old_owner, &sub),
            Err(Ok(NamesError::NotOwner))
        );
        assert_eq!(
            client.try_register(
                &old_owner,
                &String::from_str(&env, "team.alice"),
                &meta(&env, 4)
            ),
            Err(Ok(NamesError::NotOwner))
        );

        client.update(&new_owner, &sub, &meta(&env, 5));
        assert_eq!(client.resolve(&sub), meta(&env, 5));
        client.register(
            &new_owner,
            &String::from_str(&env, "team.alice"),
            &meta(&env, 6),
        );
        client.release(&new_owner, &sub);
        assert_eq!(client.try_resolve(&sub), Err(Ok(NamesError::NameNotFound)));
    }

    fn subdomain_era_of(env: &Env, contract_id: &Address, name: &str) -> Option<u64> {
        env.as_contract(contract_id, || {
            let hash = WraithNamesContract::hash_name(env, &String::from_str(env, name));
            env.storage().persistent().get(&DataKey::SubdomainEra(hash))
        })
    }

    fn parent_era_of(env: &Env, contract_id: &Address, name: &str) -> u64 {
        env.as_contract(contract_id, || {
            let entry: NameEntry = env
                .storage()
                .persistent()
                .get(&name_key(env, name))
                .unwrap();
            entry.era.unwrap()
        })
    }

    #[test]
    fn test_subdomain_eras_never_repeat_across_reregistrations() {
        let (env, client, contract_id) = subdomain_setup();
        let holders = [Address::generate(&env), Address::generate(&env)];
        let parent = String::from_str(&env, "alice");
        let sub = String::from_str(&env, "pay.alice");

        // Release and re-register the parent repeatedly, all in one ledger and
        // alternating holders: the case a time- or randomness-derived era
        // could repeat.
        let mut eras: alloc::vec::Vec<u64> = alloc::vec::Vec::new();
        for round in 0..5u8 {
            let holder = &holders[round as usize % 2];
            client.register(holder, &parent, &meta(&env, 1));
            client.register(holder, &sub, &meta(&env, 10 + round));
            assert_eq!(client.resolve(&sub), meta(&env, 10 + round));
            eras.push(parent_era_of(&env, &contract_id, "pay.alice"));
            client.release(holder, &parent);

            // Every subdomain registered so far stays retired.
            assert_eq!(client.try_resolve(&sub), Err(Ok(NamesError::NameNotFound)));
            for earlier in 0..=round {
                assert_eq!(
                    client.try_name_of(&meta(&env, 10 + earlier)),
                    Err(Ok(NamesError::NameNotFound))
                );
            }
        }

        assert!(
            eras.windows(2).all(|w| w[0] < w[1]),
            "eras repeat: {eras:?}"
        );
        assert_eq!(subdomain_era_of(&env, &contract_id, "alice"), Some(5));
    }

    #[test]
    fn test_stale_subdomain_from_any_earlier_era_cannot_resolve() {
        let (env, client, contract_id) = subdomain_setup();
        let first = Address::generate(&env);
        let second = Address::generate(&env);
        let parent = String::from_str(&env, "alice");
        for round in 0..3u8 {
            client.register(&first, &parent, &meta(&env, 1));
            client.register(&first, &String::from_str(&env, "pay.alice"), &meta(&env, 2));
            client.release(&first, &parent);
            assert_eq!(
                subdomain_era_of(&env, &contract_id, "alice"),
                Some(round as u64 + 1)
            );
        }
        client.register(&second, &parent, &meta(&env, 3));
        let current = subdomain_era_of(&env, &contract_id, "alice").unwrap();

        // Forge a stale subdomain record, with its reverse entry, for every era
        // issued before the latest release and for an era not yet issued.
        let team = String::from_str(&env, "team.alice");
        for (i, era) in (0..current).chain([current + 1, u64::MAX]).enumerate() {
            let stale_meta = meta(&env, 100 + i as u8);
            env.as_contract(&contract_id, || {
                let hash = WraithNamesContract::hash_name(&env, &team);
                let entry = NameEntry {
                    name: team.clone(),
                    stealth_meta_address: stale_meta.clone(),
                    owner: first.clone(),
                    parent: Some(WraithNamesContract::hash_name(&env, &parent)),
                    era: Some(era),
                };
                env.storage()
                    .persistent()
                    .set(&DataKey::Name(hash.clone()), &entry);
                let meta_hash =
                    BytesN::from_array(&env, &env.crypto().sha256(&stale_meta).to_array());
                env.storage()
                    .persistent()
                    .set(&DataKey::Reverse(meta_hash), &hash);
            });

            assert_eq!(
                client.try_resolve(&team),
                Err(Ok(NamesError::NameNotFound)),
                "era {era}"
            );
            assert_eq!(
                client.try_name_of(&stale_meta),
                Err(Ok(NamesError::NameNotFound))
            );
            assert_eq!(
                client.try_update(&second, &team, &meta(&env, 4)),
                Err(Ok(NamesError::NameNotFound))
            );
        }

        // The current holder registers over the stale record; only that resolves.
        client.register(&second, &team, &meta(&env, 5));
        assert_eq!(client.resolve(&team), meta(&env, 5));
        assert_eq!(parent_era_of(&env, &contract_id, "team.alice"), current);
    }

    #[test]
    fn test_names_without_subdomains_get_no_era_entry() {
        let (env, client, contract_id) = subdomain_setup();
        let owner = Address::generate(&env);
        let name = String::from_str(&env, "bob");
        client.register(&owner, &name, &meta(&env, 1));
        client.release(&owner, &name);
        assert_eq!(subdomain_era_of(&env, &contract_id, "bob"), None);
    }

    #[test]
    fn test_renewing_subdomain_renews_parent() {
        use soroban_sdk::testutils::storage::Persistent as _;
        use soroban_sdk::testutils::Ledger as _;

        // `extend_name_ttl` / `bulk_renew` renew entries whose remaining TTL is
        // below the current ledger sequence, so start past the initial TTL.
        let env = Env::default();
        env.mock_all_auths();
        let seq = TTL_EXTEND_TO + 10_000;
        env.ledger().with_mut(|li| {
            li.sequence_number = seq;
            li.max_entry_ttl = 1_000_000;
        });
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        for (i, parent) in ["alice", "bob", "carol"].into_iter().enumerate() {
            let i = i as u8 * 2;
            client.register(&owner, &String::from_str(&env, parent), &meta(&env, i + 1));
            let sub = format!("pay.{parent}");
            client.register(&owner, &String::from_str(&env, &sub), &meta(&env, i + 2));
        }
        let ttl = |name: &str| {
            env.as_contract(&contract_id, || {
                env.storage().persistent().get_ttl(&name_key(&env, name))
            })
        };

        client.extend_name_ttl(&String::from_str(&env, "pay.alice"), &(seq + 1_000));
        assert_eq!(ttl("pay.alice"), seq + 1_000);
        assert_eq!(ttl("alice"), seq + 1_000);

        let mut names = Vec::new(&env);
        names.push_back(String::from_str(&env, "pay.bob"));
        client.bulk_renew(&names, &(seq + 2_000));
        assert_eq!(ttl("pay.bob"), seq + 2_000);
        assert_eq!(ttl("bob"), seq + 2_000);

        // Renewing a parent alone leaves its subdomains as they were.
        client.extend_name_ttl(&String::from_str(&env, "carol"), &(seq + 3_000));
        assert_eq!(ttl("carol"), seq + 3_000);
        assert_eq!(ttl("pay.carol"), TTL_EXTEND_TO);
    }

    #[test]
    #[should_panic(expected = "Error(Storage, InternalError)")]
    fn test_subdomain_unusable_while_parent_archived() {
        use soroban_sdk::testutils::Ledger as _;

        let (env, client, contract_id) = subdomain_setup();
        let owner = Address::generate(&env);
        client.register(&owner, &String::from_str(&env, "alice"), &meta(&env, 1));
        let sub = String::from_str(&env, "payments.alice");
        client.register(&owner, &sub, &meta(&env, 2));

        // Keep the instance and the subdomain's own entry live, but let the
        // parent's entry expire and be archived.
        let far = TTL_EXTEND_TO * 2;
        env.as_contract(&contract_id, || {
            env.storage().instance().extend_ttl(far, far);
            env.storage()
                .persistent()
                .extend_ttl(&name_key(&env, "payments.alice"), far, far);
        });
        env.ledger()
            .with_mut(|li| li.sequence_number += TTL_EXTEND_TO + 1);

        // Reading the subdomain needs its archived parent, so it cannot succeed
        // until the parent is restored (on a real network the transaction is
        // rejected before the contract runs).
        let _ = client.try_resolve(&sub);
    }

    #[test]
    fn test_orphaned_subdomain_cannot_be_renewed() {
        let (env, client, _) = subdomain_setup();
        let owner = Address::generate(&env);
        let parent = String::from_str(&env, "alice");
        let sub = String::from_str(&env, "payments.alice");
        client.register(&owner, &parent, &meta(&env, 1));
        client.register(&owner, &sub, &meta(&env, 2));
        client.release(&owner, &parent);

        let to = env.ledger().sequence() + 1_000;
        assert_eq!(
            client.try_extend_name_ttl(&sub, &to),
            Err(Ok(NamesError::NameNotFound))
        );
        let mut names = Vec::new(&env);
        names.push_back(sub);
        assert_eq!(
            client.try_bulk_renew(&names, &to),
            Err(Ok(NamesError::NameNotFound))
        );
    }

    #[test]
    fn test_bulk_register_parent_and_subdomain() {
        let (env, client, _) = subdomain_setup();
        let owner = Address::generate(&env);
        let mut names = Vec::new(&env);
        names.push_back(String::from_str(&env, "alice"));
        names.push_back(String::from_str(&env, "pay.alice"));
        let mut metas = Vec::new(&env);
        metas.push_back(meta(&env, 1));
        metas.push_back(meta(&env, 2));

        client.bulk_register(&owner, &names, &metas);
        assert_eq!(
            client.resolve(&String::from_str(&env, "pay.alice")),
            meta(&env, 2)
        );

        // A retired subdomain does not block bulk registration under a new parent
        // registration.
        client.release(&owner, &String::from_str(&env, "alice"));
        client.bulk_register(&owner, &names, &metas);
        assert_eq!(
            client.resolve(&String::from_str(&env, "pay.alice")),
            meta(&env, 2)
        );
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
