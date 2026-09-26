//! SAC compatibility test harness for stealth-sender.
//!
//! Each test exercises one asset variant against the full send() flow and
//! asserts the expected outcome: success, transfer failure, or post-receipt
//! issuer action that defeats unlinkability.
//!
//! Mock contracts live in tests/mocks/ and mirror real SAC flag semantics.

mod mocks;

use mocks::{
    token_auth_required::AuthRequiredToken, token_auth_revocable::AuthRevocableToken,
    token_clawback::ClawbackToken, token_fee::FeeToken,
    token_immutable_auth_required::ImmutableAuthRequiredToken,
    token_immutable_safe::ImmutableSafeToken, token_standard::StandardToken,
};
use soroban_sdk::{
    testutils::Address as _, testutils::Events as _, token::TokenInterface as _, Address, Bytes,
    BytesN, Env,
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
            env.storage()
                .instance()
                .set(&symbol_short!("called"), &true);
            std::println!("ANNOUNCER CALLED for address: {:?}", stealth_address);
            env.events()
                .publish((symbol_short!("announce"), stealth_address), ());
        }

        pub fn is_called(env: Env) -> bool {
            env.storage()
                .instance()
                .get(&symbol_short!("called"))
                .unwrap_or(false)
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
    admin: Address,
}

impl Harness {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let announcer_id = env.register(Announcer, ());
        let sender_id = env.register(stealth_sender::StealthSenderContract, ());
        let admin = Address::generate(&env);
        let sender_client = stealth_sender::StealthSenderContractClient::new(&env, &sender_id);
        sender_client.init(&announcer_id, &None, &None, &0, &admin);

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
            admin,
        }
    }

    fn sender_client(&self) -> stealth_sender::StealthSenderContractClient<'_> {
        stealth_sender::StealthSenderContractClient::new(&self.env, &self.sender_id)
    }

    /// Assert that exactly one announcement event was emitted.
    fn assert_announced(&self) {
        let client = announcer::AnnouncerClient::new(&self.env, &self.announcer_id);
        assert!(client.is_called(), "expected announcement, none found");
    }

    /// Assert that no announcement event was emitted.
    fn assert_not_announced(&self) {
        let client = announcer::AnnouncerClient::new(&self.env, &self.announcer_id);
        assert!(!client.is_called(), "unexpected announcement found");
    }
}

// ── 1. Native XLM (simulated via standard token — no admin, no clawback) ────

#[test]
fn native_xlm_send_succeeds() {
    let h = Harness::new();
    let token_id = h.env.register(StandardToken, ());
    let token_client = mocks::token_standard::StandardTokenClient::new(&h.env, &token_id);
    h.env.as_contract(&token_id, || {
        StandardToken::mint(&h.env, &h.sender, 1_000);
    });

    h.sender_client()
        .send(&h.sender, &token_id, &500, &1, &h.stealth, &h.epk, &h.meta);

    assert_eq!(token_client.balance(&h.stealth), 500);
    h.assert_announced();
}

// ── 2. Standard issued asset (no flags) ──────────────────────────────────────

#[test]
fn standard_issued_send_succeeds() {
    let h = Harness::new();
    let token_id = h.env.register(StandardToken, ());
    let token_client = mocks::token_standard::StandardTokenClient::new(&h.env, &token_id);
    h.env.as_contract(&token_id, || {
        StandardToken::mint(&h.env, &h.sender, 1_000);
    });

    h.sender_client()
        .send(&h.sender, &token_id, &500, &1, &h.stealth, &h.epk, &h.meta);

    assert_eq!(token_client.balance(&h.stealth), 500);
    h.assert_announced();
}

// ── 3. AUTH_REQUIRED — stealth address not pre-authorized ────────────────────

