#![cfg(test)]

use governance::{
    GovernanceContract, GovernanceContractClient, GovernanceError,
};
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Ledger},
    Address, Bytes, Env, String,
};

#[contract]
pub struct MockTarget;

#[contractimpl]
impl MockTarget {
    pub fn set_value(_env: Env, value: Bytes) -> Bytes {
        value
    }
}

fn setup_env() -> (Env, GovernanceContractClient<'static>, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin)
        .address();
    let gov_id = env.register(GovernanceContract, ());
    let gov_client = GovernanceContractClient::new(&env, &gov_id);
    let target_id = env.register(MockTarget, ());
    gov_client.init(&admin, &token_id, &100i128, &50u32, &10u32);

    (env, gov_client, token_id, target_id, admin)
}

#[test]
fn test_quorum_not_met_blocks_execution() {
    let (env, gov, token_id, _target_id, _admin) = setup_env();
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    let proposer = Address::generate(&env);
    let voter = Address::generate(&env);
    token_client.mint(&voter, &75);

    let pid = gov.propose(
        &proposer,
        &Address::generate(&env),
        &soroban_sdk::symbol_short!("set_value"),
        &Bytes::from_slice(&env, b"too-low"),
        &String::from_str(&env, "quorum test"),
    );

    gov.vote(&voter, &pid, &true);
    let proposal = gov.get_proposal(&pid);
    env.ledger().with_mut(|li| {
        li.sequence_number = proposal.end_ledger + 20;
    });

    let result = gov.try_execute(&pid);
    assert_eq!(result, Err(Ok(GovernanceError::QuorumNotMet)));
}

#[test]
fn test_cancelled_proposal_cannot_execute() {
    let (env, gov, token_id, _target_id, admin) = setup_env();
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    let voter = Address::generate(&env);
    token_client.mint(&voter, &200);

    let proposer = Address::generate(&env);
    let pid = gov.propose(
        &proposer,
        &Address::generate(&env),
        &soroban_sdk::symbol_short!("set_value"),
        &Bytes::from_slice(&env, b"cancelled"),
        &String::from_str(&env, "cancel test"),
    );

    gov.vote(&voter, &pid, &true);
    gov.cancel(&pid);

    let proposal = gov.get_proposal(&pid);
    assert!(proposal.cancelled);

    let result = gov.try_execute(&pid);
    assert_eq!(result, Err(Ok(GovernanceError::AlreadyCancelled)));
    let _ = admin;
}

#[test]
fn test_proposal_cannot_execute_twice() {
    let (env, gov, token_id, target_id, _admin) = setup_env();
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);
    let proposer = Address::generate(&env);
    let voter = Address::generate(&env);
    token_client.mint(&voter, &200);

    let pid = gov.propose(
        &proposer,
        &target_id,
        &soroban_sdk::symbol_short!("set_value"),
        &Bytes::from_slice(&env, b"done"),
        &String::from_str(&env, "execute once"),
    );

    gov.vote(&voter, &pid, &true);
    let proposal = gov.get_proposal(&pid);
    env.ledger().with_mut(|li| {
        li.sequence_number = proposal.end_ledger + 20;
    });

    gov.execute(&pid);
    let second = gov.try_execute(&pid);
    assert_eq!(second, Err(Ok(GovernanceError::AlreadyExecuted)));
}
