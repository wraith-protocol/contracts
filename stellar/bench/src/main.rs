use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::{vec, Address, Bytes, BytesN, Env, String as SorobanString, Vec as SorobanVec};

use stealth_announcer::{StealthAnnouncerContract, StealthAnnouncerContractClient};
use stealth_registry::{StealthRegistryContract, StealthRegistryContractClient};
use stealth_sender::{StealthSenderContract, StealthSenderContractClient};
use wraith_names::{WraithNamesContract, WraithNamesContractClient};

#[derive(Clone)]
struct Row {
    contract: &'static str,
    function: &'static str,
    params: std::string::String,
    instructions: i64,
    mem_bytes: i64,
    read_entries: u32,
    write_entries: u32,
    read_bytes: u32,
    write_bytes: u32,
    events_bytes: u32,
}

fn main() {
    let mut rows = std::vec::Vec::new();

    for metadata_len in [1u32, 32, 256, 1024, 4096] {
        rows.push(measure(
            "stealth-announcer",
            "announce",
            format!("metadata_len={metadata_len}"),
            |env| {
                let contract_id = env.register(StealthAnnouncerContract, ());
                let client = StealthAnnouncerContractClient::new(env, &contract_id);
                client.announce(
                    &stealth_announcer::STELLAR_V2_SCHEME_ID,
                    &Address::generate(env),
                    &BytesN::from_array(env, &[7u8; 32]),
                    &bytes(env, metadata_len, 9),
                );
            },
        ));
    }

    rows.push(measure(
        "stealth-registry",
        "register_keys",
        "first_time".into(),
        |env| {
            env.mock_all_auths();
            let contract_id = env.register(StealthRegistryContract, ());
            let client = StealthRegistryContractClient::new(env, &contract_id);
            client.register_keys(&Address::generate(env), &1, &bytes(env, 64, 1));
        },
    ));

    rows.push(measure(
        "stealth-registry",
        "register_keys",
        "replacement".into(),
        |env| {
            env.mock_all_auths();
            let contract_id = env.register(StealthRegistryContract, ());
            let client = StealthRegistryContractClient::new(env, &contract_id);
            let registrant = Address::generate(env);
            client.register_keys(&registrant, &1, &bytes(env, 64, 1));
            client.register_keys(&registrant, &1, &bytes(env, 64, 2));
        },
    ));

    for asset in ["xlm", "issued"] {
        rows.push(measure(
            "stealth-sender",
            "send",
            format!("asset={asset}"),
            |env| {
                env.mock_all_auths();
                let sender_contract_id = env.register(StealthSenderContract, ());
                let announcer_id = env.register(StealthAnnouncerContract, ());
                let client = StealthSenderContractClient::new(env, &sender_contract_id);
                client.init(&announcer_id, &None, &None, &0);
                let (token, sender) = funded_token(env, asset == "xlm");
                client.send(
                    &sender,
                    &token,
                    &100,
                    &stealth_announcer::STELLAR_V2_SCHEME_ID,
                    &Address::generate(env),
                    &BytesN::from_array(env, &[3u8; 32]),
                    &bytes(env, 32, 4),
                );
            },
        ));
    }

    for batch_size in [1u32, 5, 10, 25] {
        rows.push(measure(
            "stealth-sender",
            "batch_send",
            format!("batch_size={batch_size}"),
            |env| {
                env.mock_all_auths();
                let sender_contract_id = env.register(StealthSenderContract, ());
                let announcer_id = env.register(StealthAnnouncerContract, ());
                let client = StealthSenderContractClient::new(env, &sender_contract_id);
                client.init(&announcer_id, &None, &None, &0);
                let (token, sender) = funded_token(env, true);
                let mut addresses: SorobanVec<Address> = vec![env];
                let mut keys: SorobanVec<BytesN<32>> = vec![env];
                let mut metadatas: SorobanVec<Bytes> = vec![env];
                let mut amounts: SorobanVec<i128> = vec![env];
                for i in 0..batch_size {
                    addresses.push_back(Address::generate(env));
                    keys.push_back(BytesN::from_array(env, &[i as u8; 32]));
                    metadatas.push_back(bytes(env, 32, i as u8));
                    amounts.push_back(100);
                }
                client.batch_send(
                    &sender,
                    &token,
                    &stealth_announcer::STELLAR_V2_SCHEME_ID,
                    &addresses,
                    &keys,
                    &metadatas,
                    &amounts,
                );
            },
        ));
    }

    for batch_size in [1u32, 5, 10, 20] {
        rows.push(measure(
            "stealth-sender",
            "sponsored_announce",
            format!("batch_size={batch_size}"),
            |env| {
                env.mock_all_auths();
                let sender_contract_id = env.register(StealthSenderContract, ());
                let announcer_id = env.register(StealthAnnouncerContract, ());
                let client = StealthSenderContractClient::new(env, &sender_contract_id);
                client.init(&announcer_id, &None, &None, &0);
                let (token, sender) = funded_token(env, true);
                let sponsor = Address::generate(env);
                let mut entries: SorobanVec<stealth_sender::SponsoredEntry> = vec![env];
                for i in 0..batch_size {
                    entries.push_back(stealth_sender::SponsoredEntry {
                        sender: sender.clone(),
                        token: token.clone(),
                        amount: 100,
                        scheme_id: stealth_announcer::STELLAR_V2_SCHEME_ID,
                        stealth_address: Address::generate(env),
                        ephemeral_pub_key: BytesN::from_array(env, &[i as u8; 32]),
                        metadata: bytes(env, 32, i as u8),
                    });
                }
                client.sponsored_announce(&sponsor, &entries);
            },
        ));
    }

    for name_len in [3u32, 32] {
        rows.push(measure(
            "wraith-names",
            "register",
            format!("name_len={name_len}"),
            |env| {
                env.mock_all_auths();
                let contract_id = env.register(WraithNamesContract, ());
                let client = WraithNamesContractClient::new(env, &contract_id);
                client.register(
                    &Address::generate(env),
                    &name(env, name_len),
                    &bytes(env, 64, 5),
                );
            },
        ));
    }

    rows.push(measure("wraith-names", "resolve", "hit".into(), |env| {
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(env, &contract_id);
        let n = SorobanString::from_str(env, "alice");
        client.register(&Address::generate(env), &n, &bytes(env, 64, 6));
        client.resolve(&n);
    }));

    rows.push(measure("wraith-names", "resolve", "miss".into(), |env| {
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(env, &contract_id);
        let _ = client.try_resolve(&SorobanString::from_str(env, "missing"));
    }));

    rows.push(measure("wraith-names", "name_of", "hit".into(), |env| {
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(env, &contract_id);
        let meta = bytes(env, 64, 7);
        client.register(
            &Address::generate(env),
            &SorobanString::from_str(env, "charlie"),
            &meta,
        );
        client.name_of(&meta);
    }));

    rows.push(measure("wraith-names", "name_of", "miss".into(), |env| {
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(env, &contract_id);
        let _ = client.try_name_of(&bytes(env, 64, 8));
    }));

    print_markdown(&rows);
}