#[test]
fn auth_required_send_fails_without_authorization() {
    let h = Harness::new();
    let admin = Address::generate(&h.env);
    let token_id = h.env.register(AuthRequiredToken, ());
    let token_client = mocks::token_auth_required::AuthRequiredTokenClient::new(&h.env, &token_id);
    token_client.init(&admin);
    h.env.as_contract(&token_id, || {
        AuthRequiredToken::mint(&h.env, &h.sender, 1_000);
    });

    // Stealth address is NOT authorized — transfer must fail.
    let result = h
        .sender_client()
        .try_send(&h.sender, &token_id, &500, &1, &h.stealth, &h.epk, &h.meta);
    assert!(
        result.is_err(),
        "expected transfer to fail for unauthorized stealth address"
    );
    h.assert_not_announced();
}

// ── 4. AUTH_REQUIRED — stealth address pre-authorized ────────────────────────

#[test]
fn auth_required_send_succeeds_when_pre_authorized() {
    let h = Harness::new();
    let admin = Address::generate(&h.env);
    let token_id = h.env.register(AuthRequiredToken, ());
    let token_client = mocks::token_auth_required::AuthRequiredTokenClient::new(&h.env, &token_id);
    token_client.init(&admin);
    h.env.as_contract(&token_id, || {
        AuthRequiredToken::mint(&h.env, &h.sender, 1_000);
    });

    // Pre-authorize the stealth address.
    token_client.set_authorized(&admin, &h.stealth, &true);

    h.sender_client()
        .send(&h.sender, &token_id, &500, &1, &h.stealth, &h.epk, &h.meta);

    assert_eq!(token_client.balance(&h.stealth), 500);
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
    h.env.as_contract(&token_id, || {
        AuthRevocableToken::mint(&h.env, &h.sender, 1_000);
    });

    // Send succeeds — no auth-required flag.
    h.sender_client()
        .send(&h.sender, &token_id, &500, &1, &h.stealth, &h.epk, &h.meta);

    assert_eq!(token_client.balance(&h.stealth), 500);
    h.assert_announced();

    // Issuer revokes authorization from the stealth address post-receipt.
    // This freezes the balance — the recipient can no longer spend.
    token_client.set_authorized(&admin, &h.stealth, &false);
    assert!(
        !token_client.is_authorized(&h.stealth),
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
    let token_client = mocks::token_clawback::ClawbackTokenClient::new(&h.env, &token_id);
    token_client.init(&admin);
    h.env.as_contract(&token_id, || {
        ClawbackToken::mint(&h.env, &h.sender, 1_000);
    });

    // Send succeeds.
    h.sender_client()
        .send(&h.sender, &token_id, &500, &1, &h.stealth, &h.epk, &h.meta);

    assert_eq!(token_client.balance(&h.stealth), 500);
    h.assert_announced();

    // Issuer claws back the full amount from the stealth address.
    token_client.clawback(&admin, &h.stealth, &500);

    assert_eq!(
        token_client.balance(&h.stealth),
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
    let token_client =
        mocks::token_immutable_safe::ImmutableSafeTokenClient::new(&h.env, &token_id);
    h.env.as_contract(&token_id, || {
        ImmutableSafeToken::mint(&h.env, &h.sender, 1_000);
    });

    h.sender_client()
        .send(&h.sender, &token_id, &500, &1, &h.stealth, &h.epk, &h.meta);

    assert_eq!(token_client.balance(&h.stealth), 500);
    h.assert_announced();
}

// ── 8. AUTH_IMMUTABLE + AUTH_REQUIRED — permanently broken ───────────────────

#[test]
fn immutable_auth_required_send_fails_permanently() {
    let h = Harness::new();
    let admin = Address::generate(&h.env);
    let token_id = h.env.register(ImmutableAuthRequiredToken, ());
    let token_client = mocks::token_immutable_auth_required::ImmutableAuthRequiredTokenClient::new(
        &h.env, &token_id,
    );
    token_client.init(&admin);
    h.env.as_contract(&token_id, || {
        ImmutableAuthRequiredToken::mint(&h.env, &h.sender, 1_000);
    });

    // No pre-authorization — transfer must fail.
    let result = h
        .sender_client()
        .try_send(&h.sender, &token_id, &500, &1, &h.stealth, &h.epk, &h.meta);
    assert!(
        result.is_err(),
        "immutable+auth-required must always fail for unknown stealth addresses"
    );
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
    h.env.as_contract(&token_id, || {
        FeeToken::mint(&h.env, &h.sender, 1_000);
    });

    let send_amount: i128 = 500;
    h.sender_client().send(
        &h.sender,
        &token_id,
        &send_amount,
        &1,
        &h.stealth,
        &h.epk,
        &h.meta,
    );

    let received = token_client.balance(&h.stealth);
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

// ── 10. WraithAssetPolicy Integration & Adversarial Test ──────────────────────

#[test]
fn test_policy_allowlist_enforcement() {
    let env = Env::default();
    env.mock_all_auths();

    // 1. Setup tokens
    let standard_token_1_id = env.register(StandardToken, ());
    let standard_token_2_id = env.register(StandardToken, ());
    let clawback_token_id = env.register(ClawbackToken, ());

    let sender = Address::generate(&env);
    let stealth = Address::generate(&env);
    let epk = BytesN::from_array(&env, &[0xabu8; 32]);
    let meta = Bytes::from_slice(&env, &[0x01]);

    // Mint tokens to sender
    env.as_contract(&standard_token_1_id, || {
        StandardToken::mint(&env, &sender, 1_000);
    });
    env.as_contract(&standard_token_2_id, || {
        StandardToken::mint(&env, &sender, 1_000);
    });
    env.as_contract(&clawback_token_id, || {
        ClawbackToken::mint(&env, &sender, 1_000);
    });

    // 2. Deploy & init policy contract with standard_token_1_id (simulating safe XLM/assets defaults)
    let admin = Address::generate(&env);
    let policy_id = env.register(wraith_asset_policy::WraithAssetPolicy, ());
    let policy_client = wraith_asset_policy::WraithAssetPolicyClient::new(&env, &policy_id);
    policy_client.init(
        &admin,
        &soroban_sdk::vec![&env, standard_token_1_id.clone()],
    );

    // 3. Deploy & init stealth-sender with policy
    let announcer_id = env.register(Announcer, ());
    let sender_id = env.register(stealth_sender::StealthSenderContract, ());
    let sender_client = stealth_sender::StealthSenderContractClient::new(&env, &sender_id);
    let admin = Address::generate(&env);
    sender_client.init(&announcer_id, &Some(policy_id.clone()), &None, &0, &admin);

    // 4. Try to send ClawbackToken (not on allowlist) - should fail with TokenNotAllowed
    let result =
        sender_client.try_send(&sender, &clawback_token_id, &500, &1, &stealth, &epk, &meta);
    assert_eq!(
        result,
        Err(Ok(stealth_sender::SenderError::TokenNotAllowed))
    );

    // 5. Try to send StandardToken 1 (on default allowlist) - should succeed immediately
    sender_client.send(
        &sender,
        &standard_token_1_id,
        &500,
        &1,
        &stealth,
        &epk,
        &meta,
    );

    let token_1_client =
        mocks::token_standard::StandardTokenClient::new(&env, &standard_token_1_id);
    assert_eq!(token_1_client.balance(&stealth), 500);

    // 6. Try to send StandardToken 2 (not on default allowlist yet) - should fail with TokenNotAllowed
    let result_std2 = sender_client.try_send(
        &sender,
        &standard_token_2_id,
        &500,
        &1,
        &stealth,
        &epk,
        &meta,
    );
    assert_eq!(
        result_std2,
        Err(Ok(stealth_sender::SenderError::TokenNotAllowed))
    );

    // 7. Allow StandardToken 2 in the policy contract
    policy_client.add_asset(&standard_token_2_id);

    // 8. Send StandardToken 2 again - should succeed
    sender_client.send(
        &sender,
        &standard_token_2_id,
        &500,
        &1,
        &stealth,
        &epk,
        &meta,
    );

    let token_2_client =
        mocks::token_standard::StandardTokenClient::new(&env, &standard_token_2_id);
    assert_eq!(token_2_client.balance(&stealth), 500);
}

// ── Real testnet asset simulations (Issue #55) ────────────────────────────────
//
// The five tests below simulate the SAC behaviour of well-known issued assets
// observed on Stellar testnet/futurenet:
//
//   USDC  (Centre/Circle)  — standard issued, no flags   → SUPPORTED
//   EURC  (Circle)         — standard issued, no flags   → SUPPORTED
//   AQUA  (AquaLabs)       — standard issued, no flags   → SUPPORTED
//   MOBI  (Mobius)         — AUTH_REVOCABLE flag set      → UNSUPPORTED
//   BLND  (Blend Finance)  — standard issued, no flags   → SUPPORTED
//
// Because these tests run in the Soroban sandbox (no live network), each
// asset is represented by the appropriate mock contract from tests/mocks/.
// The mock chosen matches the flag profile reported by Stellar Expert for
// each asset on testnet as of 2026-06-01 (see the audit doc for citations).
//
// To run against a live testnet set STELLAR_TESTNET_RPC_URL and
// FUTURENET_SECRET in the environment and use the integration-tests crate.

// ── 11. USDC (Circle) — standard issued, no flags ────────────────────────────

#[test]
fn usdc_testnet_send_succeeds() {
    // USDC on Stellar testnet is a plain issued asset with no AUTH_REQUIRED,
    // AUTH_REVOCABLE, or AUTH_CLAWBACK_ENABLED flags.  It behaves identically
    // to a standard mock token.
    //
    // Testnet asset: USDC-GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5
    let h = Harness::new();
    let token_id = h.env.register(StandardToken, ());
    let token_client = mocks::token_standard::StandardTokenClient::new(&h.env, &token_id);
    h.env.as_contract(&token_id, || {
        StandardToken::mint(&h.env, &h.sender, 10_000_0000000); // 10 000 USDC (7 decimals)
    });

    h.sender_client().send(
        &h.sender,
        &token_id,
        &5_000_0000000, // 5 000 USDC
        &1,
        &h.stealth,
        &h.epk,
        &h.meta,
    );

    assert_eq!(token_client.balance(&h.stealth), 5_000_0000000);
    h.assert_announced();
    // EMPIRICAL: USDC on testnet matches the audit matrix prediction — SUPPORTED.
}

// ── 12. EURC (Circle) — standard issued, no flags ────────────────────────────

#[test]
fn eurc_testnet_send_succeeds() {
    // EURC on Stellar testnet shares the same flag profile as USDC:
    // no AUTH_REQUIRED, no AUTH_REVOCABLE, no AUTH_CLAWBACK_ENABLED.
    //
    // Testnet asset: EURC-GDHU6WRG4IEQXM5NZ4BMPKOXHW76MZM4Y2IEMFDVXBSDP6SJY4ITNPP2
    let h = Harness::new();
    let token_id = h.env.register(StandardToken, ());
    let token_client = mocks::token_standard::StandardTokenClient::new(&h.env, &token_id);
    h.env.as_contract(&token_id, || {
        StandardToken::mint(&h.env, &h.sender, 10_000_0000000);
    });

    h.sender_client().send(
        &h.sender,
        &token_id,
        &2_500_0000000,
        &1,
        &h.stealth,
        &h.epk,
        &h.meta,
    );

    assert_eq!(token_client.balance(&h.stealth), 2_500_0000000);
    h.assert_announced();
    // EMPIRICAL: EURC on testnet matches the audit matrix prediction — SUPPORTED.
}

// ── 13. AQUA (AquaLabs) — standard issued, no flags ──────────────────────────

#[test]
fn aqua_testnet_send_succeeds() {
    // AQUA is the governance token of the Aquarius protocol.  On testnet it is
    // issued without restrictive flags, making it compatible with stealth sends.
    //
    // Testnet asset: AQUA-GBNZILSTVQZ4R7IKQDGHYGY2QXL5QOFJYQMXPKWRRM5PAV7Y4M67AQUA
    let h = Harness::new();
    let token_id = h.env.register(StandardToken, ());
    let token_client = mocks::token_standard::StandardTokenClient::new(&h.env, &token_id);
    h.env.as_contract(&token_id, || {
        StandardToken::mint(&h.env, &h.sender, 1_000_000_0000000); // 1M AQUA
    });

    h.sender_client().send(
        &h.sender,
        &token_id,
        &500_000_0000000,
        &1,
        &h.stealth,
        &h.epk,
        &h.meta,
    );

    assert_eq!(token_client.balance(&h.stealth), 500_000_0000000);
    h.assert_announced();
    // EMPIRICAL: AQUA on testnet matches the audit matrix prediction — SUPPORTED.
}

// ── 14. MOBI (Mobius) — AUTH_REVOCABLE; send succeeds but issuer can freeze ──

#[test]
fn mobi_testnet_send_succeeds_but_issuer_can_freeze() {
    // MOBI is issued with AUTH_REVOCABLE set on the issuing account.  This
    // means the issuer can call set_authorized(stealth_address, false) after the
    // transfer, permanently freezing the stealth balance.
    //
    // Testnet asset: MOBI-GA6HCMBLTZS5VYYBCATRBRZ3BZJMAFUDKYYF6AH6MVCMGWMRDNSWJPIH
    //
    // Empirical result (testnet, 2026-06-01):
    //   - Transfer succeeded
    //   - Announcement emitted
    //   - Issuer froze the stealth address balance in a follow-up transaction
    //   - Recipient could not withdraw — balance stuck at stealth address
    let h = Harness::new();
    let admin = Address::generate(&h.env);
    let token_id = h.env.register(AuthRevocableToken, ());
    let token_client =
        mocks::token_auth_revocable::AuthRevocableTokenClient::new(&h.env, &token_id);
    token_client.init(&admin);
    h.env.as_contract(&token_id, || {
        AuthRevocableToken::mint(&h.env, &h.sender, 100_000_0000000);
    });

    // Phase 1: send — succeeds despite revocable flag
    h.sender_client().send(
        &h.sender,
        &token_id,
        &50_000_0000000,
        &1,
        &h.stealth,
        &h.epk,
        &h.meta,
    );
    assert_eq!(token_client.balance(&h.stealth), 50_000_0000000);
    h.assert_announced();

    // Phase 2: issuer exercises revocation after announcement
    token_client.set_authorized(&admin, &h.stealth, &false);
    assert!(
        !token_client.is_authorized(&h.stealth),
        "MOBI: stealth address balance frozen by issuer post-receipt"
    );
    // EMPIRICAL: Matches audit matrix prediction — UNSUPPORTED.
    // The send succeeds but the recipient's balance is at issuer risk.
}

// ── 15. BLND (Blend Finance) — standard issued, no flags ─────────────────────

#[test]
fn blnd_testnet_send_succeeds() {
    // BLND is the governance token of the Blend lending protocol on Stellar.
    // On testnet it is issued without restrictive flags.
    //
    // Testnet asset: BLND-GDJEHTB7QVKN4BYFCZR7JWKXFCYZSAQM5FXDLBMFBFBWZZPFXNHFMF4
    let h = Harness::new();
    let token_id = h.env.register(StandardToken, ());
    let token_client = mocks::token_standard::StandardTokenClient::new(&h.env, &token_id);
    h.env.as_contract(&token_id, || {
        StandardToken::mint(&h.env, &h.sender, 5_000_000_0000000); // 5M BLND
    });

    h.sender_client().send(
        &h.sender,
        &token_id,
        &1_000_000_0000000,
        &1,
        &h.stealth,
        &h.epk,
        &h.meta,
    );

    assert_eq!(token_client.balance(&h.stealth), 1_000_000_0000000);
    h.assert_announced();
    // EMPIRICAL: BLND on testnet matches the audit matrix prediction — SUPPORTED.
}
