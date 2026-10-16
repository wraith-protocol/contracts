//! Received-amount check in `batch_send` (issue #227).
//!
//! Each transfer must raise its stealth address's balance by exactly
//! `transfer.amount`; otherwise the batch fails with `ReceivedAmountMismatch`
//! and nothing is announced. A single send through this contract is a
//! one-transfer batch.

use soroban_sdk::{
    testutils::Address as _,
    token::{Client as TokenClient, StellarAssetClient},
    vec, Address, Bytes, Env,
};
use stealth_batch_sender::{
    BatchSenderError, StealthBatchSender, StealthBatchSenderClient, Transfer,
};

// Each mock lives in its own module: soroban-sdk's #[contractimpl] macro
// generates helper items named after the method, which collide across types
// that share a module scope.

/// Announcer stand-in that counts calls in instance storage. A rejected batch
/// rolls the count back with every other state change, so a count of zero
/// after a failed call means no announcement survived.
mod counting_announcer {
    use soroban_sdk::{contract, contractimpl, symbol_short, Address, Bytes, BytesN, Env};

    #[contract]
    pub struct CountingAnnouncer;

    #[contractimpl]
    impl CountingAnnouncer {
        pub fn announce(
            env: Env,
            _scheme_id: u32,
            _stealth_address: Address,
            _ephemeral_pub_key: BytesN<32>,
            _metadata: Bytes,
        ) {
            let count = Self::count(env.clone());
            env.storage()
                .instance()
                .set(&symbol_short!("count"), &(count + 1));
        }

        pub fn count(env: Env) -> u32 {
            env.storage()
                .instance()
                .get(&symbol_short!("count"))
                .unwrap_or(0)
        }
    }
}
use counting_announcer::{CountingAnnouncer, CountingAnnouncerClient};

/// Non-SAC token whose `transfer` debits `amount` but credits the recipient
/// `amount + amount * skew_bps / 10_000`. A negative skew models a
/// fee-on-transfer token (the difference is burned), a positive one a token
/// that credits more than it is asked to.
mod skewed_token {
    use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

    #[contracttype]
    #[derive(Clone)]
    enum DataKey {
        Balance(Address),
        SkewBps,
    }

    #[contract]
    pub struct SkewedToken;

    #[contractimpl]
    impl SkewedToken {
        pub fn init(env: Env, skew_bps: i128) {
            env.storage().instance().set(&DataKey::SkewBps, &skew_bps);
        }

        pub fn mint(env: Env, to: Address, amount: i128) {
            let balance = Self::balance(env.clone(), to.clone());
            env.storage()
                .persistent()
                .set(&DataKey::Balance(to), &(balance + amount));
        }

        pub fn balance(env: Env, id: Address) -> i128 {
            env.storage()
                .persistent()
                .get(&DataKey::Balance(id))
                .unwrap_or(0)
        }

        pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
            from.require_auth();
            let skew_bps: i128 = env.storage().instance().get(&DataKey::SkewBps).unwrap();
            let credited = amount + amount * skew_bps / 10_000;

            let from_balance = Self::balance(env.clone(), from.clone());
            env.storage()
                .persistent()
                .set(&DataKey::Balance(from), &(from_balance - amount));
            let to_balance = Self::balance(env.clone(), to.clone());
            env.storage()
                .persistent()
                .set(&DataKey::Balance(to), &(to_balance + credited));
        }
    }
}
use skewed_token::{SkewedToken, SkewedTokenClient};

fn setup(
    env: &Env,
) -> (
    StealthBatchSenderClient<'static>,
    CountingAnnouncerClient<'static>,
) {
    let contract_id = env.register(StealthBatchSender, ());
    let client = StealthBatchSenderClient::new(env, &contract_id);
    let announcer_id = env.register(CountingAnnouncer, ());
    client.init(&Address::generate(env), &announcer_id, &None);
    (client, CountingAnnouncerClient::new(env, &announcer_id))
}

fn skewed_token(env: &Env, skew_bps: i128, holder: &Address) -> SkewedTokenClient<'static> {
    let token = SkewedTokenClient::new(env, &env.register(SkewedToken, ()));
    token.init(&skew_bps);
    token.mint(holder, &1_000);
    token
}

fn sac(env: &Env, holder: &Address) -> TokenClient<'static> {
    let token_id = env
        .register_stellar_asset_contract_v2(Address::generate(env))
        .address();
    StellarAssetClient::new(env, &token_id).mint(holder, &1_000);
    TokenClient::new(env, &token_id)
}

fn transfer(env: &Env, stealth_address: &Address, amount: i128) -> Transfer {
    Transfer {
        stealth_address: stealth_address.clone(),
        ephemeral_pub_key: Bytes::from_slice(env, &[0x02u8; 32]),
        amount,
        metadata: Bytes::from_slice(env, &[0x2Au8]),
    }
}

