#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, Bytes, BytesN, Env, Vec,
};

mod multisig;
pub use multisig::RotationProposal;

/// Storage keys.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// The address of the deployed StealthAnnouncer contract.
    Announcer,
    /// Governance multisig signer set.
    MultisigSigners,
    /// Governance multisig quorum threshold.
    MultisigThreshold,
    /// Pending signer-rotation proposal, if any.
    PendingRotation,
}

/// Errors that the sender contract can produce.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SenderError {
    /// The contract has already been initialised.
    AlreadyInitialized = 1,
    /// The contract has not been initialised yet.
    NotInitialized = 2,
    /// The batch input vectors have mismatched lengths.
    LengthMismatch = 3,
    /// The governance multisig has not been initialised.
    MultisigNotInitialized = 6,
    /// The governance multisig has already been initialised.
    MultisigAlreadyInitialized = 7,
    /// The caller is not a current governance signer.
    NotSigner = 8,
    /// The requested threshold is invalid (zero, or greater than signer count).
    InvalidThreshold = 9,
    /// A signer-rotation proposal is already pending.
    RotationAlreadyPending = 10,
    /// No signer-rotation proposal is pending.
    NoPendingRotation = 11,
    /// The caller has already approved the pending rotation.
    AlreadyApprovedRotation = 12,
    /// The pending rotation has not collected enough approvals yet.
    QuorumNotMet = 13,
    /// The rotation timelock has not elapsed yet.
    TimelockNotElapsed = 14,
}

/// Lightweight client wrapper that invokes the StealthAnnouncer contract via
/// `env.invoke_contract`. This avoids needing a compiled WASM at build time
/// (unlike `contractimport!`) and keeps the build self-contained.
mod announcer_client {
    use soroban_sdk::{Address, Bytes, BytesN, Env};

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
            &soroban_sdk::symbol_short!("announce"),
            soroban_sdk::vec![
                env,
                scheme_id.into_val(env),
                stealth_address.into_val(env),
                ephemeral_pub_key.into_val(env),
                metadata.into_val(env),
            ],
        );
    }

    use soroban_sdk::IntoVal;
}

#[contract]
pub struct StealthSenderContract;

#[contractimpl]
impl StealthSenderContract {
    /// Initialise the contract by storing the announcer address.
    ///
    /// Must be called exactly once before any `send` or `batch_send`.
    pub fn init(env: Env, announcer: Address) -> Result<(), SenderError> {
        if env.storage().instance().has(&DataKey::Announcer) {
            return Err(SenderError::AlreadyInitialized);
        }
        env.storage()
            .instance()
            .set(&DataKey::Announcer, &announcer);
        Ok(())
    }

    /// Transfer tokens to a stealth address and emit an announcement.
    ///
    /// # Arguments
    /// * `sender`            - The address sending funds (must authorise).
    /// * `token`             - SAC token contract address (works for native XLM too).
    /// * `amount`            - Amount of tokens to transfer.
    /// * `scheme_id`         - Stealth address scheme identifier.
    /// * `stealth_address`   - The derived one-time stealth address.
    /// * `ephemeral_pub_key` - Ephemeral public key for the recipient to scan.
    /// * `metadata`          - Extra data (e.g. view tag).
    pub fn send(
        env: Env,
        sender: Address,
        token: Address,
        amount: i128,
        scheme_id: u32,
        stealth_address: Address,
        ephemeral_pub_key: BytesN<32>,
        metadata: Bytes,
    ) -> Result<(), SenderError> {
        sender.require_auth();

        let announcer: Address = env
            .storage()
            .instance()
            .get(&DataKey::Announcer)
            .ok_or(SenderError::NotInitialized)?;

        // Transfer tokens from sender to the stealth address.
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&sender, &stealth_address, &amount);

        // Emit the announcement via the announcer contract.
        announcer_client::announce(
            &env,
            &announcer,
            scheme_id,
            &stealth_address,
            &ephemeral_pub_key,
            &metadata,
        );

