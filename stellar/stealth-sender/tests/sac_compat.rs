//! SAC compatibility test harness for stealth-sender.
//!
//! Each test exercises one asset variant against the full send() flow and
//! asserts the expected outcome: success, transfer failure, or post-receipt
//! issuer action that defeats unlinkability.
//!
//! Mock contracts live in tests/mocks/ and mirror real SAC flag semantics.

mod mocks;

use mocks::{
    token_auth_required::AuthRequiredToken,
    token_auth_revocable::AuthRevocableToken,
    token_clawback::ClawbackToken,
    token_fee::FeeToken,
    token_immutable_auth_required::ImmutableAuthRequiredToken,
    token_immutable_safe::ImmutableSafeToken,
    token_standard::StandardToken,
};
use soroban_sdk::{
    testutils::{Address as _},
    Address, Bytes, BytesN, Env,
};

// ── helpers ──────────────────────────────────────────────────────────────────

/// Minimal announcer: just emits an event so the sender can call it.
mod announcer {
    use soroban_sdk::{contract, contractimpl, symbol_short, Address, Bytes, BytesN, Env};

    #[contract]
    pub struct Announcer;

    #[contractimpl]
    impl Announcer {
        pub fn announce(
            env: Env,
            _scheme_id: u32,
            stealth_address: Address,
            _ephemeral_pub_key: BytesN<32>,
            _metadata: Bytes,
        ) {
            env.events().publish(
                (symbol_short!("announce"), stealth_address),
                (),
            );
        }
    }
}

use announcer::Announcer;

struct Harness {
    env: Env,
    sender_id: Address,
    announcer_id: Address,
    sender: Address,
    stealth: Address,
    epk: BytesN<32>,
    meta: Bytes,
}

impl Harness {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let announcer_id = env.register(Announcer, ());
        let sender_id = env.register(StealthSenderContract, ());
        let sender_client =
            stealth_sender::StealthSenderContractClient::new(&env, &sender_id);
        sender_client.init(&announcer_id);

        let sender = Address::generate(&env);
        let stealth = Address::generate(&env);
        let epk = BytesN::from_array(&env, &[0xabu8; 32]);
        let meta = Bytes::from_slice(&env, &[0x01]);

        Harness {
            env,
            sender_id,
            announcer_id,
            sender,
            stealth,
            epk,
            meta,
        }
    }

    fn sender_client(&self) -> stealth_sender::StealthSenderContractClient {
        stealth_sender::StealthSenderContractClient::new(&self.env, &self.sender_id)
    }

    /// Assert that exactly one announcement event was emitted.
    fn assert_announced(&self) {
        let events = self.env.events().all();
        let announced = events.iter().any(|e| e.0 == self.announcer_id);
        assert!(announced, "expected announcement event, none found");
    }

    /// Assert that no announcement event was emitted.
    fn assert_not_announced(&self) {
        let events = self.env.events().all();
        let announced = events.iter().any(|e| e.0 == self.announcer_id);
        assert!(!announced, "unexpected announcement event found");
    }
}

// ── 1. Native XLM (simulated via standard token — no admin, no clawback) ────

#[test]
fn native_xlm_send_succeeds() {
    let h = Harness::new();
    let token_id = h.env.register(StandardToken, ());
    StandardToken::mint(&h.env, &h.sender, 1_000);

    h.sender_client()
        .send(&h.sender, &token_id, &500, &1, &h.stealth, &h.epk, &h.meta)
        .unwrap();

    assert_eq!(
        mocks::token_standard::StandardToken::balance(h.env.clone(), h.stealth.clone()),
        500
    );
    h.assert_announced();
}

// ── 2. Standard issued asset (no flags) ──────────────────────────────────────