#[test]
fn fee_on_transfer_single_send_rejected_before_announcement() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, announcer) = setup(&env);
    let sender = Address::generate(&env);
    let token = skewed_token(&env, -100, &sender);
    let stealth = Address::generate(&env);

    // A 1% fee leaves 495 of 500 at the stealth address.
    let result = client.try_batch_send(
        &sender,
        &vec![&env, transfer(&env, &stealth, 500)],
        &token.address,
    );
    assert_eq!(result, Err(Ok(BatchSenderError::ReceivedAmountMismatch)));

    assert_eq!(token.balance(&sender), 1_000);
    assert_eq!(token.balance(&stealth), 0);
    assert_eq!(announcer.count(), 0);
}

#[test]
fn fee_on_transfer_batch_rejected_before_announcement() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, announcer) = setup(&env);
    let sender = Address::generate(&env);
    let token = skewed_token(&env, -100, &sender);
    let stealth_1 = Address::generate(&env);
    let stealth_2 = Address::generate(&env);

    // 1% of 99 rounds down to a zero fee, so the first transfer arrives in
    // full and is announced. The second loses 5 of 500, which must reject the
    // whole batch, including the first transfer and its announcement.
    let result = client.try_batch_send(
        &sender,
        &vec![
            &env,
            transfer(&env, &stealth_1, 99),
            transfer(&env, &stealth_2, 500),
        ],
        &token.address,
    );
    assert_eq!(result, Err(Ok(BatchSenderError::ReceivedAmountMismatch)));

    assert_eq!(token.balance(&sender), 1_000);
    assert_eq!(token.balance(&stealth_1), 0);
    assert_eq!(token.balance(&stealth_2), 0);
    assert_eq!(announcer.count(), 0);
}

#[test]
fn fee_on_transfer_to_funded_stealth_address_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, announcer) = setup(&env);
    let sender = Address::generate(&env);
    let token = skewed_token(&env, -100, &sender);
    let stealth = Address::generate(&env);

    // Both transfers go to one stealth address. 99 arrives in full (the 1% fee
    // rounds to 0), so the second transfer lands on an address that already
    // holds funds, and its 5-token shortfall must still reject the batch.
    let result = client.try_batch_send(
        &sender,
        &vec![
            &env,
            transfer(&env, &stealth, 99),
            transfer(&env, &stealth, 500),
        ],
        &token.address,
    );
    assert_eq!(result, Err(Ok(BatchSenderError::ReceivedAmountMismatch)));

    assert_eq!(token.balance(&sender), 1_000);
    assert_eq!(token.balance(&stealth), 0);
    assert_eq!(announcer.count(), 0);
}

#[test]
fn over_crediting_token_rejected_before_announcement() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, announcer) = setup(&env);
    let sender = Address::generate(&env);
    let token = skewed_token(&env, 100, &sender);
    let stealth = Address::generate(&env);

    // The token credits 505 for a 500 transfer. The check requires the exact
    // amount, so a token that does not move exactly what it is asked to is
    // rejected in either direction.
    let result = client.try_batch_send(
        &sender,
        &vec![&env, transfer(&env, &stealth, 500)],
        &token.address,
    );
    assert_eq!(result, Err(Ok(BatchSenderError::ReceivedAmountMismatch)));

    assert_eq!(token.balance(&sender), 1_000);
    assert_eq!(token.balance(&stealth), 0);
    assert_eq!(announcer.count(), 0);
}

#[test]
fn sac_batch_to_same_stealth_address_twice_succeeds() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, announcer) = setup(&env);
    let sender = Address::generate(&env);
    let token = sac(&env, &sender);
    let stealth = Address::generate(&env);

    // The second transfer reaches an address that already holds the first
    // payment. The balance is read around each transfer, so both pass.
    client.batch_send(
        &sender,
        &vec![
            &env,
            transfer(&env, &stealth, 300),
            transfer(&env, &stealth, 400),
        ],
        &token.address,
    );

    assert_eq!(token.balance(&sender), 300);
    assert_eq!(token.balance(&stealth), 700);
    assert_eq!(announcer.count(), 2);
}

#[test]
fn sac_batch_to_own_address_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, announcer) = setup(&env);
    let sender = Address::generate(&env);
    let token = sac(&env, &sender);

    // Sending to the caller's own address moves nothing, so the "stealth
    // address" does not gain the amount and the payment is not announced.
    let result = client.try_batch_send(
        &sender,
        &vec![&env, transfer(&env, &sender, 500)],
        &token.address,
    );
    assert_eq!(result, Err(Ok(BatchSenderError::ReceivedAmountMismatch)));

    assert_eq!(token.balance(&sender), 1_000);
    assert_eq!(announcer.count(), 0);
}
