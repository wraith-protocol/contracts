//! Asserts the shape of the WraithMetricEvent stream emitted by wraith-names.
//!
//! The wire contract that off-chain indexers depend on is
//! `(("metric", contract, metric_name), (value, dimensions))` — see
//! `stellar/METRICS.md`. These tests pin both halves.

use soroban_sdk::testutils::{Address as _, Events};
use soroban_sdk::{symbol_short, vec, Address, Bytes, Env, IntoVal, String, Val, Vec};
use wraith_metrics::{contract_ids, metric_names};
use wraith_names::{WraithNamesContract, WraithNamesContractClient};

fn setup() -> (Env, WraithNamesContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);
    let owner = Address::generate(&env);
    (env, client, owner)
}

/// Return the (topics, data) of the single metric event in the buffer,
/// panicking when the count is anything other than one.
fn only_metric_event(env: &Env) -> (Vec<Val>, Val) {
    let metric = symbol_short!("metric").into_val(env);
    let mut found: Option<(Vec<Val>, Val)> = None;

    for (_, topics, data) in env.events().all().iter() {
        if topics.first().map(|t: Val| t.shallow_eq(&metric)) == Some(true) {
            assert!(found.is_none(), "expected exactly one metric event");
            found = Some((topics, data));
        }
    }

    found.expect("no metric event emitted")
}

fn assert_metric(
    env: &Env,
    topics: &Vec<Val>,
    data: &Val,
    metric_name: soroban_sdk::Symbol,
    value: i128,
) {
    let expected_topics: Vec<Val> = vec![
        env,
        symbol_short!("metric").into_val(env),
        contract_ids::WRAITH_NAMES.into_val(env),
        metric_name.into_val(env),
    ];
    assert_eq!(topics, &expected_topics);

    let (emitted_value, dimensions): (i128, Vec<(soroban_sdk::Symbol, Val)>) = data.into_val(env);
    assert_eq!(emitted_value, value);
    assert_eq!(dimensions.len(), 0, "names metrics carry no dimensions");
}

fn meta(env: &Env, byte: u8) -> Bytes {
    Bytes::from_slice(env, &[byte; 64])
}

#[test]
fn register_emits_register_count_metric() {
    let (env, client, owner) = setup();

    client.register(&owner, &String::from_str(&env, "alice"), &meta(&env, 1));

    let (topics, data) = only_metric_event(&env);
    assert_metric(&env, &topics, &data, metric_names::REGISTER_COUNT, 1);
}

#[test]
fn release_emits_release_count_metric() {
    let (env, client, owner) = setup();
    let name = String::from_str(&env, "alice");

    client.register(&owner, &name, &meta(&env, 1));
    client.release(&owner, &name);

    let (topics, data) = only_metric_event(&env);
    assert_metric(&env, &topics, &data, metric_names::RELEASE_COUNT, 1);
}

#[test]
fn extend_name_ttl_emits_renew_count_metric() {
    let (env, client, owner) = setup();
    let name = String::from_str(&env, "alice");

    client.register(&owner, &name, &meta(&env, 1));
    let extend_to = env.ledger().sequence() + 10_000;
    client.extend_name_ttl(&name, &extend_to);

    let (topics, data) = only_metric_event(&env);
    assert_metric(&env, &topics, &data, metric_names::RENEW_COUNT, 1);
}

#[test]
fn bulk_renew_emits_renew_count_metric_carrying_batch_size() {
    let (env, client, owner) = setup();

    client.register(&owner, &String::from_str(&env, "alpha"), &meta(&env, 1));
    client.register(&owner, &String::from_str(&env, "beta"), &meta(&env, 2));

    let names = vec![
        &env,
        String::from_str(&env, "alpha"),
        String::from_str(&env, "beta"),
    ];
    let extend_to = env.ledger().sequence() + 10_000;
    client.bulk_renew(&names, &extend_to);

    let (topics, data) = only_metric_event(&env);
    assert_metric(&env, &topics, &data, metric_names::RENEW_COUNT, 2);
}

#[test]
fn resolve_emits_hit_metric() {
    let (env, client, owner) = setup();
    let name = String::from_str(&env, "alice");

    client.register(&owner, &name, &meta(&env, 1));
    client.resolve(&name);

    let (topics, data) = only_metric_event(&env);
    assert_metric(&env, &topics, &data, metric_names::RESOLVE_HIT_COUNT, 1);
}

#[test]
fn resolve_emits_miss_metric_before_returning_not_found() {
    let (env, client, _owner) = setup();

    assert!(client
        .try_resolve(&String::from_str(&env, "missing"))
        .is_err());

    let (topics, data) = only_metric_event(&env);
    assert_metric(&env, &topics, &data, metric_names::RESOLVE_MISS_COUNT, 1);
}
