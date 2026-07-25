#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, Bytes, BytesN, Env,
    IntoVal, Vec,
};
use wraith_metrics::{contract_ids, dimension_names, emit_metric, metric_names};

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
    /// Pause admin address.
    Admin,
    /// Whether the contract is paused.
    Paused,
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
    /// The contract is paused.
    Paused = 6,
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
        admin: Address,
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
        env.storage().instance().set(&DataKey::Admin, &admin);

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

    /// Pause the contract — admin only.
    /// Prevents all sends and batch_sends while paused.
    pub fn pause(env: Env, caller: Address) -> Result<(), SenderError> {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("admin not set");
        if caller != admin {
            panic!("unauthorized: only admin can pause");
        }
        env.storage()
            .instance()
            .set(&DataKey::Paused, &true);
        env.events()
            .publish((soroban_sdk::symbol_short!("paused"),), (caller,));
        Ok(())
    }

    /// Unpause the contract — admin only.
    pub fn unpause(env: Env, caller: Address) -> Result<(), SenderError> {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("admin not set");
        if caller != admin {
            panic!("unauthorized: only admin can unpause");
        }
        env.storage()
            .instance()
            .set(&DataKey::Paused, &false);
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

    /// Require the contract is not paused. Call at the top of every
    /// state-mutating entrypoint that should be disabled during an incident.
    fn require_not_paused(env: &Env) -> Result<(), SenderError> {
        if env
            .storage()
            .instance()
            .get::<_, bool>(&DataKey::Paused)
            .unwrap_or(false)
        {
            return Err(SenderError::Paused);
        }
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
        Self::require_not_paused(&env)?;
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
        Self::require_not_paused(&env)?;
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

        let admin = Address::generate(&env);

        // 4. Initialize StealthSender
        client.init(&announcer_id, &None, &None, &0, &admin);

        // Verify AlreadyInitialized error
        let init_res = client.try_init(&announcer_id, &None, &None, &0, &admin);
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

        let admin = Address::generate(&env);

        let token_admin = Address::generate(&env);
        let token_id = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();
        let token_client = token::Client::new(&env, &token_id);

        client.init(&announcer_id, &None, &None, &0, &admin);

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

    // ── Pause / unpause tests ──────────────────────────────────────────────

    #[test]
    fn test_pause_by_admin() {
        let env = Env::default();
        env.mock_all_auths();

        let announcer_id = env.register(MockAnnouncer, ());
        let sender_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(&env, &sender_id);

        let admin = Address::generate(&env);
        client.init(&announcer_id, &None, &None, &0, &admin);

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
    fn test_send_rejected_when_paused() {
        let env = Env::default();
        env.mock_all_auths();

        // Configure test ledger to have large min_persistent_entry_ttl
        env.ledger().with_mut(|li| {
            li.min_persistent_entry_ttl = 600000;
        });

        let announcer_id = env.register(MockAnnouncer, ());
        let sender_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(&env, &sender_id);

        let admin = Address::generate(&env);
        client.init(&announcer_id, &None, &None, &0, &admin);

        let token_admin = Address::generate(&env);
        let token_id = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();
        let token_client = soroban_sdk::token::Client::new(&env, &token_id);

        let sender = Address::generate(&env);
        let token_admin_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
        token_admin_client.mint(&sender, &1000);

        let stealth_address = Address::generate(&env);
        let epk = BytesN::from_array(&env, &[1u8; 32]);
        let meta = Bytes::from_slice(&env, &[0u8; 1]);

        // Pause the contract
        client.pause(&admin);
        assert!(client.is_paused());

        // Send should be rejected
        let result = client.try_send(
            &sender, &token_id, &500, &1, &stealth_address, &epk, &meta,
        );
        assert_eq!(result, Err(Ok(SenderError::Paused)));

        // Balances unchanged
        assert_eq!(token_client.balance(&sender), 1000);
        assert_eq!(token_client.balance(&stealth_address), 0);
    }

    #[test]
    fn test_batch_send_rejected_when_paused() {
        let env = Env::default();
        env.mock_all_auths();

        let announcer_id = env.register(MockAnnouncer, ());
        let sender_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(&env, &sender_id);

        let admin = Address::generate(&env);
        client.init(&announcer_id, &None, &None, &0, &admin);

        let token_admin = Address::generate(&env);
        let token_id = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();
        let token_client = soroban_sdk::token::Client::new(&env, &token_id);

        let sender = Address::generate(&env);
        let token_admin_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
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

        // Pause
        client.pause(&admin);
        assert!(client.is_paused());

        let result = client.try_batch_send(
            &sender, &token_id, &1, &addresses, &epks, &metadatas, &amounts,
        );
        assert_eq!(result, Err(Ok(SenderError::Paused)));

        // Balances unchanged
        assert_eq!(token_client.balance(&sender), 2000);
        assert_eq!(token_client.balance(&stealth_addr_1), 0);
        assert_eq!(token_client.balance(&stealth_addr_2), 0);
    }

    #[test]
    #[should_panic(expected = "HostError")]
    fn test_admin_only_can_pause() {
        let env = Env::default();
        env.mock_all_auths();

        let announcer_id = env.register(MockAnnouncer, ());
        let sender_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(&env, &sender_id);

        let admin = Address::generate(&env);
        let attacker = Address::generate(&env);
        client.init(&announcer_id, &None, &None, &0, &admin);

        // Non-admin cannot pause (panic expected)
        client.pause(&attacker);
    }

    #[test]
    fn test_send_allowed_after_unpause() {
        let env = Env::default();
        env.mock_all_auths();

        env.ledger().with_mut(|li| {
            li.min_persistent_entry_ttl = 600000;
        });

        let announcer_id = env.register(MockAnnouncer, ());
        let sender_id = env.register(StealthSenderContract, ());
        let client = StealthSenderContractClient::new(&env, &sender_id);

        let admin = Address::generate(&env);
        client.init(&announcer_id, &None, &None, &0, &admin);

        let token_admin = Address::generate(&env);
        let token_id = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();
        let token_client = soroban_sdk::token::Client::new(&env, &token_id);

        let sender = Address::generate(&env);
        let token_admin_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
        token_admin_client.mint(&sender, &1000);

        let stealth_address = Address::generate(&env);
        let epk = BytesN::from_array(&env, &[1u8; 32]);
        let meta = Bytes::from_slice(&env, &[0u8; 1]);

        // Pause then unpause
        client.pause(&admin);
        client.unpause(&admin);
        assert!(!client.is_paused());

        // Send should succeed
        client.send(&sender, &token_id, &500, &1, &stealth_address, &epk, &meta);
        assert_eq!(token_client.balance(&sender), 500);
        assert_eq!(token_client.balance(&stealth_address), 500);
    }
}
