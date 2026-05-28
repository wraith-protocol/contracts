#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes, BytesN, Env,
    String, Vec,
};

const DELAY_WINDOW: u32 = 100_000;

/// Storage keys.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Maps name hash (BytesN<32>) to NameEntry.
    Name(BytesN<32>),
    /// Reverse lookup: meta-address hash (BytesN<32>) to name hash (BytesN<32>).
    Reverse(BytesN<32>),
    /// Guardian config for a name.
    Guardians(BytesN<32>),
    /// Pending recovery proposal for a name.
    Recovery(BytesN<32>),
}

/// A registered name entry.
#[contracttype]
#[derive(Clone)]
pub struct NameEntry {
    pub name: String,
    pub stealth_meta_address: Bytes,
    pub owner: Address,
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
    NotGuardian = 8,
    NoProposal = 9,
    ProposalAlreadyExists = 10,
    AlreadyApproved = 11,
    DelayNotElapsed = 12,
    ThresholdNotMet = 13,
    TooManyGuardians = 14,
    InvalidThreshold = 15,
}

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

        Self::validate_name(&env, &name)?;
        if stealth_meta_address.len() != 64 {
            return Err(NamesError::InvalidMetaAddress);
        }

        let name_hash = Self::hash_name(&env, &name);
        let name_key = DataKey::Name(name_hash.clone());

        if env.storage().instance().has(&name_key) {
            return Err(NamesError::NameTaken);
        }

        let entry = NameEntry {
            name: name.clone(),
            stealth_meta_address: stealth_meta_address.clone(),
            owner,
        };
        env.storage().instance().set(&name_key, &entry);

        let meta_hash =
            BytesN::from_array(&env, &env.crypto().sha256(&stealth_meta_address).to_array());
        env.storage()
            .instance()
            .set(&DataKey::Reverse(meta_hash), &name_hash);

        env.events().publish(
            (symbol_short!("register"), name_hash),
            (name, stealth_meta_address),
        );

        Ok(())
    }

    /// Update the meta-address for an existing name. Only the current owner can update.
    pub fn update(
        env: Env,
        owner: Address,
        name: String,
        new_meta_address: Bytes,
    ) -> Result<(), NamesError> {
        owner.require_auth();

        if new_meta_address.len() != 64 {
            return Err(NamesError::InvalidMetaAddress);
        }

        let name_hash = Self::hash_name(&env, &name);
        let name_key = DataKey::Name(name_hash.clone());

        let entry: NameEntry = env
            .storage()
            .instance()
            .get(&name_key)
            .ok_or(NamesError::NameNotFound)?;

        if entry.owner != owner {
            return Err(NamesError::NotOwner);
        }

        let old_meta_hash = BytesN::from_array(
            &env,
            &env.crypto().sha256(&entry.stealth_meta_address).to_array(),
        );
        env.storage()
            .instance()
            .remove(&DataKey::Reverse(old_meta_hash));

        let new_entry = NameEntry {
            name: name.clone(),
            stealth_meta_address: new_meta_address.clone(),
            owner,
        };
        env.storage().instance().set(&name_key, &new_entry);

        let new_meta_hash =
            BytesN::from_array(&env, &env.crypto().sha256(&new_meta_address).to_array());
        env.storage()
            .instance()
            .set(&DataKey::Reverse(new_meta_hash), &name_hash);

        env.events().publish(
            (symbol_short!("register"), name_hash),
            (name, new_meta_address),
        );

        Ok(())
    }

    /// Release a name, making it available again.
    pub fn release(env: Env, owner: Address, name: String) -> Result<(), NamesError> {
        owner.require_auth();

        let name_hash = Self::hash_name(&env, &name);
        let name_key = DataKey::Name(name_hash.clone());

        let entry: NameEntry = env
            .storage()
            .instance()
            .get(&name_key)
            .ok_or(NamesError::NameNotFound)?;

        if entry.owner != owner {
            return Err(NamesError::NotOwner);
        }

        let meta_hash = BytesN::from_array(
            &env,
            &env.crypto().sha256(&entry.stealth_meta_address).to_array(),
        );
        env.storage()
            .instance()
            .remove(&DataKey::Reverse(meta_hash));
        env.storage().instance().remove(&name_key);
        env.storage()
            .instance()
            .remove(&DataKey::Guardians(name_hash.clone()));
        env.storage()
            .instance()
            .remove(&DataKey::Recovery(name_hash.clone()));

        env.events()
            .publish((symbol_short!("release"), name_hash), name);

        Ok(())
    }

    /// Resolve a name to its stealth meta-address.
    pub fn resolve(env: Env, name: String) -> Result<Bytes, NamesError> {
        let name_hash = Self::hash_name(&env, &name);
        let entry: NameEntry = env
            .storage()
            .instance()
            .get(&DataKey::Name(name_hash))
            .ok_or(NamesError::NameNotFound)?;
        Ok(entry.stealth_meta_address)
    }

    /// Reverse lookup: find the name for a given stealth meta-address.
    pub fn name_of(env: Env, stealth_meta_address: Bytes) -> Result<String, NamesError> {
        let meta_hash =
            BytesN::from_array(&env, &env.crypto().sha256(&stealth_meta_address).to_array());
        let name_hash: BytesN<32> = env
            .storage()
            .instance()
            .get(&DataKey::Reverse(meta_hash))
            .ok_or(NamesError::NameNotFound)?;
        let entry: NameEntry = env
            .storage()
            .instance()
            .get(&DataKey::Name(name_hash))
            .ok_or(NamesError::NameNotFound)?;
        Ok(entry.name)
    }

    /// Set guardians and threshold for a name. Caller must be the current owner.
    /// Clears any pending recovery proposal.
    pub fn set_guardians(
        env: Env,
        name: String,
        guardians: Vec<Address>,
        threshold: u32,
    ) -> Result<(), NamesError> {
        let name_hash = Self::hash_name(&env, &name);
        let entry: NameEntry = env
            .storage()
            .instance()
            .get(&DataKey::Name(name_hash.clone()))
            .ok_or(NamesError::NameNotFound)?;

        entry.owner.require_auth();

        if guardians.len() > 7 {
            return Err(NamesError::TooManyGuardians);
        }
        if threshold < 1 || threshold > guardians.len() {
            return Err(NamesError::InvalidThreshold);
        }

        env.storage().instance().set(
            &DataKey::Guardians(name_hash.clone()),
            &GuardianConfig { guardians, threshold },
        );
        env.storage()
            .instance()
            .remove(&DataKey::Recovery(name_hash));

        Ok(())
    }

    /// Propose a recovery. `proposer` must be a guardian. No pending proposal may exist.
    pub fn propose_recovery(
        env: Env,
        proposer: Address,
        name: String,
        new_owner: Address,
        new_meta_address: Bytes,
    ) -> Result<(), NamesError> {
        proposer.require_auth();

        if new_meta_address.len() != 64 {
            return Err(NamesError::InvalidMetaAddress);
        }

        let name_hash = Self::hash_name(&env, &name);

        // Name must exist.
        if !env
            .storage()
            .instance()
            .has(&DataKey::Name(name_hash.clone()))
        {
            return Err(NamesError::NameNotFound);
        }

        let config: GuardianConfig = env
            .storage()
            .instance()
            .get(&DataKey::Guardians(name_hash.clone()))
            .ok_or(NamesError::NotGuardian)?;

        if !Self::is_guardian(&proposer, &config.guardians) {
            return Err(NamesError::NotGuardian);
        }

        if env
            .storage()
            .instance()
            .has(&DataKey::Recovery(name_hash.clone()))
        {
            return Err(NamesError::ProposalAlreadyExists);
        }

        let proposal = RecoveryProposal {
            new_owner,
            new_meta_address,
            proposed_at: env.ledger().sequence(),
            approvals: Vec::from_array(&env, [proposer]),
        };
        env.storage()
            .instance()
            .set(&DataKey::Recovery(name_hash), &proposal);

        Ok(())
    }

    /// Approve a pending recovery. `approver` must be a guardian not already in approvals.
    /// If threshold met and delay elapsed, executes the recovery.
    pub fn approve_recovery(env: Env, approver: Address, name: String) -> Result<(), NamesError> {
        approver.require_auth();

        let name_hash = Self::hash_name(&env, &name);

        let config: GuardianConfig = env
            .storage()
            .instance()
            .get(&DataKey::Guardians(name_hash.clone()))
            .ok_or(NamesError::NotGuardian)?;

        if !Self::is_guardian(&approver, &config.guardians) {
            return Err(NamesError::NotGuardian);
        }

        let mut proposal: RecoveryProposal = env
            .storage()
            .instance()
            .get(&DataKey::Recovery(name_hash.clone()))
            .ok_or(NamesError::NoProposal)?;

        for i in 0..proposal.approvals.len() {
            if proposal.approvals.get_unchecked(i) == approver {
                return Err(NamesError::AlreadyApproved);
            }
        }

        proposal.approvals.push_back(approver);

        let threshold_met = proposal.approvals.len() >= config.threshold;
        let delay_elapsed = env.ledger().sequence() >= proposal.proposed_at + DELAY_WINDOW;

        if threshold_met && delay_elapsed {
            let name_key = DataKey::Name(name_hash.clone());
            let entry: NameEntry = env
                .storage()
                .instance()
                .get(&name_key)
                .ok_or(NamesError::NameNotFound)?;

            // Update reverse lookup.
            let old_meta_hash = BytesN::from_array(
                &env,
                &env.crypto().sha256(&entry.stealth_meta_address).to_array(),
            );
            env.storage()
                .instance()
                .remove(&DataKey::Reverse(old_meta_hash));

            let new_meta_hash = BytesN::from_array(
                &env,
                &env.crypto().sha256(&proposal.new_meta_address).to_array(),
            );
            env.storage()
                .instance()
                .set(&DataKey::Reverse(new_meta_hash), &name_hash);

            env.storage().instance().set(
                &name_key,
                &NameEntry {
                    name: entry.name,
                    stealth_meta_address: proposal.new_meta_address,
                    owner: proposal.new_owner,
                },
            );
            env.storage()
                .instance()
                .remove(&DataKey::Recovery(name_hash.clone()));
            env.storage()
                .instance()
                .remove(&DataKey::Guardians(name_hash));
        } else {
            env.storage()
                .instance()
                .set(&DataKey::Recovery(name_hash), &proposal);
        }

        Ok(())
    }

    /// Cancel a pending recovery. Caller must be the current owner and within the delay window.
    pub fn cancel_recovery(env: Env, name: String) -> Result<(), NamesError> {
        let name_hash = Self::hash_name(&env, &name);
        let entry: NameEntry = env
            .storage()
            .instance()
            .get(&DataKey::Name(name_hash.clone()))
            .ok_or(NamesError::NameNotFound)?;

        entry.owner.require_auth();

        let proposal: RecoveryProposal = env
            .storage()
            .instance()
            .get(&DataKey::Recovery(name_hash.clone()))
            .ok_or(NamesError::NoProposal)?;

        if env.ledger().sequence() >= proposal.proposed_at + DELAY_WINDOW {
            return Err(NamesError::DelayNotElapsed);
        }

        env.storage()
            .instance()
            .remove(&DataKey::Recovery(name_hash));

        Ok(())
    }

    // ── helpers ──────────────────────────────────────────────────────────────

    fn is_guardian(addr: &Address, guardians: &Vec<Address>) -> bool {
        for i in 0..guardians.len() {
            if guardians.get_unchecked(i) == *addr {
                return true;
            }
        }
        false
    }

    fn hash_name(env: &Env, name: &String) -> BytesN<32> {
        let len = name.len() as usize;
        let mut buf = [0u8; 32];
        if len > 0 {
            name.copy_into_slice(&mut buf[..len]);
        }
        let bytes = Bytes::from_slice(env, &buf[..len]);
        BytesN::from_array(env, &env.crypto().sha256(&bytes).to_array())
    }

    fn validate_name(_env: &Env, name: &String) -> Result<(), NamesError> {
        let len = name.len() as usize;
        if len < 3 {
            return Err(NamesError::NameTooShort);
        }
        if len > 32 {
            return Err(NamesError::NameTooLong);
        }

        let mut buf = [0u8; 32];
        name.copy_into_slice(&mut buf[..len]);
        for i in 0..len {
            let c = buf[i];
            if !(c >= b'a' && c <= b'z') && !(c >= b'0' && c <= b'9') {
                return Err(NamesError::InvalidNameCharacter);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};
    use soroban_sdk::{Bytes, Env, String, Vec};

    fn setup() -> (Env, soroban_sdk::Address, WraithNamesContractClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        // Set min_persistent_entry_ttl large enough that instance storage
        // never expires when we advance the ledger by DELAY_WINDOW.
        env.ledger().with_mut(|li| {
            li.min_persistent_entry_ttl = DELAY_WINDOW + 10_000;
            li.max_entry_ttl = DELAY_WINDOW + 100_000;
        });
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        (env, owner, client)
    }

    fn register_name<'a>(
        env: &Env,
        client: &WraithNamesContractClient<'a>,
        owner: &Address,
        name: &str,
        meta: &[u8; 64],
    ) {
        let name_str = String::from_str(env, name);
        let meta_bytes = Bytes::from_slice(env, meta);
        client.register(owner, &name_str, &meta_bytes);
    }

    fn make_guardians(env: &Env, n: usize) -> Vec<Address> {
        let mut v = Vec::new(env);
        for _ in 0..n {
            v.push_back(Address::generate(env));
        }
        v
    }

    /// 1. Happy path: propose → approve (threshold met after delay) → ownership transferred.
    #[test]
    fn test_happy_path_recovery() {
        let (env, owner, client) = setup();
        let name = String::from_str(&env, "alice");
        let meta = [1u8; 64];
        register_name(&env, &client, &owner, "alice", &meta);

        let guardians = make_guardians(&env, 2);
        let g0 = guardians.get_unchecked(0);
        let g1 = guardians.get_unchecked(1);
        client.set_guardians(&name, &guardians, &2);

        let new_owner = Address::generate(&env);
        let new_meta = Bytes::from_slice(&env, &[2u8; 64]);

        // Propose at ledger 0.
        client.propose_recovery(&g0, &name, &new_owner, &new_meta);

        // Advance ledger past delay window.
        env.ledger().set_sequence_number(DELAY_WINDOW);

        // Second guardian approves — threshold met and delay elapsed.
        client.approve_recovery(&g1, &name);

        // Ownership transferred.
        assert_eq!(client.resolve(&name), new_meta);
    }

    /// 2. Insufficient approvals: threshold not reached, ownership unchanged.
    #[test]
    fn test_insufficient_approvals() {
        let (env, owner, client) = setup();
        let name = String::from_str(&env, "bob");
        let meta = [1u8; 64];
        register_name(&env, &client, &owner, "bob", &meta);

        let guardians = make_guardians(&env, 3);
        let g0 = guardians.get_unchecked(0);
        let g1 = guardians.get_unchecked(1);
        client.set_guardians(&name, &guardians, &3);

        let new_owner = Address::generate(&env);
        let new_meta = Bytes::from_slice(&env, &[2u8; 64]);

        client.propose_recovery(&g0, &name, &new_owner, &new_meta);
        env.ledger().set_sequence_number(DELAY_WINDOW);

        // Only one more approval (2 total, threshold 3).
        client.approve_recovery(&g1, &name);

        // Ownership unchanged.
        assert_eq!(client.resolve(&name), Bytes::from_slice(&env, &meta));
    }

    /// 3. Delay not elapsed: threshold reached but ledger < proposed_at + DELAY_WINDOW.
    #[test]
    fn test_delay_not_elapsed() {
        let (env, owner, client) = setup();
        let name = String::from_str(&env, "carol");
        let meta = [1u8; 64];
        register_name(&env, &client, &owner, "carol", &meta);

        let guardians = make_guardians(&env, 2);
        let g0 = guardians.get_unchecked(0);
        let g1 = guardians.get_unchecked(1);
        client.set_guardians(&name, &guardians, &2);

        let new_owner = Address::generate(&env);
        let new_meta = Bytes::from_slice(&env, &[2u8; 64]);

        client.propose_recovery(&g0, &name, &new_owner, &new_meta);

        // Do NOT advance ledger — delay not elapsed.
        client.approve_recovery(&g1, &name);

        // Ownership unchanged.
        assert_eq!(client.resolve(&name), Bytes::from_slice(&env, &meta));
    }

    /// 4. Cancel by owner within window: subsequent approve_recovery fails with NoProposal.
    #[test]
    fn test_cancel_recovery() {
        let (env, owner, client) = setup();
        let name = String::from_str(&env, "dave");
        let meta = [1u8; 64];
        register_name(&env, &client, &owner, "dave", &meta);

        let guardians = make_guardians(&env, 2);
        let g0 = guardians.get_unchecked(0);
        let g1 = guardians.get_unchecked(1);
        client.set_guardians(&name, &guardians, &2);

        let new_owner = Address::generate(&env);
        let new_meta = Bytes::from_slice(&env, &[2u8; 64]);

        client.propose_recovery(&g0, &name, &new_owner, &new_meta);
        client.cancel_recovery(&name);

        let result = client.try_approve_recovery(&g1, &name);
        assert_eq!(result, Err(Ok(NamesError::NoProposal)));
    }

    /// 5. Non-guardian cannot call propose_recovery or approve_recovery.
    #[test]
    fn test_non_guardian_rejected() {
        let (env, owner, client) = setup();
        let name = String::from_str(&env, "eve");
        let meta = [1u8; 64];
        register_name(&env, &client, &owner, "eve", &meta);

        let guardians = make_guardians(&env, 1);
        let g0 = guardians.get_unchecked(0);
        client.set_guardians(&name, &guardians, &1);

        let outsider = Address::generate(&env);
        let new_owner = Address::generate(&env);
        let new_meta = Bytes::from_slice(&env, &[2u8; 64]);

        let result = client.try_propose_recovery(&outsider, &name, &new_owner, &new_meta);
        assert_eq!(result, Err(Ok(NamesError::NotGuardian)));

        // Propose legitimately so we can test approve by non-guardian.
        client.propose_recovery(&g0, &name, &new_owner, &new_meta);

        let result = client.try_approve_recovery(&outsider, &name);
        assert_eq!(result, Err(Ok(NamesError::NotGuardian)));
    }

    /// 6. Double approval by same guardian returns AlreadyApproved.
    #[test]
    fn test_double_approval() {
        let (env, owner, client) = setup();
        let name = String::from_str(&env, "frank");
        let meta = [1u8; 64];
        register_name(&env, &client, &owner, "frank", &meta);

        let guardians = make_guardians(&env, 2);
        let g0 = guardians.get_unchecked(0);
        client.set_guardians(&name, &guardians, &2);

        let new_owner = Address::generate(&env);
        let new_meta = Bytes::from_slice(&env, &[2u8; 64]);

        client.propose_recovery(&g0, &name, &new_owner, &new_meta);

        let result = client.try_approve_recovery(&g0, &name);
        assert_eq!(result, Err(Ok(NamesError::AlreadyApproved)));
    }

    /// 7. After successful recovery, old proposal is cleared (double recovery attempt fails).
    #[test]
    fn test_proposal_cleared_after_recovery() {
        let (env, owner, client) = setup();
        let name = String::from_str(&env, "grace");
        let meta = [1u8; 64];
        register_name(&env, &client, &owner, "grace", &meta);

        // Use 2 guardians, threshold 1: g0 proposes (auto-approved), g1 approves after delay.
        let guardians = make_guardians(&env, 2);
        let g0 = guardians.get_unchecked(0);
        let g1 = guardians.get_unchecked(1);
        client.set_guardians(&name, &guardians, &1);

        let new_owner = Address::generate(&env);
        let new_meta = Bytes::from_slice(&env, &[2u8; 64]);

        client.propose_recovery(&g0, &name, &new_owner, &new_meta);
        env.ledger().set_sequence_number(DELAY_WINDOW);
        // g1 approves: threshold met (1 existing + 1 new = 2 >= 1) and delay elapsed.
        client.approve_recovery(&g1, &name);

        // Recovery executed; proposal and guardians cleared.
        // Trying to approve again should fail with NotGuardian (config cleared).
        let result = client.try_approve_recovery(&g0, &name);
        assert_eq!(result, Err(Ok(NamesError::NotGuardian)));
    }

    /// 8. set_guardians clears any pending proposal.
    #[test]
    fn test_set_guardians_clears_proposal() {
        let (env, owner, client) = setup();
        let name = String::from_str(&env, "henry");
        let meta = [1u8; 64];
        register_name(&env, &client, &owner, "henry", &meta);

        let guardians = make_guardians(&env, 2);
        let g0 = guardians.get_unchecked(0);
        client.set_guardians(&name, &guardians, &2);

        let new_owner = Address::generate(&env);
        let new_meta = Bytes::from_slice(&env, &[2u8; 64]);

        client.propose_recovery(&g0, &name, &new_owner, &new_meta);

        // Owner resets guardians — should clear the proposal.
        let new_guardians = make_guardians(&env, 1);
        client.set_guardians(&name, &new_guardians, &1);

        // Old guardian can no longer approve (proposal cleared, and they're not in new config).
        let result = client.try_approve_recovery(&g0, &name);
        assert_eq!(result, Err(Ok(NamesError::NotGuardian)));
    }
}
