//! Production-hardening acceptance tests for stealth-batch-sender (issue #155).
//!
//! Covers: init idempotency + AlreadyInitialized, pause/unpause + is_paused,
//! paused-call rejection, the full typed BatchSenderError surface (including
//! asset-policy rejection), and signer-rotation happy + adversarial paths.

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::StellarAssetClient,
    vec, Address, Bytes, Env,
};
use stealth_announcer::StealthAnnouncerContract;
use stealth_batch_sender::{
    BatchSenderError, StealthBatchSender, StealthBatchSenderClient, Transfer, MAX_BATCH_SIZE,
};

fn dummy_pub_key(env: &Env) -> Bytes {
    Bytes::from_slice(env, &[0x02u8; 32])
}

fn dummy_metadata(env: &Env) -> Bytes {
    Bytes::from_slice(env, &[0x2Au8])
}

fn real_announcer(env: &Env) -> Address {
    env.register(StealthAnnouncerContract, ())
}

fn create_token(env: &Env, admin: &Address) -> (Address, StellarAssetClient<'static>) {
    let contract_id = env.register_stellar_asset_contract_v2(admin.clone());
    (
        contract_id.address(),
        StellarAssetClient::new(env, &contract_id.address()),
    )
}

fn deploy(env: &Env) -> StealthBatchSenderClient<'static> {
    let contract_id = env.register(StealthBatchSender, ());
    StealthBatchSenderClient::new(env, &contract_id)
}

// ─── Minimal asset-policy mocks used only by the AssetNotAllowed tests ────
// Each lives in its own module: soroban-sdk's #[contractimpl] macro
// generates helper items named after the method, which collide across
// types when they share a module scope.

mod deny_all_policy {
    use soroban_sdk::{contract, contractimpl, Address, Env};

    #[contract]
    pub struct DenyAllPolicy;

    #[contractimpl]
    impl DenyAllPolicy {
        pub fn check_asset(_env: Env, _token: Address) -> bool {
            false
        }
    }
}
use deny_all_policy::DenyAllPolicy;

mod allow_all_policy {
    use soroban_sdk::{contract, contractimpl, Address, Env};

    #[contract]
    pub struct AllowAllPolicy;

    #[contractimpl]
    impl AllowAllPolicy {
        pub fn check_asset(_env: Env, _token: Address) -> bool {
            true
        }
    }
}
use allow_all_policy::AllowAllPolicy;

// ─────────────────────────── init lifecycle ───────────────────────────

#[test]
fn init_is_idempotent() {
    let env = Env::default();
    env.mock_all_auths();

    let client = deploy(&env);
    let admin = Address::generate(&env);
    let announcer = Address::generate(&env);

    client.init(&admin, &announcer, &None);

    let result = client.try_init(&admin, &announcer, &None);
    assert_eq!(result, Err(Ok(BatchSenderError::AlreadyInitialized)));
}

#[test]
fn batch_send_requires_init() {
    let env = Env::default();
    env.mock_all_auths();

    let client = deploy(&env);
    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    token_admin.mint(&sender, &100);

    let transfers = vec![
        &env,
        Transfer {
            stealth_address: Address::generate(&env),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 100,
            metadata: dummy_metadata(&env),
        },
    ];

    let result = client.try_batch_send(&sender, &transfers, &token);
    assert_eq!(result, Err(Ok(BatchSenderError::NotInitialized)));
}

// ─────────────────────────── pause / unpause ───────────────────────────

#[test]
fn pause_unpause_by_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let client = deploy(&env);
    let admin = Address::generate(&env);
    let announcer = Address::generate(&env);
    client.init(&admin, &announcer, &None);

    assert!(!client.is_paused());
    client.pause(&admin);
    assert!(client.is_paused());
    client.unpause(&admin);
    assert!(!client.is_paused());
}

#[test]
#[should_panic(expected = "HostError")]
fn only_admin_can_pause() {
    let env = Env::default();
    env.mock_all_auths();

    let client = deploy(&env);
    let admin = Address::generate(&env);
    let announcer = Address::generate(&env);
    let attacker = Address::generate(&env);
    client.init(&admin, &announcer, &None);

    client.pause(&attacker);
}

