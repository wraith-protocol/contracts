//! Dedicated integration tests for the stealth-sender protocol fee mechanism.
//!
//! Enforces:
//! 1. Fee cap at 50 basis points (0.5%)
//! 2. Validation that fee recipient is present if basis points > 0
//! 3. Default/off behavior when fee is 0
//! 4. Atomic fee diversion for send and batch_send
//! 5. Adversarial tests (fee > cap, fee_recipient not authorized for token)

mod mocks;

use mocks::{
    token_auth_required::AuthRequiredToken,
    token_standard::StandardToken,
};
use soroban_sdk::{
    testutils::Address as _,
    testutils::Events as _,
    token::TokenInterface as _,
    Address, Bytes, BytesN, Env,
};

// Minimal announcer implementation for test events
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
            env.storage().instance().set(&symbol_short!("called"), &true);
            env.events().publish(
                (symbol_short!("announce"), stealth_address),
                (),
            );
        }

        pub fn is_called(env: Env) -> bool {
            env.storage().instance().get(&symbol_short!("called")).unwrap_or(false)
        }
    }
}

struct Harness {
    env: Env,
    sender_id: Address,
    announcer_id: Address,
    sender: Address,
    stealth: Address,
    fee_recipient: Address,
    epk: BytesN<32>,
    meta: Bytes,
}

impl Harness {
    fn new(fee_recipient: Option<Address>, fee_basis_points: u32) -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let announcer_id = env.register(announcer::Announcer, ());
        let sender_id = env.register(stealth_sender::StealthSenderContract, ());
        
        let sender_client = stealth_sender::StealthSenderContractClient::new(&env, &sender_id);
        sender_client.init(&announcer_id, &None, &fee_recipient, &fee_basis_points);

        let sender = Address::generate(&env);
        let stealth = Address::generate(&env);
        let fee_recipient_addr = fee_recipient.unwrap_or_else(|| Address::generate(&env));
        let epk = BytesN::from_array(&env, &[0xabu8; 32]);
        let meta = Bytes::from_slice(&env, &[0x01]);

        Harness {
            env,
            sender_id,
            announcer_id,
            sender,
            stealth,
            fee_recipient: fee_recipient_addr,
            epk,
            meta,
        }
    }

    fn sender_client(&self) -> stealth_sender::StealthSenderContractClient<'_> {
        stealth_sender::StealthSenderContractClient::new(&self.env, &self.sender_id)
    }

    fn assert_announced(&self) {
        let client = announcer::AnnouncerClient::new(&self.env, &self.announcer_id);
        assert!(client.is_called(), "expected announcement, none found");
    }

    fn assert_not_announced(&self) {
        let client = announcer::AnnouncerClient::new(&self.env, &self.announcer_id);
        assert!(!client.is_called(), "unexpected announcement found");
    }
}

// ── 1. Adversarial/Validation Tests ──────────────────────────────────────────

