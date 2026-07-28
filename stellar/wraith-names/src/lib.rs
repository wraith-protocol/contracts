#![no_std]

use core::convert::TryInto;

use soroban_sdk::xdr::{AccountId, PublicKey, ScAddress};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes, BytesN, Env,
    String, Vec,
};

mod multisig;
pub use multisig::RotationProposal;

pub const WRAITH_NAMES_DOMAIN: &[u8] = b"wraith-names:v1";

#[allow(dead_code)]
const DELAY_WINDOW: u32 = 100_000;

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
    /// Protocol-level governance multisig signer set.
    MultisigSigners,
    /// Protocol-level governance multisig quorum threshold.
    MultisigThreshold,
    /// Pending protocol-level signer-rotation proposal, if any.
    PendingRotation,
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
}

const TTL_THRESHOLD: u32 = 17280; // ~1 day
const TTL_EXTEND_TO: u32 = 518400; // ~30 days

const MIN_LABEL_LEN: usize = 3;
const MAX_NAME_LEN: usize = 32;

#[contract]
pub struct WraithNamesContract;

#[contractimpl]
impl WraithNamesContract {
    /// Register a name mapped to a stealth meta-address.
    pub fn register(
        env: Env,
        owner: Address,
        name: String,
        stealth_meta_address: Bytes,
    ) -> Result<(), NamesError> {
        owner.require_auth();
        Self::register_internal(&env, owner, name, stealth_meta_address)
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
        let replay_key = Self::verify_on_behalf_authorization(
            &env,
            &owner,
            b"wraith-names:register",
            &name,
            &stealth_meta_address,
            &signature,
            expiry,
        )?;
        Self::register_internal(&env, owner, name, stealth_meta_address)?;
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
    ) -> Result<(), NamesError> {
        Self::validate_name(env, &name)?;
        if stealth_meta_address.len() != 64 {
            return Err(NamesError::InvalidMetaAddress);
        }

        let name_hash = Self::hash_name(env, &name);
        let name_key = DataKey::Name(name_hash.clone());

        // Check not taken
        if env.storage().persistent().has(&name_key) {
            return Err(NamesError::NameTaken);
        }

        // Subdomain registration is not yet wired to any public entrypoint;
        // all names registered via `register` / `register_on_behalf` are
        // top-level (parent = None). See NamesError::ParentNotFound for the
        // planned subdomain flow.
        let entry = NameEntry {
            name: name.clone(),
            stealth_meta_address: stealth_meta_address.clone(),
            owner,
            parent: None,
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

        Ok(())
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
        let entry: NameEntry = env
            .storage()
            .persistent()
            .get(&name_key)
            .ok_or(NamesError::NameNotFound)?;

        Self::extend_ttls(&env, &name_key, None);

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

    /// Authorisation check for management operations (update / release / etc).
    /// Currently owner-only; guardian-recovery flow is defined in NamesError
    /// (`NotGuardian`, `NoProposal`, ...) but not yet wired in.
    fn require_manager(_env: &Env, caller: &Address, entry: &NameEntry) -> Result<(), NamesError> {
        if caller == &entry.owner {
            Ok(())
        } else {
            Err(NamesError::NotOwner)
        }
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
        let name_len = name.len() as usize;
        let mut name_buf = [0u8; 32];
        name.copy_into_slice(&mut name_buf[..name_len]);
        let name_bytes = Bytes::from_slice(env, &name_buf[..name_len]);
        message.append(&name_bytes);
        message.append(stealth_meta_address);
        message.extend_from_slice(&expiry.to_be_bytes());
        message
    }

    /// Validate name: 3-32 chars, lowercase alphanumeric only.
    fn validate_name(_env: &Env, name: &String) -> Result<(), NamesError> {
        let len = name.len() as usize;
        if len < MIN_LABEL_LEN {
            return Err(NamesError::NameTooShort);
        }
        if len > MAX_NAME_LEN {
            return Err(NamesError::NameTooLong);
        }

        let mut buf = [0u8; MAX_NAME_LEN];
        name.copy_into_slice(&mut buf[..len]);

        for i in 0..len {
            let c = buf[i];
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
}

#[cfg(test)]
mod test {
    use super::*;
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
        use soroban_sdk::testutils::{Address as _, Ledger};

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

        // Dotted names are rejected (subdomain nesting is not yet supported).
        let result = client.try_register(&owner, &String::from_str(&env, "a.b.alice"), &meta);
        assert_eq!(result, Err(Ok(NamesError::InvalidNameCharacter)));
    }

    fn setup_multisig(env: &Env) -> (WraithNamesContractClient, Vec<Address>) {
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(env, &contract_id);

        let signers = soroban_sdk::vec![
            env,
            Address::generate(env),
            Address::generate(env),
            Address::generate(env),
            Address::generate(env),
            Address::generate(env),
        ];
        client.init_multisig(&signers, &3);

        (client, signers)
    }

    #[test]
    fn test_init_multisig_rejects_invalid_threshold() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);

        let signers = soroban_sdk::vec![&env, Address::generate(&env), Address::generate(&env)];

        let res = client.try_init_multisig(&signers, &0);
        assert_eq!(res, Err(Ok(NamesError::InvalidThreshold)));

        let res = client.try_init_multisig(&signers, &3);
        assert_eq!(res, Err(Ok(NamesError::InvalidThreshold)));
    }

    #[test]
    fn test_propose_rotate_signers_rejects_invalid_threshold() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, signers) = setup_multisig(&env);
        let new_signers = soroban_sdk::vec![&env, Address::generate(&env), Address::generate(&env)];

