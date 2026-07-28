use soroban_sdk::testutils::EnvTestConfig;
use soroban_sdk::{vec, Address, Bytes, BytesN, Env};
use stealth_announcer::{
    StealthAnnouncerContract, StealthAnnouncerContractClient, METADATA_KIND_VIEW_TAG,
    STELLAR_V2_SCHEME_ID,
};

fn test_env() -> Env {
    Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    })
}

#[test]
fn integration_dual_emit_present() {
    let env = test_env();
    let contract_id = env.register(StealthAnnouncerContract, ());
    let client = StealthAnnouncerContractClient::new(&env, &contract_id);

    let stealth_address = Address::generate(&env);
    let ephemeral_pub_key = BytesN::from_array(&env, &[9u8; 32]);
    let metadata = Bytes::from_slice(&env, &[7u8; 2]);

    client.announce(
        &STELLAR_V2_SCHEME_ID,
        &stealth_address,
        &ephemeral_pub_key,
        &metadata,
    );

    let events = env.events().all();
    // must have both v2 (4-topic) and legacy v1 (3-topic)
    let mut found_v2 = false;
    let mut found_v1 = false;
    for e in events.iter() {
        if e.1.len() == 4 {
            found_v2 = true;
            assert_eq!(e.1[0], soroban_sdk::Symbol::short("announce").into_val(&env));
        } else if e.1.len() == 3 {
            found_v1 = true;
            assert_eq!(e.1[0], soroban_sdk::Symbol::short("announce").into_val(&env));
        }
    }

    assert!(found_v2 && found_v1, "both v2 and legacy v1 events should be emitted");
}
