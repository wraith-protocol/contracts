#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, Bytes, BytesN, Env,
    IntoVal, Vec,
};
use wraith_metrics::{contract_ids, dimension_names, emit_metric, metric_names};

mod multisig;
pub use multisig::RotationProposal;

/// Storage keys.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// The address of the deployed StealthAnnouncer contract.
    Announcer,
    /// Optional address of the asset policy contract.
    AssetPolicy,
    /// Optional address of the protocol fee recipient.
    FeeRecipient,
    /// Protocol fee in basis points (max 50 bps, 0 = disabled).
    FeeBasisPoints,
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
    /// The token is not allowed by the asset policy.
    TokenNotAllowed = 4,
    /// The fee configuration is invalid (e.g. fee > 50 bps, or fee > 0 with no recipient).
    InvalidFeeConfig = 5,
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
    /// The sponsored announcement entry list exceeded the per-call cap.
    BatchTooLarge = 15,
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

/// Wrapper for calling check_asset on the optional asset policy contract.
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

const TTL_THRESHOLD: u32 = 17280; // ~1 day
const TTL_EXTEND_TO: u32 = 518400; // ~30 days

/// Maximum entries permitted in a single `sponsored_announce` call.
///
/// Enforced to keep a single fee-bumped Stellar op bounded: base + 20 op-count
/// fits the standard Soroban resource budget comfortably and protects against
/// landmark-bloat of the announcement event stream.
pub const SPONSORED_MAX_ENTRIES: u32 = 20;

/// One signer's bundled stealth announcement inside a `sponsored_announce` call.
///
/// The `sender` field carries the per-entry signer authentication (the fee
/// source the Stellar fee-bump envelope delegates per-op charging to). The
/// `sponsor` carrying the network-level fee is recorded at the function
/// argument level so it is recorded on the transaction auth list without being
/// conflated with any individual entry's signer.
#[contracttype]
#[derive(Clone, Debug)]
pub struct SponsoredEntry {
    pub sender: Address,
    pub token: Address,
    pub amount: i128,
    pub scheme_id: u32,
    pub stealth_address: Address,
    pub ephemeral_pub_key: BytesN<32>,
    pub metadata: Bytes,
}

#[contract]
pub struct StealthSenderContract;

#[contractimpl]
impl StealthSenderContract {
    /// Initialise the contract by storing the announcer address, optional asset policy,
    /// and optional protocol fee configuration.
    ///
    /// Must be called exactly once before any `send` or `batch_send`.
    pub fn init(
        env: Env,
        announcer: Address,
        asset_policy: Option<Address>,
        fee_recipient: Option<Address>,
        fee_basis_points: u32,
    ) -> Result<(), SenderError> {
        if env.storage().instance().has(&DataKey::Announcer) {
            return Err(SenderError::AlreadyInitialized);
        }
        if fee_basis_points > 50 {
            return Err(SenderError::InvalidFeeConfig);
        }
        if fee_basis_points > 0 && fee_recipient.is_none() {
            return Err(SenderError::InvalidFeeConfig);
        }

        env.storage()
            .instance()
            .set(&DataKey::Announcer, &announcer);

        if let Some(ref policy) = asset_policy {
            env.storage().instance().set(&DataKey::AssetPolicy, policy);
        }

        if let Some(ref recipient) = fee_recipient {
            env.storage()
                .instance()
                .set(&DataKey::FeeRecipient, recipient);
        }
        env.storage()
            .instance()
            .set(&DataKey::FeeBasisPoints, &fee_basis_points);

        // Extend instance TTL
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);

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

