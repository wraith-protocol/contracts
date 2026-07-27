//! On-chain multi-sig quorum + timelock signer-rotation flow.
//!
//! Governance signers propose a new signer set and threshold. The proposal
//! must collect approvals from a quorum of the *current* signers and wait
//! out a timelock before it can be executed, mirroring the upgrade timelock
//! described in GOVERNANCE.md / MULTISIG.md. This is the protocol-level
//! governance signer set for `wraith-names` itself — distinct from the
//! per-name `GuardianConfig` recovery guardians defined elsewhere in this
//! crate, which govern individual name ownership rather than the contract.

use soroban_sdk::{contracttype, Address, Env, Symbol, Vec};

use crate::{DataKey, NamesError};

/// 7 days, matching the GOVERNANCE.md upgrade timelock.
pub const ROTATION_TIMELOCK_SECS: u64 = 7 * 24 * 60 * 60;

/// A pending signer-rotation proposal.
#[contracttype]
#[derive(Clone)]
pub struct RotationProposal {
    pub new_signers: Vec<Address>,
    pub new_threshold: u32,
    pub executable_at: u64,
    pub approvals: Vec<Address>,
}

/// One-time setup of the governance signer set used to authorise signer
/// rotations.
pub fn init(env: &Env, signers: Vec<Address>, threshold: u32) -> Result<(), NamesError> {
    if env.storage().instance().has(&DataKey::MultisigSigners) {
        return Err(NamesError::MultisigAlreadyInitialized);
    }
    validate_threshold(&signers, threshold)?;

    env.storage()
        .instance()
        .set(&DataKey::MultisigSigners, &signers);
    env.storage()
        .instance()
        .set(&DataKey::MultisigThreshold, &threshold);
    Ok(())
}

pub fn signers(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&DataKey::MultisigSigners)
        .unwrap_or_else(|| Vec::new(env))
}

pub fn threshold(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::MultisigThreshold)
        .unwrap_or(0)
}

pub fn pending_rotation(env: &Env) -> Option<RotationProposal> {
    env.storage().instance().get(&DataKey::PendingRotation)
}

fn require_signer(env: &Env, caller: &Address) -> Result<(), NamesError> {
    caller.require_auth();
    if !signers(env).contains(caller) {
        return Err(NamesError::NotSigner);
    }
    Ok(())
}

/// Rejects a threshold of zero or a threshold greater than the proposed
/// signer count — a quorum that could never be reached.
fn validate_threshold(signers: &Vec<Address>, threshold: u32) -> Result<(), NamesError> {
    if threshold == 0 || threshold > signers.len() {
        return Err(NamesError::InvalidThreshold);
    }
    Ok(())
}

/// Propose a new signer set + threshold behind the rotation timelock. The
/// caller's approval is recorded automatically. Only one rotation may be
/// pending at a time.
pub fn propose_rotate_signers(
    env: &Env,
    caller: Address,
    new_signers: Vec<Address>,
    new_threshold: u32,
) -> Result<(), NamesError> {
    if !env.storage().instance().has(&DataKey::MultisigSigners) {
        return Err(NamesError::MultisigNotInitialized);
    }
    require_signer(env, &caller)?;

    if env.storage().instance().has(&DataKey::PendingRotation) {
        return Err(NamesError::RotationAlreadyPending);
    }

    validate_threshold(&new_signers, new_threshold)?;

    let mut approvals = Vec::new(env);
    approvals.push_back(caller);

    let proposal = RotationProposal {
        new_signers,
        new_threshold,
        executable_at: env.ledger().timestamp() + ROTATION_TIMELOCK_SECS,
        approvals,
    };
    env.storage()
        .instance()
        .set(&DataKey::PendingRotation, &proposal);
    Ok(())
}

/// Add the caller's approval to the pending rotation proposal.
pub fn approve_rotate_signers(env: &Env, caller: Address) -> Result<(), NamesError> {
    require_signer(env, &caller)?;

    let mut proposal: RotationProposal = env
        .storage()
        .instance()
        .get(&DataKey::PendingRotation)
        .ok_or(NamesError::NoPendingRotation)?;

    if proposal.approvals.contains(&caller) {
        return Err(NamesError::AlreadyApprovedRotation);
    }
    proposal.approvals.push_back(caller);
    env.storage()
        .instance()
        .set(&DataKey::PendingRotation, &proposal);
    Ok(())
}

/// Execute the pending rotation once quorum (measured against the *current*
/// threshold) has been reached and the timelock has elapsed. Clears the
/// pending proposal and emits `SignersRotated`.
pub fn execute_rotate_signers(env: &Env, caller: Address) -> Result<(), NamesError> {
    require_signer(env, &caller)?;

    let proposal: RotationProposal = env
        .storage()
        .instance()
        .get(&DataKey::PendingRotation)
        .ok_or(NamesError::NoPendingRotation)?;

    let old_threshold = threshold(env);
    if (proposal.approvals.len()) < old_threshold {
        return Err(NamesError::QuorumNotMet);
    }
    if env.ledger().timestamp() < proposal.executable_at {
        return Err(NamesError::TimelockNotElapsed);
    }

    env.storage()
        .instance()
        .set(&DataKey::MultisigSigners, &proposal.new_signers);
    env.storage()
        .instance()
        .set(&DataKey::MultisigThreshold, &proposal.new_threshold);
    env.storage().instance().remove(&DataKey::PendingRotation);

    env.events().publish(
        (Symbol::new(env, "SignersRotated"),),
        (proposal.new_signers, old_threshold, proposal.new_threshold),
    );

    Ok(())
}

/// Cancel the pending rotation, fully clearing its storage so a future
/// proposal starts from a clean slate.
pub fn cancel_rotate_signers(env: &Env, caller: Address) -> Result<(), NamesError> {
    require_signer(env, &caller)?;

    if !env.storage().instance().has(&DataKey::PendingRotation) {
        return Err(NamesError::NoPendingRotation);
    }
    env.storage().instance().remove(&DataKey::PendingRotation);
    Ok(())
}