#[test]
fn batch_send_rejected_when_paused() {
    let env = Env::default();
    env.mock_all_auths();

    let client = deploy(&env);
    let admin = Address::generate(&env);
    let announcer = Address::generate(&env);
    client.init(&admin, &announcer, &None);

    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    token_admin.mint(&sender, &1000);

    let transfers = vec![
        &env,
        Transfer {
            stealth_address: Address::generate(&env),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 100,
            metadata: dummy_metadata(&env),
        },
    ];

    client.pause(&admin);
    let result = client.try_batch_send(&sender, &transfers, &token);
    assert_eq!(result, Err(Ok(BatchSenderError::Paused)));

    // Balances unchanged.
    let token_client = soroban_sdk::token::Client::new(&env, &token);
    assert_eq!(token_client.balance(&sender), 1000);
}

#[test]
fn batch_send_allowed_after_unpause() {
    let env = Env::default();
    env.mock_all_auths();

    let client = deploy(&env);
    let admin = Address::generate(&env);
    let announcer = real_announcer(&env);
    client.init(&admin, &announcer, &None);

    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    token_admin.mint(&sender, &1000);
    let stealth = Address::generate(&env);

    let transfers = vec![
        &env,
        Transfer {
            stealth_address: stealth.clone(),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 100,
            metadata: dummy_metadata(&env),
        },
    ];

    client.pause(&admin);
    client.unpause(&admin);
    assert!(!client.is_paused());

    client.batch_send(&sender, &transfers, &token);

    let token_client = soroban_sdk::token::Client::new(&env, &token);
    assert_eq!(token_client.balance(&stealth), 100);
}

// ────────────────────────── typed-error surface ─────────────────────────

#[test]
fn empty_batch_returns_typed_error() {
    let env = Env::default();
    env.mock_all_auths();

    let client = deploy(&env);
    let admin = Address::generate(&env);
    let announcer = Address::generate(&env);
    client.init(&admin, &announcer, &None);

    let sender = Address::generate(&env);
    let (token, _) = create_token(&env, &admin);

    let result = client.try_batch_send(&sender, &soroban_sdk::Vec::new(&env), &token);
    assert_eq!(result, Err(Ok(BatchSenderError::EmptyBatch)));
}

#[test]
fn oversized_batch_returns_typed_error() {
    let env = Env::default();
    env.mock_all_auths();

    let client = deploy(&env);
    let admin = Address::generate(&env);
    let announcer = Address::generate(&env);
    client.init(&admin, &announcer, &None);

    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    token_admin.mint(&sender, &1_000_000);

    let mut transfers = soroban_sdk::Vec::new(&env);
    for _ in 0..=MAX_BATCH_SIZE {
        transfers.push_back(Transfer {
            stealth_address: Address::generate(&env),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 1,
            metadata: dummy_metadata(&env),
        });
    }

    let result = client.try_batch_send(&sender, &transfers, &token);
    assert_eq!(result, Err(Ok(BatchSenderError::BatchTooLarge)));
}

#[test]
fn non_positive_amount_returns_typed_error() {
    let env = Env::default();
    env.mock_all_auths();

    let client = deploy(&env);
    let admin = Address::generate(&env);
    let announcer = Address::generate(&env);
    client.init(&admin, &announcer, &None);

    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    token_admin.mint(&sender, &100);

    let transfers = vec![
        &env,
        Transfer {
            stealth_address: Address::generate(&env),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 0,
            metadata: dummy_metadata(&env),
        },
    ];

    let result = client.try_batch_send(&sender, &transfers, &token);
    assert_eq!(result, Err(Ok(BatchSenderError::NonPositiveAmount)));
}