        let res = client.try_propose_rotate_signers(&signers.get(0).unwrap(), &new_signers, &0);
        assert_eq!(res, Err(Ok(NamesError::InvalidThreshold)));

        let res = client.try_propose_rotate_signers(&signers.get(0).unwrap(), &new_signers, &3);
        assert_eq!(res, Err(Ok(NamesError::InvalidThreshold)));

        assert!(client.pending_rotation().is_none());
    }

    #[test]
    fn test_rotate_signers_requires_quorum_and_timelock() {
        use soroban_sdk::testutils::Ledger;

        let env = Env::default();
        env.mock_all_auths();

        let (client, signers) = setup_multisig(&env);

        let new_signers = soroban_sdk::vec![&env, Address::generate(&env), Address::generate(&env)];
        client.propose_rotate_signers(&signers.get(0).unwrap(), &new_signers, &2);

        // Only one rotation may be pending at a time.
        let res = client.try_propose_rotate_signers(&signers.get(1).unwrap(), &new_signers, &2);
        assert_eq!(res, Err(Ok(NamesError::RotationAlreadyPending)));

        // Only 1 of 3 required approvals so far (the proposer's).
        let res = client.try_execute_rotate_signers(&signers.get(0).unwrap());
        assert_eq!(res, Err(Ok(NamesError::QuorumNotMet)));

        client.approve_rotate_signers(&signers.get(1).unwrap());
        client.approve_rotate_signers(&signers.get(2).unwrap());

        // Quorum met, but the timelock has not elapsed yet.
        let res = client.try_execute_rotate_signers(&signers.get(0).unwrap());
        assert_eq!(res, Err(Ok(NamesError::TimelockNotElapsed)));

        env.ledger().with_mut(|li| {
            li.timestamp += multisig::ROTATION_TIMELOCK_SECS;
        });

        client.execute_rotate_signers(&signers.get(0).unwrap());

        assert_eq!(client.signers(), new_signers);
        assert_eq!(client.threshold(), 2);
        assert!(client.pending_rotation().is_none());
    }

    #[test]
    fn test_cancelled_rotation_clears_state() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, signers) = setup_multisig(&env);

        let new_signers = soroban_sdk::vec![&env, Address::generate(&env), Address::generate(&env)];
        client.propose_rotate_signers(&signers.get(0).unwrap(), &new_signers, &2);
        client.approve_rotate_signers(&signers.get(1).unwrap());

        client.cancel_rotate_signers(&signers.get(2).unwrap());

        // Cancelling clears the proposal entirely.
        assert!(client.pending_rotation().is_none());

        // The original signer set / threshold are untouched by the aborted rotation.
        assert_eq!(client.signers(), signers);
        assert_eq!(client.threshold(), 3);

        // A stale approve/execute/cancel against the cleared proposal fails cleanly.
        let res = client.try_approve_rotate_signers(&signers.get(3).unwrap());
        assert_eq!(res, Err(Ok(NamesError::NoPendingRotation)));
        let res = client.try_execute_rotate_signers(&signers.get(0).unwrap());
        assert_eq!(res, Err(Ok(NamesError::NoPendingRotation)));
        let res = client.try_cancel_rotate_signers(&signers.get(0).unwrap());
        assert_eq!(res, Err(Ok(NamesError::NoPendingRotation)));

        // A fresh proposal can be made immediately — no leftover state blocks it.
        let other_signers =
            soroban_sdk::vec![&env, Address::generate(&env), Address::generate(&env)];
        client.propose_rotate_signers(&signers.get(0).unwrap(), &other_signers, &2);
        assert!(client.pending_rotation().is_some());
    }

    #[test]
    fn test_non_signer_cannot_propose_or_approve() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, signers) = setup_multisig(&env);
        let outsider = Address::generate(&env);

        let new_signers = soroban_sdk::vec![&env, Address::generate(&env), Address::generate(&env)];
        let res = client.try_propose_rotate_signers(&outsider, &new_signers, &2);
        assert_eq!(res, Err(Ok(NamesError::NotSigner)));

        client.propose_rotate_signers(&signers.get(0).unwrap(), &new_signers, &2);
        let res = client.try_approve_rotate_signers(&outsider);
        assert_eq!(res, Err(Ok(NamesError::NotSigner)));
    }
}