#[test]
fn standard_issued_send_succeeds() {
    let h = Harness::new();
    let token_id = h.env.register(StandardToken, ());
    StandardToken::mint(&h.env, &h.sender, 1_000);

    h.sender_client()
        .send(&h.sender, &token_id, &500, &1, &h.stealth, &h.epk, &h.meta)
        .unwrap();

    assert_eq!(
        mocks::token_standard::StandardToken::balance(h.env.clone(), h.stealth.clone()),
        500
    );
    h.assert_announced();
}

// ── 3. AUTH_REQUIRED — stealth address not pre-authorized ────────────────────

#[test]
fn auth_required_send_fails_without_authorization() {
    let h = Harness::new();
    let admin = Address::generate(&h.env);
    let token_id = h.env.register(AuthRequiredToken, ());
    let token_client =
        mocks::token_auth_required::AuthRequiredTokenClient::new(&h.env, &token_id);
    token_client.init(&admin);
    AuthRequiredToken::mint(&h.env, &h.sender, 1_000);

    // Stealth address is NOT authorized — transfer must fail.
    let result = h.sender_client().try_send(
        &h.sender, &token_id, &500, &1, &h.stealth, &h.epk, &h.meta,
    );
    assert!(result.is_err(), "expected transfer to fail for unauthorized stealth address");
    h.assert_not_announced();
}

// ── 4. AUTH_REQUIRED — stealth address pre-authorized ────────────────────────

#[test]
fn auth_required_send_succeeds_when_pre_authorized() {
    let h = Harness::new();
    let admin = Address::generate(&h.env);
    let token_id = h.env.register(AuthRequiredToken, ());
    let token_client =
        mocks::token_auth_required::AuthRequiredTokenClient::new(&h.env, &token_id);
    token_client.init(&admin);
    AuthRequiredToken::mint(&h.env, &h.sender, 1_000);

    // Pre-authorize the stealth address.
    token_client.set_authorized(&admin, &h.stealth, &true);

    h.sender_client()
        .send(&h.sender, &token_id, &500, &1, &h.stealth, &h.epk, &h.meta)
        .unwrap();

    assert_eq!(
        mocks::token_auth_required::AuthRequiredToken::balance(h.env.clone(), h.stealth.clone()),
        500
    );
    h.assert_announced();
}

// ── 5. AUTH_REVOCABLE — transfer succeeds, but issuer can freeze post-receipt ─

#[test]
fn auth_revocable_send_succeeds_then_issuer_freezes() {
    let h = Harness::new();
    let admin = Address::generate(&h.env);
    let token_id = h.env.register(AuthRevocableToken, ());
    let token_client =
        mocks::token_auth_revocable::AuthRevocableTokenClient::new(&h.env, &token_id);
    token_client.init(&admin);
    AuthRevocableToken::mint(&h.env, &h.sender, 1_000);

    // Send succeeds — no auth-required flag.
    h.sender_client()
        .send(&h.sender, &token_id, &500, &1, &h.stealth, &h.epk, &h.meta)
        .unwrap();

    assert_eq!(
        mocks::token_auth_revocable::AuthRevocableToken::balance(h.env.clone(), h.stealth.clone()),
        500
    );
    h.assert_announced();

    // Issuer revokes authorization from the stealth address post-receipt.
    // This freezes the balance — the recipient can no longer spend.
    token_client.set_authorized(&admin, &h.stealth, &false);
    assert!(
        !mocks::token_auth_revocable::AuthRevocableToken::is_authorized(&h.env, &h.stealth),
        "stealth address should be frozen after revocation"
    );
    // AUDIT NOTE: funds arrived and announcement was emitted, but the recipient
    // is now unable to withdraw. Unlinkability is intact but funds are frozen.
}

// ── 6. AUTH_CLAWBACK_ENABLED — transfer succeeds, issuer claws back ──────────