#[test]
fn empty_ephemeral_key_returns_typed_error() {
    let env = Env::default();
    env.mock_all_auths();

    let client = deploy(&env);
    let admin = Address::generate(&env);
    let announcer = Address::generate(&env);
    client.init(&admin, &announcer, &None);

    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    token_admin.mint(&sender, &100);

    let transfers = vec![
        &env,
        Transfer {
            stealth_address: Address::generate(&env),
            ephemeral_pub_key: Bytes::new(&env),
            amount: 100,
            metadata: dummy_metadata(&env),
        },
    ];

    let result = client.try_batch_send(&sender, &transfers, &token);
    assert_eq!(result, Err(Ok(BatchSenderError::EmptyEphemeralKey)));
}

#[test]
fn asset_not_allowed_by_policy_returns_typed_error() {
    let env = Env::default();
    env.mock_all_auths();

    let client = deploy(&env);
    let admin = Address::generate(&env);
    let announcer = Address::generate(&env);
    let policy_id = env.register(DenyAllPolicy, ());
    client.init(&admin, &announcer, &Some(policy_id));

    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    token_admin.mint(&sender, &100);

    let transfers = vec![
        &env,
        Transfer {
            stealth_address: Address::generate(&env),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 100,
            metadata: dummy_metadata(&env),
        },
    ];

    let result = client.try_batch_send(&sender, &transfers, &token);
    assert_eq!(result, Err(Ok(BatchSenderError::AssetNotAllowed)));
}

#[test]
fn asset_allowed_by_policy_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let client = deploy(&env);
    let admin = Address::generate(&env);
    let announcer = real_announcer(&env);
    let policy_id = env.register(AllowAllPolicy, ());
    client.init(&admin, &announcer, &Some(policy_id));

    let sender = Address::generate(&env);
    let (token, token_admin) = create_token(&env, &admin);
    token_admin.mint(&sender, &100);
    let stealth = Address::generate(&env);

    let transfers = vec![
        &env,
        Transfer {
            stealth_address: stealth.clone(),
            ephemeral_pub_key: dummy_pub_key(&env),
            amount: 100,
            metadata: dummy_metadata(&env),
        },
    ];

    client.batch_send(&sender, &transfers, &token);

    let token_client = soroban_sdk::token::Client::new(&env, &token);
    assert_eq!(token_client.balance(&stealth), 100);
}

// ──────────────────────── signer-rotation: happy path ───────────────────

fn setup_multisig(env: &Env) -> (StealthBatchSenderClient<'static>, soroban_sdk::Vec<Address>) {
    let client = deploy(env);
    let admin = Address::generate(env);
    let announcer = Address::generate(env);
    client.init(&admin, &announcer, &None);

    let signers = vec![
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
fn rotation_requires_quorum_and_timelock() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, signers) = setup_multisig(&env);
    let new_signers = vec![&env, Address::generate(&env), Address::generate(&env)];

    client.propose_rotate_signers(&signers.get(0).unwrap(), &new_signers, &2);

    // Only 1 of 3 approvals so far (the proposer's).
    let result = client.try_execute_rotate_signers(&signers.get(0).unwrap());
    assert_eq!(result, Err(Ok(BatchSenderError::QuorumNotMet)));

    client.approve_rotate_signers(&signers.get(1).unwrap());
    client.approve_rotate_signers(&signers.get(2).unwrap());

    // Quorum met, but the 7-day timelock has not elapsed yet.
    let result = client.try_execute_rotate_signers(&signers.get(0).unwrap());
    assert_eq!(result, Err(Ok(BatchSenderError::TimelockNotElapsed)));

    env.ledger().with_mut(|li| {
        li.timestamp += stealth_batch_sender::ROTATION_TIMELOCK_SECS;
    });

    client.execute_rotate_signers(&signers.get(0).unwrap());

    assert_eq!(client.signers(), new_signers);
    assert_eq!(client.threshold(), 2);
    assert!(client.pending_rotation().is_none());
}

