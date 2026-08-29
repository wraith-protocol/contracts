#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short,
    token::Client as TokenClient, Address, Env, IntoVal, Vec,
};
use wraith_metrics::{contract_ids, dimension_names, emit_metric, metric_names};

mod multisig;
pub use multisig::{RotationProposal, ROTATION_TIMELOCK_SECS};

#[cfg(test)]
mod test;

/// Maximum transfers per batch — justified against Soroban's ~100M instruction
/// budget. Each transfer costs ~500K instructions (token transfer + event emit).
/// 100 transfers = ~50M instructions, leaving headroom for overhead.
pub const MAX_BATCH_SIZE: u32 = 100;

/// Storage keys.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Pause admin address.
    Admin,
    /// Address of the deployed StealthAnnouncer contract, recorded at init
    /// time. Not yet invoked by `batch_send` — see IMPLEMENTATION_NOTES.md.
    Announcer,
    /// Optional address of the asset policy contract.
    AssetPolicy,
    /// Whether the contract is paused.
    Paused,
    /// Governance multisig signer set.
    MultisigSigners,
    /// Governance multisig quorum threshold.
    MultisigThreshold,
    /// Pending signer-rotation proposal, if any.
    PendingRotation,
}

/// Errors that the batch-sender contract can produce.
///
/// Codes are allocated from the `1300-1399` range reserved for
/// `stealth-batch-sender` in `ERRORS.md`'s code-allocation policy.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum BatchSenderError {
    /// The contract has already been initialised.
    AlreadyInitialized = 1300,
    /// The contract has not been initialised yet.
    NotInitialized = 1301,
    /// The batch contains no transfers.
    EmptyBatch = 1302,
    /// The batch exceeds `MAX_BATCH_SIZE`.
    BatchTooLarge = 1303,
    /// A transfer amount was zero or negative.
    NonPositiveAmount = 1304,
    /// A transfer's ephemeral public key was empty.
    EmptyEphemeralKey = 1305,
    /// The contract is paused.
    Paused = 1306,
    /// The asset is not allowed by the configured asset policy.
    AssetNotAllowed = 1307,
    /// The governance multisig has not been initialised.
    MultisigNotInitialized = 1308,
    /// The governance multisig has already been initialised.
    MultisigAlreadyInitialized = 1309,
    /// The caller is not a current governance signer.
    NotSigner = 1310,
    /// The requested threshold is invalid (zero, or greater than signer count).
    InvalidThreshold = 1311,
    /// A signer-rotation proposal is already pending.
    RotationAlreadyPending = 1312,
    /// No signer-rotation proposal is pending.
    NoPendingRotation = 1313,
    /// The caller has already approved the pending rotation.
    AlreadyApprovedRotation = 1314,
    /// The pending rotation has not collected enough approvals yet.
    QuorumNotMet = 1315,
    /// The rotation timelock has not elapsed yet.
    TimelockNotElapsed = 1316,
}

/// A single stealth transfer within a batch.
/// Mirrors the EVM WraithSender batchSendETH/batchSendERC20 structure.
#[contracttype]
#[derive(Clone)]
pub struct Transfer {
    /// Pre-computed stealth address (recipient)
    pub stealth_address: Address,
    /// Ephemeral public key for the recipient to scan with
    pub ephemeral_pub_key: soroban_sdk::Bytes,
    /// Token amount (in the asset's base unit)
    pub amount: i128,
}

/// Wrapper for calling check_asset on the optional asset policy contract.
/// Mirrors stealth-sender's asset_policy_client.
mod asset_policy_client {
    use soroban_sdk::{Address, Env, IntoVal, Symbol};

    pub fn check_asset(env: &Env, policy: &Address, token: &Address) -> bool {
        env.invoke_contract(
            policy,
            &Symbol::new(env, "check_asset"),
            soroban_sdk::vec![env, token.clone().into_val(env)],
        )
    }
}

#[contract]
pub struct StealthBatchSender;