#[test]
#[should_panic(expected = "HostError: Error(Contract, #5)")]
fn test_fee_exceeds_cap_fails_init() {
    let env = Env::default();
    env.mock_all_auths();

    let announcer_id = env.register(announcer::Announcer, ());
    let sender_id = env.register(stealth_sender::StealthSenderContract, ());
    let sender_client = stealth_sender::StealthSenderContractClient::new(&env, &sender_id);

    let fee_recipient = Address::generate(&env);
    // 51 bps exceeds 50 bps cap, must fail
    sender_client.init(&announcer_id, &None, &Some(fee_recipient), &51);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #5)")]
fn test_fee_with_none_recipient_fails_init() {
    let env = Env::default();
    env.mock_all_auths();

    let announcer_id = env.register(announcer::Announcer, ());
    let sender_id = env.register(stealth_sender::StealthSenderContract, ());
    let sender_client = stealth_sender::StealthSenderContractClient::new(&env, &sender_id);

    // fee basis points > 0 with None recipient must fail
    sender_client.init(&announcer_id, &None, &None, &10);
}

// ── 2. Correctness/Calculation Tests ─────────────────────────────────────────

#[test]
fn test_fee_disabled_by_default() {
    // 0 bps, None recipient -> behaves as standard sender
    let h = Harness::new(None, 0);

    let token_id = h.env.register(StandardToken, ());
    let token_client = mocks::token_standard::StandardTokenClient::new(&h.env, &token_id);
    h.env.as_contract(&token_id, || {
        StandardToken::mint(&h.env, &h.sender, 1_000);
    });

    h.sender_client()
        .send(&h.sender, &token_id, &500, &1, &h.stealth, &h.epk, &h.meta);

    assert_eq!(token_client.balance(&h.stealth), 500);
    assert_eq!(token_client.balance(&h.sender), 500);
    h.assert_announced();
}

#[test]
fn test_fee_diverted_successfully() {
    let fee_recipient = Address::generate(&Env::default());
    // 50 bps = 0.5%
    let h = Harness::new(Some(fee_recipient.clone()), 50);

    let token_id = h.env.register(StandardToken, ());
    let token_client = mocks::token_standard::StandardTokenClient::new(&h.env, &token_id);
    h.env.as_contract(&token_id, || {
        StandardToken::mint(&h.env, &h.sender, 10_000);
    });

    h.sender_client()
        .send(&h.sender, &token_id, &10_000, &1, &h.stealth, &h.epk, &h.meta);

    // 10000 * 50 / 10000 = 50 fee
    // Stealth address receives 9950
    assert_eq!(token_client.balance(&h.stealth), 9950);
    assert_eq!(token_client.balance(&fee_recipient), 50);
    assert_eq!(token_client.balance(&h.sender), 0);
    h.assert_announced();
}

#[test]
fn test_batch_fee_diverted_successfully() {
    let fee_recipient = Address::generate(&Env::default());
    // 10 bps = 0.1%
    let h = Harness::new(Some(fee_recipient.clone()), 10);

    let token_id = h.env.register(StandardToken, ());
    let token_client = mocks::token_standard::StandardTokenClient::new(&h.env, &token_id);
    h.env.as_contract(&token_id, || {
        StandardToken::mint(&h.env, &h.sender, 20_000);
    });

    let stealth_1 = Address::generate(&h.env);
    let stealth_2 = Address::generate(&h.env);

    let addresses = soroban_sdk::vec![&h.env, stealth_1.clone(), stealth_2.clone()];
    let epks = soroban_sdk::vec![&h.env, h.epk.clone(), h.epk.clone()];
    let metadatas = soroban_sdk::vec![&h.env, h.meta.clone(), h.meta.clone()];
    let amounts = soroban_sdk::vec![&h.env, 10_000, 10_000];

    h.sender_client()
        .batch_send(&h.sender, &token_id, &1, &addresses, &epks, &metadatas, &amounts);

    // Each transfer gets amount - (amount * 10 / 10000) = 10000 - 10 = 9990
    // Total fee recipient gets 10 + 10 = 20
    assert_eq!(token_client.balance(&stealth_1), 9990);
    assert_eq!(token_client.balance(&stealth_2), 9990);
    assert_eq!(token_client.balance(&fee_recipient), 20);
    assert_eq!(token_client.balance(&h.sender), 0);
}

// ── 3. Adversarial compatibility tests ──────────────────────────────────────

#[test]
fn test_fee_recipient_unauthorized_token_fails() {
    let fee_recipient = Address::generate(&Env::default());
    // 50 bps fee
    let h = Harness::new(Some(fee_recipient.clone()), 50);

    let admin = Address::generate(&h.env);
    let token_id = h.env.register(AuthRequiredToken, ());
    let token_client = mocks::token_auth_required::AuthRequiredTokenClient::new(&h.env, &token_id);
    
    token_client.init(&admin);
    h.env.as_contract(&token_id, || {
        AuthRequiredToken::mint(&h.env, &h.sender, 10_000);
    });

    // Authorize sender and stealth address, but NOT the fee_recipient
    token_client.set_authorized(&admin, &h.sender, &true);
    token_client.set_authorized(&admin, &h.stealth, &true);

    // Try to send — transfer to unauthorized fee_recipient must fail and roll back the whole tx
    let result = h.sender_client().try_send(&h.sender, &token_id, &10_000, &1, &h.stealth, &h.epk, &h.meta);
    assert!(result.is_err(), "expected tx to revert since fee_recipient is unauthorized");
    
    // Assert no tokens were moved
    assert_eq!(token_client.balance(&h.sender), 10_000);
    assert_eq!(token_client.balance(&h.stealth), 0);
    assert_eq!(token_client.balance(&fee_recipient), 0);
    h.assert_not_announced();
}