        // Extend instance TTL
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);

        // Check asset policy if configured
        if let Some(policy_address) = env
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::AssetPolicy)
        {
            if !asset_policy_client::check_asset(&env, &policy_address, &token) {
                return Err(SenderError::TokenNotAllowed);
            }
        }

        // Retrieve fee config
        let fee_basis_points: u32 = env
            .storage()
            .instance()
            .get(&DataKey::FeeBasisPoints)
            .unwrap_or(0);

        let fee_recipient: Option<Address> = env.storage().instance().get(&DataKey::FeeRecipient);

        let fee = if fee_basis_points > 0 && fee_recipient.is_some() {
            (amount * (fee_basis_points as i128)) / 10000
        } else {
            0
        };

        let transfer_amount = amount - fee;

        let token_client = token::Client::new(&env, &token);

        // Divert fee to recipient if set
        if fee > 0 {
            if let Some(ref recipient) = fee_recipient {
                token_client.transfer(&sender, recipient, &fee);
            }
        }

        // Transfer tokens from sender to the stealth address.
        token_client.transfer(&sender, &stealth_address, &transfer_amount);

        // Emit the announcement via the announcer contract.
        announcer_client::announce(
            &env,
            &announcer,
            scheme_id,
            &stealth_address,
            &ephemeral_pub_key,
            &metadata,
        );

        // Emit metric events.
        emit_metric(
            &env,
            contract_ids::STEALTH_SENDER,
            metric_names::SEND_COUNT,
            1,
            soroban_sdk::vec![
                &env,
                (dimension_names::SCHEME_ID, scheme_id.into_val(&env)),
                (dimension_names::TOKEN_ADDRESS, token.into_val(&env)),
            ],
        );
        emit_metric(
            &env,
            contract_ids::STEALTH_SENDER,
            metric_names::SEND_VOLUME,
            amount,
            soroban_sdk::vec![
                &env,
                (dimension_names::SCHEME_ID, scheme_id.into_val(&env)),
                (dimension_names::TOKEN_ADDRESS, token.into_val(&env)),
            ],
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

        // Extend instance TTL
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);

        // Check asset policy if configured
        if let Some(policy_address) = env
            .storage()
            .instance()
            .get::<_, Address>(&DataKey::AssetPolicy)
        {
            if !asset_policy_client::check_asset(&env, &policy_address, &token) {
                return Err(SenderError::TokenNotAllowed);
            }
        }

        // Retrieve fee config
        let fee_basis_points: u32 = env
            .storage()
            .instance()
            .get(&DataKey::FeeBasisPoints)
            .unwrap_or(0);

        let fee_recipient: Option<Address> = env.storage().instance().get(&DataKey::FeeRecipient);

        let token_client = token::Client::new(&env, &token);

        let mut total_amount: i128 = 0;
        let mut total_fee: i128 = 0;

        for i in 0..len {
            let stealth_address = stealth_addresses.get(i).unwrap();
            let ephemeral_pub_key = ephemeral_pub_keys.get(i).unwrap();
            let metadata = metadatas.get(i).unwrap();
            let amount = amounts.get(i).unwrap();

            let fee = if fee_basis_points > 0 && fee_recipient.is_some() {
                (amount * (fee_basis_points as i128)) / 10000
            } else {
                0
            };

            total_fee += fee;
            let transfer_amount = amount - fee;
            total_amount += amount;

            token_client.transfer(&sender, &stealth_address, &transfer_amount);

            announcer_client::announce(
                &env,
                &announcer,
                scheme_id,
                &stealth_address,
                &ephemeral_pub_key,
                &metadata,
            );
        }

        // Divert total accumulated fee atomically to recipient
        if total_fee > 0 {
            if let Some(ref recipient) = fee_recipient {
                token_client.transfer(&sender, recipient, &total_fee);
            }
        }

        // Emit metric events.
        emit_metric(
            &env,
            contract_ids::STEALTH_SENDER,
            metric_names::BATCH_SEND_COUNT,
            1,
            soroban_sdk::vec![
                &env,
                (dimension_names::SCHEME_ID, scheme_id.into_val(&env)),
                (dimension_names::TOKEN_ADDRESS, token.into_val(&env)),
            ],
        );
        emit_metric(
            &env,
            contract_ids::STEALTH_SENDER,
            metric_names::BATCH_SEND_VOLUME,
            total_amount,
            soroban_sdk::vec![
                &env,
                (dimension_names::SCHEME_ID, scheme_id.into_val(&env)),
                (dimension_names::TOKEN_ADDRESS, token.into_val(&env)),
            ],
        );
        emit_metric(
            &env,
            contract_ids::STEALTH_SENDER,
            metric_names::BATCH_SIZE,
            len as i128,
            soroban_sdk::vec![
                &env,
                (dimension_names::SCHEME_ID, scheme_id.into_val(&env)),
                (dimension_names::TOKEN_ADDRESS, token.into_val(&env)),
            ],
        );

        Ok(())
    }

    /// Bundle multiple stealth address announcements into a single Stellar op.
    ///
    /// Designed for fee-bumped sponsorship: `sponsor` authenticates the bundle
    /// at the transaction envelope level so the recorded `fee_account` on
    /// Horizon is the sponsor, and each `entry.sender` authenticates only its
    /// own entry. Each entry performs the same atomic send + announce as
    /// `send`/`batch_send`, so mid-bundle failure rolls the entire transaction
    /// back.
    ///
    /// `entries.len()` must be in `[1, SPONSORED_MAX_ENTRIES]` (call returns
    /// `LengthMismatch` if the list is empty and `BatchTooLarge` if the cap
    /// is exceeded). Per-entry token allow-list and protocol-fee behaviour are
    /// identical to `batch_send`'s single-token path.
    pub fn sponsored_announce(
        env: Env,
        sponsor: Address,
        entries: Vec<SponsoredEntry>,
    ) -> Result<(), SenderError> {
        sponsor.require_auth();

        let len = entries.len();
        if len == 0 {
            return Err(SenderError::LengthMismatch);
        }
        if len > SPONSORED_MAX_ENTRIES {
            return Err(SenderError::BatchTooLarge);
        }

        let announcer: Address = env
            .storage()
            .instance()
            .get(&DataKey::Announcer)
            .ok_or(SenderError::NotInitialized)?;

        // Extend instance TTL
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);

        // Check asset policy if configured
        let policy_address: Option<Address> =
            env.storage().instance().get(&DataKey::AssetPolicy);

        let mut total_amount: i128 = 0;

        // Dedupe signers: `Address::require_auth` is not idempotent — repeated
        // calls for the same address raise `AuthError::ExistingValue`. One sig
        // per signer key is also the right Stellar envelope semantics for a
        // fee-bumped bundle.
        let mut auth_seen: Vec<Address> = Vec::new(&env);
        auth_seen.push_back(sponsor.clone());

        for i in 0..len {
            let entry = entries.get(i).unwrap();

            if !auth_seen.contains(&entry.sender) {
                entry.sender.require_auth();
                auth_seen.push_back(entry.sender.clone());
            }

            if let Some(ref policy) = policy_address {
                if !asset_policy_client::check_asset(&env, policy, &entry.token) {
                    return Err(SenderError::TokenNotAllowed);
                }
            }

            let token_client = token::Client::new(&env, &entry.token);
            token_client.transfer(&entry.sender, &entry.stealth_address, &entry.amount);
            total_amount += entry.amount;

            announcer_client::announce(
                &env,
                &announcer,
                entry.scheme_id,
                &entry.stealth_address,
                &entry.ephemeral_pub_key,
                &entry.metadata,
            );
        }

        // Emit metric events for the whole bundle. Per-entry token dimensions
        // are heterogeneous, so we tag the metrics with the sponsor (the
        // observable fee source) instead of any single token address.
        emit_metric(
            &env,
            contract_ids::STEALTH_SENDER,
            metric_names::BATCH_SEND_COUNT,
            1,
            soroban_sdk::vec![
                &env,
                (dimension_names::TOKEN_ADDRESS, sponsor.into_val(&env)),
            ],
        );
        emit_metric(
            &env,
            contract_ids::STEALTH_SENDER,
            metric_names::BATCH_SEND_VOLUME,
            total_amount,
            soroban_sdk::vec![
                &env,
                (dimension_names::TOKEN_ADDRESS, sponsor.into_val(&env)),
            ],
        );
        emit_metric(
            &env,
            contract_ids::STEALTH_SENDER,
            metric_names::BATCH_SIZE,
            len as i128,
            soroban_sdk::vec![
                &env,
                (dimension_names::TOKEN_ADDRESS, sponsor.into_val(&env)),
            ],
        );

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
    use soroban_sdk::testutils::storage::Instance;
    use soroban_sdk::testutils::{Address as _, Ledger};
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
            let key = soroban_sdk::symbol_short!("count");
            let count: u32 = env.storage().instance().get(&key).unwrap_or(0);
            env.storage().instance().set(&key, &(count + 1));
        }

        pub fn count(env: Env) -> u32 {
            env.storage()
                .instance()
                .get(&soroban_sdk::symbol_short!("count"))
                .unwrap_or(0)
        }
    }

    #[test]
    fn test_sender_workflow() {
        let env = Env::default();
        env.mock_all_auths();

        // Configure test ledger to have large min_persistent_entry_ttl so helper contracts/balances do not expire
        env.ledger().with_mut(|li| {
            li.min_persistent_entry_ttl = 600000;
        });

        // 1. Deploy Mock Announcer
        let announcer_id = env.register(MockAnnouncer, ());

        // 2. Deploy StealthSenderContract
        let sender_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(&env, &sender_id);

        // 3. Register standard asset token contract
        let token_admin = Address::generate(&env);
        let token_id = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();
        let token_client = token::Client::new(&env, &token_id);

        // 4. Initialize StealthSender
        client.init(&announcer_id, &None, &None, &0);

        // Verify AlreadyInitialized error
        let init_res = client.try_init(&announcer_id, &None, &None, &0);
        assert_eq!(init_res, Err(Ok(SenderError::AlreadyInitialized)));

        // Setup transfer accounts and mint tokens
        let sender = Address::generate(&env);
        let stealth_address = Address::generate(&env);

        let token_admin_client = token::StellarAssetClient::new(&env, &token_id);
        token_admin_client.mint(&sender, &1000);

        assert_eq!(token_client.balance(&sender), 1000);
        assert_eq!(token_client.balance(&stealth_address), 0);

        // 5. Test send functionality
        let epk = BytesN::from_array(&env, &[1u8; 32]);
        let meta = Bytes::from_slice(&env, &[0u8; 1]);

        client.send(&sender, &token_id, &500, &1, &stealth_address, &epk, &meta);

        // Check balances
        assert_eq!(token_client.balance(&sender), 500);
        assert_eq!(token_client.balance(&stealth_address), 500);

        // 6. Test TTL extension behavior
        let initial_ttl = env.as_contract(&sender_id, || env.storage().instance().get_ttl());
        assert!(initial_ttl > 0);

        // Fast-forward sequence number to reduce TTL below the 17,280 threshold
        env.ledger().with_mut(|li| {
            li.sequence_number += 590000;
        });

        let reduced_ttl = env.as_contract(&sender_id, || env.storage().instance().get_ttl());
        assert!(reduced_ttl < initial_ttl);

        // Invoke send again to trigger TTL extension
        let stealth_address_2 = Address::generate(&env);
        client.send(
            &sender,
            &token_id,
            &100,
            &1,
            &stealth_address_2,
            &epk,
            &meta,
        );

        // Verify TTL is bumped back to max/extend_to value
        let bumped_ttl = env.as_contract(&sender_id, || env.storage().instance().get_ttl());
        assert!(bumped_ttl > reduced_ttl);
        assert_eq!(bumped_ttl, 518400); // Should be bumped to TTL_EXTEND_TO
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

        client.init(&announcer_id, &None, &None, &0);

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
        let sender_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(env, &sender_id);

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

        let sender_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(&env, &sender_id);

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

    fn setup_sponsored(
        env: &Env,
    ) -> (
        StealthSenderContractClient,
        Address,
        Address,
        Address,
        Address,
        BytesN<32>,
        Bytes,
    ) {
        env.mock_all_auths();

        let announcer_id = env.register(MockAnnouncer, ());
        let sender_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(env, &sender_id);
        client.init(&announcer_id, &None, &None, &0);

        let token_admin = Address::generate(env);
        let token_id = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();

        let sender = Address::generate(env);
        let stealth = Address::generate(env);

        let epk = BytesN::from_array(env, &[0xabu8; 32]);
        let meta = Bytes::from_slice(env, &[0x01]);

        (client, sender_id, token_id, sender, stealth, epk, meta)
    }

    #[test]
    fn test_sponsored_announce_single_entry() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _sender_id, token_id, sender, stealth, epk, meta) = setup_sponsored(&env);

        let asset = token::StellarAssetClient::new(&env, &token_id);
        asset.mint(&sender, &1_000);

        let sponsor = Address::generate(&env);
        let entries = soroban_sdk::vec![
            &env,
            SponsoredEntry {
                sender: sender.clone(),
                token: token_id.clone(),
                amount: 100,
                scheme_id: 1,
                stealth_address: stealth.clone(),
                ephemeral_pub_key: epk.clone(),
                metadata: meta.clone(),
            },
        ];

        client.sponsored_announce(&sponsor, &entries);

        let token_client = token::Client::new(&env, &token_id);
        assert_eq!(token_client.balance(&sender), 900);
        assert_eq!(token_client.balance(&stealth), 100);
    }

    #[test]
    fn test_sponsored_announce_at_cap_succeeds() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, _sender_id, token_id, _sender, _stealth, epk, meta) = setup_sponsored(&env);
        let asset = token::StellarAssetClient::new(&env, &token_id);

        let sponsor = Address::generate(&env);
        let per_entry_sender = Address::generate(&env);
        let total_mint = SPONSORED_MAX_ENTRIES as i128 * 100 + 1;
        asset.mint(&per_entry_sender, &total_mint);

        let mut entries: Vec<SponsoredEntry> = Vec::new(&env);
        for _ in 0..SPONSORED_MAX_ENTRIES {
            entries.push_back(SponsoredEntry {
                sender: per_entry_sender.clone(),
                token: token_id.clone(),
                amount: 100,
                scheme_id: 1,
                stealth_address: Address::generate(&env),
                ephemeral_pub_key: epk.clone(),
                metadata: meta.clone(),
            });
        }

        client.sponsored_announce(&sponsor, &entries);

        let token_client = token::Client::new(&env, &token_id);
        assert_eq!(
            token_client.balance(&per_entry_sender),
            1,
            "SPONSORED_MAX_ENTRIES transfers must be applied atomically",
        );
    }

    #[test]
    fn test_sponsored_announce_cap_exceeded() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, _sender_id, token_id, _sender, _stealth, epk, meta) = setup_sponsored(&env);

        let sponsor = Address::generate(&env);
        let bogus_sender = Address::generate(&env);

        // One over the cap must reject with BatchTooLarge and roll back.
        let mut entries: Vec<SponsoredEntry> = Vec::new(&env);
        for _ in 0..(SPONSORED_MAX_ENTRIES + 1) {
            entries.push_back(SponsoredEntry {
                sender: bogus_sender.clone(),
                token: token_id.clone(),
                amount: 1,
                scheme_id: 1,
                stealth_address: Address::generate(&env),
                ephemeral_pub_key: epk.clone(),
                metadata: meta.clone(),
            });
        }

        let res = client.try_sponsored_announce(&sponsor, &entries);
        assert_eq!(res, Err(Ok(SenderError::BatchTooLarge)));

        // Sanity: the rejected bundle must not have moved any tokens.
        let token_client = token::Client::new(&env, &token_id);
        assert_eq!(token_client.balance(&bogus_sender), 0);
    }

    #[test]
    fn test_sponsored_announce_empty_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let (client, _sender_id, _token_id, _sender, _stealth, _epk, _meta) =
            setup_sponsored(&env);

        let sponsor = Address::generate(&env);
        let entries: Vec<SponsoredEntry> = Vec::new(&env);

        let res = client.try_sponsored_announce(&sponsor, &entries);
        assert_eq!(res, Err(Ok(SenderError::LengthMismatch)));
    }

    #[test]
    fn test_sponsored_announce_not_initialized_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let sender_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(&env, &sender_id);

        let sponsor = Address::generate(&env);
        let bogus_sender = Address::generate(&env);
        let bogus_token = Address::generate(&env);

        let entries = soroban_sdk::vec![
            &env,
            SponsoredEntry {
                sender: bogus_sender,
                token: bogus_token,
                amount: 1,
                scheme_id: 1,
                stealth_address: Address::generate(&env),
                ephemeral_pub_key: BytesN::from_array(&env, &[0u8; 32]),
                metadata: Bytes::from_slice(&env, &[0u8; 1]),
            },
        ];

        let res = client.try_sponsored_announce(&sponsor, &entries);
        assert_eq!(res, Err(Ok(SenderError::NotInitialized)));
    }

    #[test]
    fn test_sponsored_announce_emits_one_announcement_per_entry() {
        // Wire the announcer/sender/token explicitly so we can ask the mock
        // contract how many announcements it received end-to-end.
        let env = Env::default();
        env.mock_all_auths();

        let announcer_id = env.register(MockAnnouncer, ());
        let mock_client = MockAnnouncerClient::new(&env, &announcer_id);
        assert_eq!(mock_client.count(), 0, "mock must start empty");

        let sender_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(&env, &sender_id);
        client.init(&announcer_id, &None, &None, &0);

        let token_admin = Address::generate(&env);
        let token_id = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();
        let asset = token::StellarAssetClient::new(&env, &token_id);
        let sender = Address::generate(&env);
        asset.mint(&sender, &10_000);

        let epk = BytesN::from_array(&env, &[0xabu8; 32]);
        let meta = Bytes::from_slice(&env, &[0x01]);

        let sponsor = Address::generate(&env);
        let entries = soroban_sdk::vec![
            &env,
            SponsoredEntry {
                sender: sender.clone(),
                token: token_id.clone(),
                amount: 100,
                scheme_id: 1,
                stealth_address: Address::generate(&env),
                ephemeral_pub_key: epk.clone(),
                metadata: meta.clone(),
            },
            SponsoredEntry {
                sender: sender.clone(),
                token: token_id.clone(),
                amount: 200,
                scheme_id: 1,
                stealth_address: Address::generate(&env),
                ephemeral_pub_key: epk.clone(),
                metadata: meta.clone(),
            },
        ];

        client.sponsored_announce(&sponsor, &entries);

        assert_eq!(
            mock_client.count(),
            2,
            "two entries should drive exactly two cross-contract announces",
        );
    }

    #[test]
    fn test_sponsored_announce_token_not_allowed() {
        let env = Env::default();
        env.mock_all_auths();

        let announcer_id = env.register(MockAnnouncer, ());
        let policy_admin = Address::generate(&env);
        let policy_id = env.register(wraith_asset_policy::WraithAssetPolicy, ());
        let policy_client = wraith_asset_policy::WraithAssetPolicyClient::new(&env, &policy_id);
        policy_client.init(&policy_admin, &soroban_sdk::Vec::new(&env));

        let sender_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(&env, &sender_id);
        client.init(&announcer_id, &Some(policy_id), &None, &0);

        let token_admin = Address::generate(&env);
        let blocked_token = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();

        let sponsor = Address::generate(&env);
        let blocked_sender = Address::generate(&env);
        let entries = soroban_sdk::vec![
            &env,
            SponsoredEntry {
                sender: blocked_sender.clone(),
                token: blocked_token.clone(),
                amount: 1,
                scheme_id: 1,
                stealth_address: Address::generate(&env),
                ephemeral_pub_key: BytesN::from_array(&env, &[0u8; 32]),
                metadata: Bytes::from_slice(&env, &[0u8; 1]),
            },
        ];

        let res = client.try_sponsored_announce(&sponsor, &entries);
        assert_eq!(res, Err(Ok(SenderError::TokenNotAllowed)));

        // No announce event should have been emitted and no tokens moved.
        let token_client = token::Client::new(&env, &blocked_token);
        assert_eq!(token_client.balance(&blocked_sender), 0);
    }

    #[test]
    fn test_sponsored_announce_mid_bundle_failure_rolls_back() {
        // Mint only enough balance for entry 0; entry 1 must fail and roll entry 0 back.
        let env = Env::default();
        env.mock_all_auths();
        let (client, _sender_id, token_id, _sender, _stealth, epk, meta) =
            setup_sponsored(&env);

        let asset = token::StellarAssetClient::new(&env, &token_id);
        let survivor = Address::generate(&env);
        asset.mint(&survivor, &500);

        let broke_sender = Address::generate(&env); // SAC starts broke_sender at 0

        let sponsor = Address::generate(&env);
        let survivor_stealth = Address::generate(&env);
        let entries = soroban_sdk::vec![
            &env,
            SponsoredEntry {
                sender: survivor.clone(),
                token: token_id.clone(),
                amount: 200,
                scheme_id: 1,
                stealth_address: survivor_stealth.clone(),
                ephemeral_pub_key: epk.clone(),
                metadata: meta.clone(),
            },
            SponsoredEntry {
                sender: broke_sender.clone(),
                token: token_id.clone(),
                amount: 999_999,
                scheme_id: 1,
                stealth_address: Address::generate(&env),
                ephemeral_pub_key: epk.clone(),
                metadata: meta.clone(),
            },
        ];

        let res = client.try_sponsored_announce(&sponsor, &entries);
        assert!(
            res.is_err(),
            "second entry's insufficient-balance transfer must revert the bundle",
        );

        // Entry 0's successful transfer must have been rolled back.
        let token_client = token::Client::new(&env, &token_id);
        assert_eq!(
            token_client.balance(&survivor),
            500,
            "first entry's transfer must be rolled back when a later entry fails",
        );
        assert_eq!(token_client.balance(&survivor_stealth), 0);
    }
}