#[contractimpl]
impl StealthBatchSender {
    /// Initialise the contract by storing the pause admin, the announcer
    /// address, and an optional asset policy. Idempotent: a second call
    /// returns `AlreadyInitialized` rather than overwriting the config.
    ///
    /// Must be called before `batch_send`.
    pub fn init(
        env: Env,
        admin: Address,
        announcer: Address,
        asset_policy: Option<Address>,
    ) -> Result<(), BatchSenderError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(BatchSenderError::AlreadyInitialized);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::Announcer, &announcer);

        if let Some(ref policy) = asset_policy {
            env.storage().instance().set(&DataKey::AssetPolicy, policy);
        }

        Ok(())
    }

    /// Pause the contract — admin only. Prevents `batch_send` while paused.
    pub fn pause(env: Env, caller: Address) -> Result<(), BatchSenderError> {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(BatchSenderError::NotInitialized)?;
        if caller != admin {
            panic!("unauthorized: only admin can pause");
        }
        env.storage().instance().set(&DataKey::Paused, &true);
        env.events().publish((symbol_short!("paused"),), (caller,));
        Ok(())
    }

    /// Unpause the contract — admin only.
    pub fn unpause(env: Env, caller: Address) -> Result<(), BatchSenderError> {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(BatchSenderError::NotInitialized)?;
        if caller != admin {
            panic!("unauthorized: only admin can unpause");
        }
        env.storage().instance().set(&DataKey::Paused, &false);
        env.events()
            .publish((symbol_short!("unpaused"),), (caller,));
        Ok(())
    }

    /// Returns true if the contract is paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    /// Require the contract is not paused.
    fn require_not_paused(env: &Env) -> Result<(), BatchSenderError> {
        if env
            .storage()
            .instance()
            .get::<_, bool>(&DataKey::Paused)
            .unwrap_or(false)
        {
            return Err(BatchSenderError::Paused);
        }
        Ok(())
    }

    /// Atomically send `asset` tokens from `from` to N pre-computed stealth
    /// addresses in a single transaction.
    ///
    /// # All-or-nothing semantics
    /// Soroban reverts every state change made during a failed invocation —
    /// whether the failure is a panic or a returned contract error — so a
    /// rejected batch never leaves partial transfers behind.
    ///
    /// # Resource budget
    /// Capped at MAX_BATCH_SIZE (100) transfers. This keeps instruction usage
    /// well under Soroban's per-transaction limit while still being ~100x more
    /// efficient than N individual stealth-sender::send calls (one auth, one
    /// ledger round-trip vs N).
    pub fn batch_send(
        env: Env,
        from: Address,
        transfers: Vec<Transfer>,
        asset: Address,
    ) -> Result<(), BatchSenderError> {
        if !env.storage().instance().has(&DataKey::Admin) {
            return Err(BatchSenderError::NotInitialized);
        }
        Self::require_not_paused(&env)?;

        // Auth: sender must sign once for the entire batch
        from.require_auth();

        if let Some(policy_address) = env
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::AssetPolicy)
        {
            if !asset_policy_client::check_asset(&env, &policy_address, &asset) {
                return Err(BatchSenderError::AssetNotAllowed);
            }
        }

        // Validate batch size
        let count = transfers.len();
        if count == 0 {
            return Err(BatchSenderError::EmptyBatch);
        }
        if count > MAX_BATCH_SIZE {
            return Err(BatchSenderError::BatchTooLarge);
        }

        // Validate every entry before executing any transfer.
        for transfer in transfers.iter() {
            if transfer.amount <= 0 {
                return Err(BatchSenderError::NonPositiveAmount);
            }
            if transfer.ephemeral_pub_key.is_empty() {
                return Err(BatchSenderError::EmptyEphemeralKey);
            }
        }

        let token = TokenClient::new(&env, &asset);

        let mut total_amount: i128 = 0;

        for transfer in transfers.iter() {
            total_amount += transfer.amount;

            // Execute transfer — any failure here aborts the whole tx (atomicity)
            token.transfer(&from, &transfer.stealth_address, &transfer.amount);

            // Per-transfer announcement (mirrors stealth-sender pattern)
            env.events().publish(
                (symbol_short!("ANNOUNCE"),),
                (
                    transfer.stealth_address.clone(),
                    transfer.ephemeral_pub_key.clone(),
                    transfer.amount,
                    asset.clone(),
                ),
            );
        }

        // Batch-level summary event
        env.events()
            .publish((symbol_short!("BATCH"),), (from, count, asset.clone()));

        // Emit metric events.
        emit_metric(
            &env,
            contract_ids::STEALTH_BATCH_SENDER,
            metric_names::BATCH_SEND_COUNT,
            1,
            soroban_sdk::vec![&env, (dimension_names::ASSET_ADDRESS, asset.into_val(&env))],
        );
        emit_metric(
            &env,
            contract_ids::STEALTH_BATCH_SENDER,
            metric_names::BATCH_SEND_VOLUME,
            total_amount,
            soroban_sdk::vec![&env, (dimension_names::ASSET_ADDRESS, asset.into_val(&env))],
        );
        emit_metric(
            &env,
            contract_ids::STEALTH_BATCH_SENDER,
            metric_names::BATCH_SIZE,
            count as i128,
            soroban_sdk::vec![&env, (dimension_names::ASSET_ADDRESS, asset.into_val(&env))],
        );

        Ok(())
    }

    /// Query the maximum allowed batch size.
    pub fn max_batch_size(_env: Env) -> u32 {
        MAX_BATCH_SIZE
    }

    /// One-time setup of the governance signer set used to authorise signer
    /// rotations. Independent of `init` — does not gate `batch_send`.
    pub fn init_multisig(
        env: Env,
        signers: Vec<Address>,
        threshold: u32,
    ) -> Result<(), BatchSenderError> {
        multisig::init(&env, signers, threshold)
    }

    /// Current governance signer set.
    pub fn signers(env: Env) -> Vec<Address> {
        multisig::signers(&env)
    }

    /// Current governance quorum threshold.
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
    ) -> Result<(), BatchSenderError> {
        multisig::propose_rotate_signers(&env, caller, new_signers, new_threshold)
    }

    /// Approve the pending signer-rotation proposal.
    pub fn approve_rotate_signers(env: Env, caller: Address) -> Result<(), BatchSenderError> {
        multisig::approve_rotate_signers(&env, caller)
    }

    /// Execute the pending rotation once quorum is met and the timelock has
    /// elapsed. Emits `SignersRotated`.
    pub fn execute_rotate_signers(env: Env, caller: Address) -> Result<(), BatchSenderError> {
        multisig::execute_rotate_signers(&env, caller)
    }

    /// Cancel the pending rotation, clearing all of its state.
    pub fn cancel_rotate_signers(env: Env, caller: Address) -> Result<(), BatchSenderError> {
        multisig::cancel_rotate_signers(&env, caller)
    }
}