        Ok(())
    }

    /// Batch version of `send` — transfers tokens to multiple stealth addresses
    /// and emits an announcement for each.
    ///
    /// All input vectors must have the same length.
    pub fn batch_send(
        env: Env,
        sender: Address,
        token: Address,
        scheme_id: u32,
        stealth_addresses: Vec<Address>,
        ephemeral_pub_keys: Vec<BytesN<32>>,
        metadatas: Vec<Bytes>,
        amounts: Vec<i128>,
    ) -> Result<(), SenderError> {
        sender.require_auth();

        let len = stealth_addresses.len();
        if ephemeral_pub_keys.len() != len || metadatas.len() != len || amounts.len() != len {
            return Err(SenderError::LengthMismatch);
        }

        let announcer: Address = env
            .storage()
            .instance()
            .get(&DataKey::Announcer)
            .ok_or(SenderError::NotInitialized)?;

        let token_client = token::Client::new(&env, &token);

        for i in 0..len {
            let stealth_address = stealth_addresses.get(i).unwrap();
            let ephemeral_pub_key = ephemeral_pub_keys.get(i).unwrap();
            let metadata = metadatas.get(i).unwrap();
            let amount = amounts.get(i).unwrap();

            token_client.transfer(&sender, &stealth_address, &amount);

            announcer_client::announce(
                &env,
                &announcer,
                scheme_id,
                &stealth_address,
                &ephemeral_pub_key,
                &metadata,
            );
        }

        Ok(())
    }

    /// One-time setup of the governance signer set used to authorise signer
    /// rotations. Independent of `init` — does not gate `send`/`batch_send`.
    pub fn init_multisig(
        env: Env,
        signers: Vec<Address>,
        threshold: u32,
    ) -> Result<(), SenderError> {
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
    ) -> Result<(), SenderError> {
        multisig::propose_rotate_signers(&env, caller, new_signers, new_threshold)
    }

    /// Approve the pending signer-rotation proposal.
    pub fn approve_rotate_signers(env: Env, caller: Address) -> Result<(), SenderError> {
        multisig::approve_rotate_signers(&env, caller)
    }

    /// Execute the pending rotation once quorum is met and the timelock has
    /// elapsed. Emits `SignersRotated`.
    pub fn execute_rotate_signers(env: Env, caller: Address) -> Result<(), SenderError> {
        multisig::execute_rotate_signers(&env, caller)
    }

    /// Cancel the pending rotation, clearing all of its state.
    pub fn cancel_rotate_signers(env: Env, caller: Address) -> Result<(), SenderError> {
        multisig::cancel_rotate_signers(&env, caller)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::testutils::Ledger;
    use soroban_sdk::{Bytes, BytesN, Env};

    #[contract]
    pub struct MockAnnouncer;

    #[contractimpl]
    impl MockAnnouncer {
        pub fn announce(
            env: Env,
            scheme_id: u32,
            stealth_address: Address,
            ephemeral_pub_key: BytesN<32>,
            metadata: Bytes,
        ) {
            env.events().publish(
                (
                    soroban_sdk::symbol_short!("announce"),
                    scheme_id,
                    stealth_address,
                ),
                (env.current_contract_address(), ephemeral_pub_key, metadata),
            );
        }
    }

    #[test]
    fn test_sender_workflow() {
        let env = Env::default();
        env.mock_all_auths();

        let announcer_id = env.register(MockAnnouncer, ());
        let sender_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(&env, &sender_id);

        let token_admin = Address::generate(&env);
        let token_id = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();
        let token_client = token::Client::new(&env, &token_id);

        client.init(&announcer_id);

        // Verify AlreadyInitialized error
        let init_res = client.try_init(&announcer_id);
        assert_eq!(init_res, Err(Ok(SenderError::AlreadyInitialized)));

        // Setup transfer accounts and mint tokens
        let sender = Address::generate(&env);
        let stealth_address = Address::generate(&env);

        let token_admin_client = token::StellarAssetClient::new(&env, &token_id);
        token_admin_client.mint(&sender, &1000);

        assert_eq!(token_client.balance(&sender), 1000);
        assert_eq!(token_client.balance(&stealth_address), 0);

        // Test send functionality
        let epk = BytesN::from_array(&env, &[1u8; 32]);
        let meta = Bytes::from_slice(&env, &[0u8; 1]);

        client.send(&sender, &token_id, &500, &1, &stealth_address, &epk, &meta);

        // Check balances
        assert_eq!(token_client.balance(&sender), 500);
        assert_eq!(token_client.balance(&stealth_address), 500);
    }

    #[test]
    fn test_batch_send() {
        let env = Env::default();
        env.mock_all_auths();

        let announcer_id = env.register(MockAnnouncer, ());
        let sender_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(&env, &sender_id);

        let token_admin = Address::generate(&env);
        let token_id = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();
        let token_client = token::Client::new(&env, &token_id);

        client.init(&announcer_id);

        let sender = Address::generate(&env);
        let token_admin_client = token::StellarAssetClient::new(&env, &token_id);
        token_admin_client.mint(&sender, &2000);

        let stealth_addr_1 = Address::generate(&env);
        let stealth_addr_2 = Address::generate(&env);

        let epk_1 = BytesN::from_array(&env, &[1u8; 32]);
        let epk_2 = BytesN::from_array(&env, &[2u8; 32]);

        let meta_1 = Bytes::from_slice(&env, &[10u8; 1]);
        let meta_2 = Bytes::from_slice(&env, &[20u8; 1]);

        let addresses = soroban_sdk::vec![&env, stealth_addr_1.clone(), stealth_addr_2.clone()];
        let epks = soroban_sdk::vec![&env, epk_1, epk_2];
        let metadatas = soroban_sdk::vec![&env, meta_1, meta_2];
        let amounts = soroban_sdk::vec![&env, 700, 800];

        client.batch_send(
            &sender, &token_id, &1, &addresses, &epks, &metadatas, &amounts,
        );

        assert_eq!(token_client.balance(&sender), 500);
        assert_eq!(token_client.balance(&stealth_addr_1), 700);
        assert_eq!(token_client.balance(&stealth_addr_2), 800);
    }

    fn setup_multisig(env: &Env) -> (StealthSenderContractClient, Vec<Address>) {
        let announcer_id = env.register(MockAnnouncer, ());
        let sender_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(env, &sender_id);
        client.init(&announcer_id);

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

        let announcer_id = env.register(MockAnnouncer, ());
        let sender_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(&env, &sender_id);
        client.init(&announcer_id);

        let signers = soroban_sdk::vec![&env, Address::generate(&env), Address::generate(&env)];

        // Zero threshold is unreachable.
        let res = client.try_init_multisig(&signers, &0);
        assert_eq!(res, Err(Ok(SenderError::InvalidThreshold)));

        // Threshold greater than signer count is unreachable.
        let res = client.try_init_multisig(&signers, &3);
        assert_eq!(res, Err(Ok(SenderError::InvalidThreshold)));
    }

    #[test]
    fn test_propose_rotate_signers_rejects_invalid_threshold() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, signers) = setup_multisig(&env);
        let new_signers = soroban_sdk::vec![&env, Address::generate(&env), Address::generate(&env)];

        let res = client.try_propose_rotate_signers(&signers.get(0).unwrap(), &new_signers, &0);
        assert_eq!(res, Err(Ok(SenderError::InvalidThreshold)));

        let res = client.try_propose_rotate_signers(&signers.get(0).unwrap(), &new_signers, &3);
        assert_eq!(res, Err(Ok(SenderError::InvalidThreshold)));

        // No proposal was recorded by the rejected attempts.
        assert!(client.pending_rotation().is_none());
    }

    #[test]
    fn test_rotate_signers_requires_quorum_and_timelock() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, signers) = setup_multisig(&env);

        let new_signers = soroban_sdk::vec![&env, Address::generate(&env), Address::generate(&env)];

        client.propose_rotate_signers(&signers.get(0).unwrap(), &new_signers, &2);

        // Only one rotation may be pending at a time.
        let res = client.try_propose_rotate_signers(&signers.get(1).unwrap(), &new_signers, &2);
        assert_eq!(res, Err(Ok(SenderError::RotationAlreadyPending)));

        // Only 1 of 3 required approvals so far (the proposer's).
        let res = client.try_execute_rotate_signers(&signers.get(0).unwrap());
        assert_eq!(res, Err(Ok(SenderError::QuorumNotMet)));

        client.approve_rotate_signers(&signers.get(1).unwrap());
        client.approve_rotate_signers(&signers.get(2).unwrap());

        // Quorum met, but the timelock has not elapsed yet.
        let res = client.try_execute_rotate_signers(&signers.get(0).unwrap());
        assert_eq!(res, Err(Ok(SenderError::TimelockNotElapsed)));

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
        assert_eq!(res, Err(Ok(SenderError::NoPendingRotation)));
        let res = client.try_execute_rotate_signers(&signers.get(0).unwrap());
        assert_eq!(res, Err(Ok(SenderError::NoPendingRotation)));
        let res = client.try_cancel_rotate_signers(&signers.get(0).unwrap());
        assert_eq!(res, Err(Ok(SenderError::NoPendingRotation)));

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
        assert_eq!(res, Err(Ok(SenderError::NotSigner)));

        client.propose_rotate_signers(&signers.get(0).unwrap(), &new_signers, &2);
        let res = client.try_approve_rotate_signers(&outsider);
        assert_eq!(res, Err(Ok(SenderError::NotSigner)));
    }
}