#[test]
fn cancelled_rotation_clears_state() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, signers) = setup_multisig(&env);
    let new_signers = vec![&env, Address::generate(&env), Address::generate(&env)];

    client.propose_rotate_signers(&signers.get(0).unwrap(), &new_signers, &2);
    client.approve_rotate_signers(&signers.get(1).unwrap());
    client.cancel_rotate_signers(&signers.get(2).unwrap());

    assert!(client.pending_rotation().is_none());
    assert_eq!(client.signers(), signers);
    assert_eq!(client.threshold(), 3);

    let result = client.try_approve_rotate_signers(&signers.get(3).unwrap());
    assert_eq!(result, Err(Ok(BatchSenderError::NoPendingRotation)));
}

// ──────────────────────── signer-rotation: adversarial ──────────────────

#[test]
fn non_signer_cannot_propose_or_approve() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, signers) = setup_multisig(&env);
    let outsider = Address::generate(&env);
    let new_signers = vec![&env, Address::generate(&env), Address::generate(&env)];

    let result = client.try_propose_rotate_signers(&outsider, &new_signers, &2);
    assert_eq!(result, Err(Ok(BatchSenderError::NotSigner)));

    client.propose_rotate_signers(&signers.get(0).unwrap(), &new_signers, &2);
    let result = client.try_approve_rotate_signers(&outsider);
    assert_eq!(result, Err(Ok(BatchSenderError::NotSigner)));
}

#[test]
fn duplicate_approval_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, signers) = setup_multisig(&env);
    let new_signers = vec![&env, Address::generate(&env), Address::generate(&env)];

    client.propose_rotate_signers(&signers.get(0).unwrap(), &new_signers, &2);

    // Proposer's approval was already recorded automatically.
    let result = client.try_approve_rotate_signers(&signers.get(0).unwrap());
    assert_eq!(result, Err(Ok(BatchSenderError::AlreadyApprovedRotation)));
}

#[test]
fn only_one_rotation_pending_at_a_time() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, signers) = setup_multisig(&env);
    let new_signers = vec![&env, Address::generate(&env), Address::generate(&env)];

    client.propose_rotate_signers(&signers.get(0).unwrap(), &new_signers, &2);

    let result = client.try_propose_rotate_signers(&signers.get(1).unwrap(), &new_signers, &2);
    assert_eq!(result, Err(Ok(BatchSenderError::RotationAlreadyPending)));
}

#[test]
fn early_execution_rejected_before_timelock() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, signers) = setup_multisig(&env);
    let new_signers = vec![&env, Address::generate(&env), Address::generate(&env)];

    client.propose_rotate_signers(&signers.get(0).unwrap(), &new_signers, &2);
    client.approve_rotate_signers(&signers.get(1).unwrap());
    client.approve_rotate_signers(&signers.get(2).unwrap());

    let result = client.try_execute_rotate_signers(&signers.get(0).unwrap());
    assert_eq!(result, Err(Ok(BatchSenderError::TimelockNotElapsed)));
}

#[test]
fn invalid_signer_set_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, signers) = setup_multisig(&env);
    let new_signers = vec![&env, Address::generate(&env), Address::generate(&env)];

    // Zero threshold is unreachable.
    let result = client.try_propose_rotate_signers(&signers.get(0).unwrap(), &new_signers, &0);
    assert_eq!(result, Err(Ok(BatchSenderError::InvalidThreshold)));

    // Threshold greater than the proposed signer count is unreachable.
    let result = client.try_propose_rotate_signers(&signers.get(0).unwrap(), &new_signers, &3);
    assert_eq!(result, Err(Ok(BatchSenderError::InvalidThreshold)));

    assert!(client.pending_rotation().is_none());
}

#[test]
fn init_multisig_rejects_invalid_threshold() {
    let env = Env::default();
    env.mock_all_auths();

    let client = deploy(&env);
    let admin = Address::generate(&env);
    let announcer = Address::generate(&env);
    client.init(&admin, &announcer, &None);

    let signers = vec![&env, Address::generate(&env), Address::generate(&env)];

    let result = client.try_init_multisig(&signers, &0);
    assert_eq!(result, Err(Ok(BatchSenderError::InvalidThreshold)));

    let result = client.try_init_multisig(&signers, &3);
    assert_eq!(result, Err(Ok(BatchSenderError::InvalidThreshold)));
}
