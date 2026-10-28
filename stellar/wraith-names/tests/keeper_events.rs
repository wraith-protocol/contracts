//! Golden fixture of the real events the names contract emits, for the TTL keeper.
//!
//! The keeper enumerates names from contract events (the contract keeps no name index).
//! `scripts/keeper/fixtures/contract-events.json` holds the exact events this contract
//! publishes for register, bulk_register, update and release, as XDR hex, and the keeper
//! tests decode and replay that file. This test regenerates the events from the contract
//! and fails if they differ from the committed fixture, so the fixture cannot drift from
//! what the contract really does. After an intentional change to the event surface run:
//!
//!   UPDATE_KEEPER_FIXTURE=1 cargo test -p wraith-names --test keeper_events

use soroban_sdk::{
    testutils::{Address as _, Events, Ledger},
    xdr::ToXdr,
    Address, Bytes, Env, String, Val, Vec,
};
use wraith_names::{WraithNamesContract, WraithNamesContractClient};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../scripts/keeper/fixtures/contract-events.json"
);

fn hex(env: &Env, v: Val) -> std::string::String {
    let bytes = v.to_xdr(env);
    let mut out = std::string::String::new();
    for i in 0..bytes.len() {
        out.push_str(&std::format!("{:02x}", bytes.get(i).unwrap()));
    }
    out
}

fn dump(env: &Env, ledger: u32, out: &mut std::vec::Vec<std::string::String>) {
    for (_contract, topics, data) in env.events().all().iter() {
        let t: std::vec::Vec<std::string::String> = topics
            .iter()
            .map(|x| std::format!("\"{}\"", hex(env, x)))
            .collect();
        out.push(std::format!(
            "    {{\"ledger\": {}, \"topics\": [{}], \"value\": \"{}\"}}",
            ledger,
            t.join(", "),
            hex(env, data)
        ));
    }
}

fn s(env: &Env, v: &str) -> String {
    String::from_str(env, v)
}

fn meta(env: &Env, fill: u8) -> Bytes {
    Bytes::from_slice(env, &[fill; 64])
}

fn scenario() -> std::string::String {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &id);
    let owner = Address::generate(&env);
    let mut lines = std::vec::Vec::new();

    env.ledger().with_mut(|l| l.sequence_number = 100);
    client.register(&owner, &s(&env, "solo"), &meta(&env, 1));
    dump(&env, 100, &mut lines);

    env.ledger().with_mut(|l| l.sequence_number = 101);
    let names = Vec::from_array(
        &env,
        [s(&env, "alpha"), s(&env, "bravo"), s(&env, "charlie")],
    );
    let metas = Vec::from_array(&env, [meta(&env, 2), meta(&env, 3), meta(&env, 4)]);
    client.bulk_register(&owner, &names, &metas);
    dump(&env, 101, &mut lines);

    env.ledger().with_mut(|l| l.sequence_number = 102);
    client.update(&owner, &s(&env, "alpha"), &meta(&env, 5));
    dump(&env, 102, &mut lines);

    env.ledger().with_mut(|l| l.sequence_number = 103);
    client.release(&owner, &s(&env, "bravo"));
    dump(&env, 103, &mut lines);

    std::format!("{{\n  \"events\": [\n{}\n  ]\n}}\n", lines.join(",\n"))
}

#[test]
fn contract_events_match_the_keeper_fixture() {
    let actual = scenario();
    assert_eq!(actual, scenario(), "event output must be deterministic");

    if std::env::var("UPDATE_KEEPER_FIXTURE").is_ok() {
        std::fs::create_dir_all(std::path::Path::new(FIXTURE).parent().unwrap()).unwrap();
        std::fs::write(FIXTURE, &actual).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(FIXTURE).unwrap_or_else(|_| {
        std::panic!("missing {FIXTURE}; run with UPDATE_KEEPER_FIXTURE=1 to create it")
    });
    assert_eq!(
        committed, actual,
        "the contract's events changed; if intended, regenerate with \
         UPDATE_KEEPER_FIXTURE=1 cargo test -p wraith-names --test keeper_events"
    );
}

/// Documents the surface the keeper depends on: bulk_register publishes a `register`
/// event per name (carrying the plaintext name) before the aggregate `bulk_reg` event.
#[test]
fn bulk_register_emits_a_register_event_per_name_then_bulk_reg() {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &id);
    let owner = Address::generate(&env);
    let names = Vec::from_array(&env, [s(&env, "alpha"), s(&env, "bravo")]);
    let metas = Vec::from_array(&env, [meta(&env, 2), meta(&env, 3)]);
    client.bulk_register(&owner, &names, &metas);

    let topics: std::vec::Vec<std::string::String> = env
        .events()
        .all()
        .iter()
        .map(|(_, t, _)| std::format!("{:?}", t.get(0).unwrap()))
        .collect();
    let registers = topics.iter().filter(|t| t.contains("register")).count();
    let bulk = topics.iter().filter(|t| t.contains("bulk_reg")).count();
    assert_eq!(registers, 2, "one register event per bulk-registered name");
    assert_eq!(bulk, 1);
    assert!(topics.last().unwrap().contains("bulk_reg"));
}
