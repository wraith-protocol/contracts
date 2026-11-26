//! Governance authorization tests for Wraith Names.
//!
//! The contract has no WASM-upgrade or renunciation entry point. Its
//! governance multisig controls signer-set rotation and auction-admin
//! rotation; these tests exercise the signer-set lifecycle that exists.

#![cfg(test)]

use soroban_sdk::testutils::{Address as _, Events, Ledger};
use soroban_sdk::{Address, Env, IntoVal, Symbol, Vec};
use wraith_names::{NamesError, WraithNamesContract, WraithNamesContractClient};

const ROTATION_TIMELOCK_SECS: u64 = 7 * 24 * 60 * 60;

struct Setup<'a> {
    env: Env,
    contract_id: Address,
    client: WraithNamesContractClient<'a>,
    signers: Vec<Address>,
}

fn setup() -> Setup<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);
    let signers = new_signers(&env);
    client.init_multisig(&signers, &3);

    Setup {
        env,
        contract_id,
        client,
        signers,
    }
}

fn new_signers(env: &Env) -> Vec<Address> {
    Vec::from_array(
        env,
        [
            Address::generate(env),
            Address::generate(env),
            Address::generate(env),
            Address::generate(env),
            Address::generate(env),
        ],
    )
}

fn advance_time(env: &Env, seconds: u64) {
    env.ledger().with_mut(|ledger| {
        ledger.timestamp += seconds;
    });
}

#[test]
fn rotation_requires_quorum_and_replaces_the_signer_set() {
    let setup = setup();
    let replacement = new_signers(&setup.env);
    let proposer = setup.signers.get(0).unwrap();

    setup
        .client
        .propose_rotate_signers(&proposer, &replacement, &2);
    let pending = setup.client.pending_rotation().unwrap();
    assert_eq!(pending.approvals.len(), 1);
    assert_eq!(pending.new_threshold, 2);

    advance_time(&setup.env, ROTATION_TIMELOCK_SECS);
    let result = setup.client.try_execute_rotate_signers(&proposer);
    assert_eq!(result, Err(Ok(NamesError::QuorumNotMet)));

    setup
        .client
        .approve_rotate_signers(&setup.signers.get(1).unwrap());
    setup
        .client
        .approve_rotate_signers(&setup.signers.get(2).unwrap());
    setup.client.execute_rotate_signers(&proposer);

    let events = setup.env.events().all();
    assert_eq!(events.len(), 1);
    let (contract, topics, data) = events.last().unwrap();
    assert_eq!(contract, setup.contract_id);
    assert_eq!(
        topics,
        (Symbol::new(&setup.env, "SignersRotated"),).into_val(&setup.env)
    );
    let (rotated_signers, old_threshold, new_threshold): (Vec<Address>, u32, u32) =
        data.into_val(&setup.env);
    assert_eq!(rotated_signers, replacement);
    assert_eq!(old_threshold, 3);
    assert_eq!(new_threshold, 2);

    assert_eq!(setup.client.signers(), replacement);
    assert_eq!(setup.client.threshold(), 2);
    assert!(setup.client.pending_rotation().is_none());

    let result = setup
        .client
        .try_execute_rotate_signers(&setup.signers.get(0).unwrap());
    assert_eq!(result, Err(Ok(NamesError::NotSigner)));
    let result = setup
        .client
        .try_execute_rotate_signers(&replacement.get(0).unwrap());
    assert_eq!(result, Err(Ok(NamesError::NoPendingRotation)));
}

#[test]
fn rotation_enforces_the_full_seven_day_delay_after_quorum() {
    let setup = setup();
    let replacement = new_signers(&setup.env);
    let proposer = setup.signers.get(0).unwrap();

    setup
        .client
        .propose_rotate_signers(&proposer, &replacement, &2);
    setup
        .client
        .approve_rotate_signers(&setup.signers.get(1).unwrap());
    setup
        .client
        .approve_rotate_signers(&setup.signers.get(2).unwrap());

    let result = setup.client.try_execute_rotate_signers(&proposer);
    assert_eq!(result, Err(Ok(NamesError::TimelockNotElapsed)));

    advance_time(&setup.env, ROTATION_TIMELOCK_SECS - 1);
    let result = setup.client.try_execute_rotate_signers(&proposer);
    assert_eq!(result, Err(Ok(NamesError::TimelockNotElapsed)));

    advance_time(&setup.env, 1);
    setup.client.execute_rotate_signers(&proposer);
    assert_eq!(setup.client.signers(), replacement);
}

#[test]
fn only_current_signers_can_propose_approve_execute_or_cancel() {
    let setup = setup();
    let replacement = new_signers(&setup.env);
    let outsider = Address::generate(&setup.env);
    let proposer = setup.signers.get(0).unwrap();

    let result = setup
        .client
        .try_propose_rotate_signers(&outsider, &replacement, &2);
    assert_eq!(result, Err(Ok(NamesError::NotSigner)));

    setup
        .client
        .propose_rotate_signers(&proposer, &replacement, &2);

    let result = setup.client.try_approve_rotate_signers(&outsider);
    assert_eq!(result, Err(Ok(NamesError::NotSigner)));
    let result = setup.client.try_execute_rotate_signers(&outsider);
    assert_eq!(result, Err(Ok(NamesError::NotSigner)));
    let result = setup.client.try_cancel_rotate_signers(&outsider);
    assert_eq!(result, Err(Ok(NamesError::NotSigner)));

    let result = setup
        .client
        .try_approve_rotate_signers(&setup.signers.get(0).unwrap());
    assert_eq!(result, Err(Ok(NamesError::AlreadyApprovedRotation)));
}

#[test]
fn cancellation_clears_the_proposal_and_its_approvals() {
    let setup = setup();
    let replacement = new_signers(&setup.env);
    let proposer = setup.signers.get(0).unwrap();
    let second_signer = setup.signers.get(1).unwrap();

    setup
        .client
        .propose_rotate_signers(&proposer, &replacement, &2);
    setup.client.approve_rotate_signers(&second_signer);
    setup.client.cancel_rotate_signers(&second_signer);
    assert!(setup.client.pending_rotation().is_none());

    let result = setup.client.try_approve_rotate_signers(&second_signer);
    assert_eq!(result, Err(Ok(NamesError::NoPendingRotation)));
    let result = setup.client.try_execute_rotate_signers(&proposer);
    assert_eq!(result, Err(Ok(NamesError::NoPendingRotation)));

    setup
        .client
        .propose_rotate_signers(&proposer, &replacement, &2);
    let pending = setup.client.pending_rotation().unwrap();
    assert_eq!(pending.approvals.len(), 1);

    advance_time(&setup.env, ROTATION_TIMELOCK_SECS);
    let result = setup.client.try_execute_rotate_signers(&proposer);
    assert_eq!(result, Err(Ok(NamesError::QuorumNotMet)));
}