fn measure<F>(
    contract: &'static str,
    function: &'static str,
    params: std::string::String,
    f: F,
) -> Row
where
    F: FnOnce(&Env),
{
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();
    f(&env);
    let resources = env.cost_estimate().resources();
    Row {
        contract,
        function,
        params,
        instructions: resources.instructions,
        mem_bytes: resources.mem_bytes,
        read_entries: resources.read_entries,
        write_entries: resources.write_entries,
        read_bytes: resources.read_bytes,
        write_bytes: resources.write_bytes,
        events_bytes: resources.contract_events_size_bytes,
    }
}

fn funded_token(env: &Env, native: bool) -> (Address, Address) {
    let admin = Address::generate(env);
    let sender = Address::generate(env);
    let token = if native {
        env.register_stellar_asset_contract_v2(admin.clone())
            .address()
    } else {
        env.register_stellar_asset_contract_v2(Address::generate(env))
            .address()
    };
    let asset = StellarAssetClient::new(env, &token);
    asset.mint(&sender, &1_000_000);
    (token, sender)
}

fn bytes(env: &Env, len: u32, fill: u8) -> Bytes {
    let mut out = Bytes::new(env);
    for _ in 0..len {
        out.push_back(fill);
    }
    out
}

fn name(env: &Env, len: u32) -> SorobanString {
    let raw = "abcdefghijklmnopqrstuvwxyz012345";
    SorobanString::from_str(env, &raw[..len as usize])
}

fn print_markdown(rows: &[Row]) {
    println!("| Contract | Function | Parameters | Instructions | Mem bytes | Read entries | Write entries | Read bytes | Write bytes | Event bytes |");
    println!("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|");
    for row in rows {
        println!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            row.contract,
            row.function,
            row.params,
            row.instructions,
            row.mem_bytes,
            row.read_entries,
            row.write_entries,
            row.read_bytes,
            row.write_bytes,
            row.events_bytes,
        );
    }
}
