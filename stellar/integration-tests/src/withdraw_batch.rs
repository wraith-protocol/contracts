use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{Address as _, Events as _},
    Address, Env, Vec,
};
use stealth_sender::{StealthSenderContract, StealthSenderContractClient, WithdrawalEntry};

#[contract]
pub struct FailOnNthToken;

#[contracttype]
#[derive(Clone)]
pub enum MockTokenDataKey {
    Balance(Address),
    TransferCount,
    FailOnTransferNumber,
}

#[contractimpl]
impl FailOnNthToken {
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();

        let count: u32 = env
            .storage()
            .instance()
            .get(&MockTokenDataKey::TransferCount)
            .unwrap_or(0);

        let fail_on: u32 = env
            .storage()
            .instance()
            .get(&MockTokenDataKey::FailOnTransferNumber)
            .unwrap_or(u32::MAX);

        if count == fail_on {
            panic!("transfer failed on nth attempt");
        }

        let from_balance: i128 = env
            .storage()
            .instance()
            .get(&MockTokenDataKey::Balance(from.clone()))
            .unwrap_or(0);

        if from_balance < amount {
            panic!("insufficient balance");
        }

        let to_balance: i128 = env
            .storage()
            .instance()
            .get(&MockTokenDataKey::Balance(to.clone()))
            .unwrap_or(0);

        env.storage().instance().set(
            &MockTokenDataKey::Balance(from.clone()),
            &(from_balance - amount),
        );
        env.storage().instance().set(
            &MockTokenDataKey::Balance(to.clone()),
            &(to_balance + amount),
        );

        env.storage()
            .instance()
            .set(&MockTokenDataKey::TransferCount, &(count + 1));
    }

    pub fn mint(env: Env, to: Address, amount: i128) {
        let balance: i128 = env
            .storage()
            .instance()
            .get(&MockTokenDataKey::Balance(to.clone()))
            .unwrap_or(0);

        env.storage()
            .instance()
            .set(&MockTokenDataKey::Balance(to), &(balance + amount));
    }

    pub fn set_fail_on_transfer_number(env: Env, n: u32) {
        env.storage()
            .instance()
            .set(&MockTokenDataKey::FailOnTransferNumber, &n);
    }

    pub fn balance_of(env: Env, account: Address) -> i128 {
        env.storage()
            .instance()
            .get(&MockTokenDataKey::Balance(account))
            .unwrap_or(0)
    }
}

fn fixture() -> (Env, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(StealthSenderContract, ());
    let token_id = env.register(FailOnNthToken, ());

    (env, contract_id, token_id)
}

#[test]
fn withdraw_many_transfers_each_entry_and_emits_events() {
    let (env, contract_id, token_id) = fixture();
    let client = StealthSenderContractClient::new(&env, &contract_id);
    let token_client = FailOnNthTokenClient::new(&env, &token_id);

    let withdrawer = Address::generate(&env);
    let recipient_a = Address::generate(&env);
    let recipient_b = Address::generate(&env);

    token_client.mint(&withdrawer, &1_000);

    let mut entries = Vec::new(&env);
    entries.push_back(WithdrawalEntry {
        token: token_id.clone(),
        to: recipient_a.clone(),
        amount: 100,
    });
    entries.push_back(WithdrawalEntry {
        token: token_id.clone(),
        to: recipient_b.clone(),
        amount: 200,
    });

    client.withdraw_many(&withdrawer, &entries);

    let events = env.events().all();

    assert_eq!(token_client.balance_of(&withdrawer), 700);
    assert_eq!(token_client.balance_of(&recipient_a), 100);
    assert_eq!(token_client.balance_of(&recipient_b), 200);
    assert!(events.len() >= 3);
}

#[test]
fn withdraw_many_reverts_atomically_when_one_entry_fails() {
    let (env, contract_id, token_id) = fixture();
    let client = StealthSenderContractClient::new(&env, &contract_id);
    let token_client = FailOnNthTokenClient::new(&env, &token_id);

    let withdrawer = Address::generate(&env);
    let recipient_a = Address::generate(&env);
    let recipient_b = Address::generate(&env);
    let recipient_c = Address::generate(&env);

    token_client.mint(&withdrawer, &1_000);
    token_client.set_fail_on_transfer_number(&2);

    let mut entries = Vec::new(&env);
    entries.push_back(WithdrawalEntry {
        token: token_id.clone(),
        to: recipient_a.clone(),
        amount: 100,
    });
    entries.push_back(WithdrawalEntry {
        token: token_id.clone(),
        to: recipient_b.clone(),
        amount: 200,
    });
    entries.push_back(WithdrawalEntry {
        token: token_id.clone(),
        to: recipient_c.clone(),
        amount: 300,
    });

    let result = client.try_withdraw_many(&withdrawer, &entries);

    assert!(result.is_err(), "expected batch withdrawal to fail");
    assert_eq!(token_client.balance_of(&withdrawer), 1_000);
    assert_eq!(token_client.balance_of(&recipient_a), 0);
    assert_eq!(token_client.balance_of(&recipient_b), 0);
    assert_eq!(token_client.balance_of(&recipient_c), 0);
}
