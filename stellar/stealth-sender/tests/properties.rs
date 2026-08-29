use proptest::prelude::*;
use soroban_sdk::testutils::{Address as _, EnvTestConfig};
use soroban_sdk::{contract, contractimpl, token, Address, Bytes, BytesN, Env, Vec};
use stealth_sender::{SenderError, StealthSenderContract, StealthSenderContractClient};

#[contract]
pub struct AnnouncerMock;

#[contractimpl]
impl AnnouncerMock {
    pub fn announce(
        env: Env,
        scheme_id: u32,
        stealth_address: Address,
        ephemeral_pub_key: BytesN<32>,
        metadata: Bytes,
    ) {
        let key = soroban_sdk::symbol_short!("count");
        let count: u32 = env.storage().instance().get(&key).unwrap_or(0);
        env.storage().instance().set(&key, &(count + 1));
        env.events().publish(
            (
                soroban_sdk::symbol_short!("announce"),
                scheme_id,
                stealth_address,
            ),
            (env.current_contract_address(), ephemeral_pub_key, metadata),
        );
    }

    pub fn count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&soroban_sdk::symbol_short!("count"))
            .unwrap_or(0)
    }
}

fn cases() -> u32 {
    std::env::var("WRAITH_PROPTEST_CASES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1024)
}

fn bytes(env: &Env, data: &[u8]) -> Bytes {
    Bytes::from_slice(env, data)
}

fn env() -> Env {
    Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    })
}

fn bytes32(env: &Env, data: &[u8; 32]) -> BytesN<32> {
    BytesN::from_array(env, data)
}

struct Fixture {
    env: Env,
    contract_id: Address,
    announcer: Address,
    sender: Address,
    token: Address,
    admin: Address,
}

fn fixture(initial_balance: i128) -> Fixture {
    let env = env();
    env.mock_all_auths();

    let announcer = env.register(AnnouncerMock, ());
    let contract_id = env.register(StealthSenderContract, ());

    let admin = Address::generate(&env);
    let sender = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let token = sac.address();
    let asset_client = token::StellarAssetClient::new(&env, &token);
    asset_client.mint(&sender, &initial_balance);

    Fixture {
        env,
        contract_id,
        announcer,
        sender,
        token,
        admin,
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: cases(), .. ProptestConfig::default() })]
    #[test]
    fn init_can_only_run_once(_seed in any::<[u8; 32]>()) {
        let f = fixture(1_000_000);
        let client = StealthSenderContractClient::new(&f.env, &f.contract_id);
        client.init(&f.announcer, &None, &None, &0, &f.admin);

        let result = client.try_init(&f.announcer, &None, &None, &0, &f.admin);

        prop_assert_eq!(result, Err(Ok(SenderError::AlreadyInitialized)));
    }

    #[test]
    fn send_requires_initialization(amount in 1i128..1_000_000, epk in any::<[u8; 32]>(), metadata in prop::collection::vec(any::<u8>(), 0..64)) {
        let f = fixture(1_000_000);
        let client = StealthSenderContractClient::new(&f.env, &f.contract_id);
        let stealth_address = Address::generate(&f.env);

        let result = client.try_send(
            &f.sender,
            &f.token,
            &amount,
            &1u32,
            &stealth_address,
            &bytes32(&f.env, &epk),
            &bytes(&f.env, &metadata),
        );

        prop_assert_eq!(result, Err(Ok(SenderError::NotInitialized)));
    }

    #[test]
    fn send_transfers_exact_amount_and_announces(amount in 1i128..1_000_000, scheme_id in any::<u32>(), epk in any::<[u8; 32]>(), metadata in prop::collection::vec(any::<u8>(), 0..64)) {
        let f = fixture(1_000_000);
        let client = StealthSenderContractClient::new(&f.env, &f.contract_id);
        let token_client = token::TokenClient::new(&f.env, &f.token);
        let announcer_client = AnnouncerMockClient::new(&f.env, &f.announcer);
        let stealth_address = Address::generate(&f.env);
        client.init(&f.announcer, &None, &None, &0, &f.admin);

        client.send(
            &f.sender,
            &f.token,
            &amount,
            &scheme_id,
            &stealth_address,
            &bytes32(&f.env, &epk),
            &bytes(&f.env, &metadata),
        );

        prop_assert_eq!(token_client.balance(&stealth_address), amount);
        prop_assert_eq!(token_client.balance(&f.sender), 1_000_000 - amount);
        prop_assert_eq!(announcer_client.count(), 1);
    }

    #[test]
    fn batch_send_rejects_mismatched_lengths(amount in 1i128..1_000_000, epk in any::<[u8; 32]>()) {
        let f = fixture(1_000_000);
        let client = StealthSenderContractClient::new(&f.env, &f.contract_id);
        let announcer_client = AnnouncerMockClient::new(&f.env, &f.announcer);
        let stealth_address = Address::generate(&f.env);
        client.init(&f.announcer, &None, &None, &0, &f.admin);

        let mut addresses = Vec::new(&f.env);
        addresses.push_back(stealth_address);
        let keys = Vec::new(&f.env);
        let mut metadatas = Vec::new(&f.env);
        metadatas.push_back(bytes(&f.env, &[1]));
        let mut amounts = Vec::new(&f.env);
        amounts.push_back(amount);

        let result = client.try_batch_send(
            &f.sender,
            &f.token,
            &1u32,
            &addresses,
            &keys,
            &metadatas,
            &amounts,
        );

        prop_assert_eq!(result, Err(Ok(SenderError::LengthMismatch)));
        prop_assert_eq!(announcer_client.count(), 0);
        let _ = epk;
    }

    #[test]
    fn batch_send_transfers_every_item_and_announces(count in 1u32..8, amount in 1i128..100_000, scheme_id in any::<u32>(), epk in any::<[u8; 32]>()) {
        let total = amount * i128::from(count);
        let f = fixture(total + 1_000);
        let client = StealthSenderContractClient::new(&f.env, &f.contract_id);
        let token_client = token::TokenClient::new(&f.env, &f.token);
        let announcer_client = AnnouncerMockClient::new(&f.env, &f.announcer);
        client.init(&f.announcer, &None, &None, &0, &f.admin);

        let mut addresses = Vec::new(&f.env);
        let mut keys = Vec::new(&f.env);
        let mut metadatas = Vec::new(&f.env);
        let mut amounts = Vec::new(&f.env);

        for i in 0..count {
            addresses.push_back(Address::generate(&f.env));
            keys.push_back(bytes32(&f.env, &epk));
            metadatas.push_back(bytes(&f.env, &[i as u8]));
            amounts.push_back(amount);
        }

        client.batch_send(&f.sender, &f.token, &scheme_id, &addresses, &keys, &metadatas, &amounts);

        for i in 0..count {
            prop_assert_eq!(token_client.balance(&addresses.get(i).unwrap()), amount);
        }
        prop_assert_eq!(token_client.balance(&f.sender), 1_000);
        prop_assert_eq!(announcer_client.count(), count);
    }
}

#[test]
fn default_property_case_count_is_at_least_1024() {
    assert!(cases() >= 1024);
}