#[test]
fn clawback_send_succeeds_then_issuer_claws_back() {
    let h = Harness::new();
    let admin = Address::generate(&h.env);
    let token_id = h.env.register(ClawbackToken, ());
    let token_client =
        mocks::token_clawback::ClawbackTokenClient::new(&h.env, &token_id);
    token_client.init(&admin);
    ClawbackToken::mint(&h.env, &h.sender, 1_000);

    // Send succeeds.
    h.sender_client()
        .send(&h.sender, &token_id, &500, &1, &h.stealth, &h.epk, &h.meta)
        .unwrap();

    assert_eq!(
        mocks::token_clawback::ClawbackToken::balance(h.env.clone(), h.stealth.clone()),
        500
    );
    h.assert_announced();

    // Issuer claws back the full amount from the stealth address.
    token_client.clawback(&admin, &h.stealth, &500);

    assert_eq!(
        mocks::token_clawback::ClawbackToken::balance(h.env.clone(), h.stealth.clone()),
        0,
        "clawback must drain the stealth address balance"
    );
    // AUDIT NOTE: announcement was emitted, transfer appeared to succeed, but
    // the issuer reversed it. The recipient's withdrawal will fail. This
    // directly defeats the unlinkability guarantee.
}

// ── 7. AUTH_IMMUTABLE (safe) — behaves like standard ─────────────────────────

#[test]
fn immutable_safe_send_succeeds() {
    let h = Harness::new();
    let token_id = h.env.register(ImmutableSafeToken, ());
    ImmutableSafeToken::mint(&h.env, &h.sender, 1_000);

    h.sender_client()
        .send(&h.sender, &token_id, &500, &1, &h.stealth, &h.epk, &h.meta)
        .unwrap();

    assert_eq!(
        mocks::token_immutable_safe::ImmutableSafeToken::balance(h.env.clone(), h.stealth.clone()),
        500
    );
    h.assert_announced();
}

// ── 8. AUTH_IMMUTABLE + AUTH_REQUIRED — permanently broken ───────────────────

#[test]
fn immutable_auth_required_send_fails_permanently() {
    let h = Harness::new();
    let admin = Address::generate(&h.env);
    let token_id = h.env.register(ImmutableAuthRequiredToken, ());
    let token_client =
        mocks::token_immutable_auth_required::ImmutableAuthRequiredTokenClient::new(
            &h.env, &token_id,
        );
    token_client.init(&admin);
    ImmutableAuthRequiredToken::mint(&h.env, &h.sender, 1_000);

    // No pre-authorization — transfer must fail.
    let result = h.sender_client().try_send(
        &h.sender, &token_id, &500, &1, &h.stealth, &h.epk, &h.meta,
    );
    assert!(result.is_err(), "immutable+auth-required must always fail for unknown stealth addresses");
    h.assert_not_announced();
    // AUDIT NOTE: because AUTH_IMMUTABLE prevents flag removal, this asset
    // can never be used with stealth flows without per-address issuer approval.
}

// ── 9. Custom Soroban token with fee — announced amount ≠ received amount ────

#[test]
fn fee_token_send_succeeds_but_amount_mismatch() {
    let h = Harness::new();
    let treasury = Address::generate(&h.env);
    let token_id = h.env.register(FeeToken, ());
    let token_client = mocks::token_fee::FeeTokenClient::new(&h.env, &token_id);
    token_client.init(&treasury);
    FeeToken::mint(&h.env, &h.sender, 1_000);

    let send_amount: i128 = 500;
    h.sender_client()
        .send(
            &h.sender,
            &token_id,
            &send_amount,
            &1,
            &h.stealth,
            &h.epk,
            &h.meta,
        )
        .unwrap();

    let received =
        mocks::token_fee::FeeToken::balance(h.env.clone(), h.stealth.clone());
    let fee = send_amount * 100 / 10_000; // 1%
    assert_eq!(
        received,
        send_amount - fee,
        "stealth address received less than announced amount"
    );
    h.assert_announced();
    // AUDIT NOTE: the announcement records `amount = 500` but the stealth
    // address only holds 495. Scanners that rely on the announced amount to
    // verify receipt will see a discrepancy. This is a correctness issue, not
    // a security issue, but it breaks the scanning assumption.
}
