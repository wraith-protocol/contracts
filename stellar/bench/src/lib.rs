use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::{
    contract, contractimpl, symbol_short, vec, Address, Bytes, BytesN, Env,
    String as SorobanString, Vec as SorobanVec,
};

use governance::{GovernanceContract, GovernanceContractClient};
use stealth_announcer::{
    StealthAnnouncerContract, StealthAnnouncerContractClient, STELLAR_V2_SCHEME_ID,
};
use stealth_registry::{StealthRegistryContract, StealthRegistryContractClient};
use stealth_sender::{StealthSenderContract, StealthSenderContractClient, WithdrawalEntry};
use stealth_splitter::{Beneficiary, StealthSplitterContract, StealthSplitterContractClient};
use stealth_vault::{StealthVaultContract, StealthVaultContractClient};
use wraith_names::{WraithNamesContract, WraithNamesContractClient};

#[derive(Clone, Debug)]
pub struct Row {
    pub contract: &'static str,
    pub function: &'static str,
    pub params: std::string::String,
    pub instructions: i64,
    pub mem_bytes: i64,
    pub read_entries: u32,
    pub write_entries: u32,
    pub read_bytes: u32,
    pub write_bytes: u32,
    pub events_bytes: u32,
}

impl Row {
    pub fn op_key(&self) -> std::string::String {
        format!("{}::{}::{}", self.contract, self.function, self.params)
    }
}

/// Collect per-op Soroban resource measurements for all harness cases.
pub fn collect_rows() -> std::vec::Vec<Row> {
    let mut rows = std::vec::Vec::new();

    // v2 announcer requires scheme_id=2 and non-empty metadata (byte 0 = view tag).
    for metadata_len in [1u32, 32, 256, 1024, 4096] {
        rows.push(measure(
            "stealth-announcer",
            "announce",
            format!("metadata_len={metadata_len}"),
            |env| {
                let contract_id = env.register(StealthAnnouncerContract, ());
                let client = StealthAnnouncerContractClient::new(env, &contract_id);
                client.announce(
                    &STELLAR_V2_SCHEME_ID,
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
                let admin = Address::generate(env);
                client.init(&announcer_id, &None, &None, &0, &admin);
                let (token, sender) = funded_token(env, asset == "xlm");
                client.send(
                    &sender,
                    &token,
                    &100,
                    &STELLAR_V2_SCHEME_ID,
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
                let admin = Address::generate(env);
                client.init(&announcer_id, &None, &None, &0, &admin);
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
                    &STELLAR_V2_SCHEME_ID,
                    &addresses,
                    &keys,
                    &metadatas,
                    &amounts,
                );
            },
        ));
    }

    for entries in [1u32, 10, 30] {
        rows.push(measure(
            "stealth-sender",
            "withdraw_many",
            format!("entries={entries}"),
            |env| {
                env.mock_all_auths();
                let sender_contract_id = env.register(StealthSenderContract, ());
                let client = StealthSenderContractClient::new(env, &sender_contract_id);
                let (token, withdrawer) = funded_token(env, true);
                client.withdraw_many(&withdrawer, &withdraw_entries(env, &token, entries));
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

    for count in [5u32, 10, 20] {
        rows.push(measure(
            "wraith-names",
            "bulk_register",
            format!("count={count}"),
            |env| {
                env.mock_all_auths();
                let contract_id = env.register(WraithNamesContract, ());
                let client = WraithNamesContractClient::new(env, &contract_id);
                client.bulk_register(
                    &Address::generate(env),
                    &numbered_names(env, count),
                    &numbered_metas(env, count),
                );
            },
        ));
    }

    for count in [5u32, 10, 20] {
        rows.push(measure(
            "wraith-names",
            "bulk_renew",
            format!("count={count}"),
            |env| {
                env.mock_all_auths();
                let contract_id = env.register(WraithNamesContract, ());
                let client = WraithNamesContractClient::new(env, &contract_id);
                let owner = Address::generate(env);
                let names = numbered_names(env, count);
                let metas = numbered_metas(env, count);
                for i in 0..count {
                    client.register(&owner, &names.get(i).unwrap(), &metas.get(i).unwrap());
                }
                client.bulk_renew(&names, &(env.ledger().sequence() + 10_000));
            },
        ));
    }

    rows.push(measure(
        "wraith-names",
        "extend_name_ttl",
        "existing".into(),
        |env| {
            env.mock_all_auths();
            let contract_id = env.register(WraithNamesContract, ());
            let client = WraithNamesContractClient::new(env, &contract_id);
            let n = SorobanString::from_str(env, "alice");
            client.register(&Address::generate(env), &n, &bytes(env, 64, 9));
            client.extend_name_ttl(&n, &(env.ledger().sequence() + 10_000));
        },
    ));

    for n in [5u32, 15, 25] {
        rows.push(measure(
            "stealth-splitter",
            "create_split",
            format!("beneficiaries={n}"),
            |env| {
                env.mock_all_auths();
                let client = splitter_client(env);
                client.create_split(
                    &Address::generate(env),
                    &split_beneficiaries(env, n),
                    &Address::generate(env),
                    &bytes(env, 8, 1),
                );
            },
        ));
    }

    for n in [5u32, 15, 25] {
        rows.push(measure(
            "stealth-splitter",
            "fund_split",
            format!("beneficiaries={n}"),
            |env| {
                env.mock_all_auths();
                let client = splitter_client(env);
                let (token, funder) = funded_token(env, true);
                let split_id = client.create_split(
                    &Address::generate(env),
                    &split_beneficiaries(env, n),
                    &token,
                    &bytes(env, 8, 2),
                );
                let mut addresses: SorobanVec<Address> = vec![env];
                let mut keys: SorobanVec<BytesN<32>> = vec![env];
                let mut metadatas: SorobanVec<Bytes> = vec![env];
                for i in 0..n {
                    addresses.push_back(Address::generate(env));
                    keys.push_back(BytesN::from_array(env, &[i as u8; 32]));
                    metadatas.push_back(bytes(env, 32, i as u8));
                }
                client.fund_split(
                    &funder,
                    &split_id,
                    &1_000,
                    &STELLAR_V2_SCHEME_ID,
                    &addresses,
                    &keys,
                    &metadatas,
                );
            },
        ));
    }

    for asset in ["xlm", "issued"] {
        rows.push(measure(
            "stealth-vault",
            "deposit",
            format!("asset={asset}"),
            |env| {
                env.mock_all_auths();
                let (client, token, sender, recipient) = vault_with_token(env, asset == "xlm");
                client.deposit(
                    &sender,
                    &recipient,
                    &1_000,
                    &token,
                    &VAULT_UNLOCK_LEDGER,
                    &VAULT_REFUND_AFTER,
                    &BytesN::from_array(env, &[11u8; 32]),
                );
            },
        ));
    }

    rows.push(measure(
        "stealth-vault",
        "claim",
        "unlocked".into(),
        |env| {
            env.mock_all_auths();
            let (client, token, sender, recipient) = vault_with_token(env, true);
            let deposit_id = client.deposit(
                &sender,
                &recipient,
                &1_000,
                &token,
                &VAULT_UNLOCK_LEDGER,
                &VAULT_REFUND_AFTER,
                &BytesN::from_array(env, &[12u8; 32]),
            );
            env.ledger()
                .with_mut(|li| li.sequence_number = VAULT_UNLOCK_LEDGER);
            client.claim(&deposit_id, &recipient);
        },
    ));

    rows.push(measure(
        "stealth-vault",
        "refund",
        "depositor".into(),
        |env| {
            env.mock_all_auths();
            let (client, token, sender, recipient) = vault_with_token(env, true);
            let deposit_id = client.deposit(
                &sender,
                &recipient,
                &1_000,
                &token,
                &VAULT_UNLOCK_LEDGER,
                &VAULT_REFUND_AFTER,
                &BytesN::from_array(env, &[13u8; 32]),
            );
            env.ledger()
                .with_mut(|li| li.sequence_number = VAULT_REFUND_AFTER);
            client.refund(&deposit_id);
        },
    ));

    rows.push(measure(
        "stealth-vault",
        "refund_permissionless",
        "keeper".into(),
        |env| {
            env.mock_all_auths();
            let (client, token, sender, recipient) = vault_with_token(env, true);
            let deposit_id = client.deposit(
                &sender,
                &recipient,
                &1_000,
                &token,
                &VAULT_UNLOCK_LEDGER,
                &VAULT_REFUND_AFTER,
                &BytesN::from_array(env, &[14u8; 32]),
            );
            env.ledger().with_mut(|li| {
                li.sequence_number = VAULT_REFUND_AFTER + stealth_vault::DEFAULT_GRACE_PERIOD
            });
            client.refund_permissionless(&Address::generate(env), &deposit_id);
        },
    ));

    rows.push(measure(
        "governance",
        "propose",
        "happy_path".into(),
        |env| {
            env.mock_all_auths();
            let (client, _token, target) = governance_client(env);
            client.propose(
                &Address::generate(env),
                &target,
                &symbol_short!("set_value"),
                &bytes(env, 16, 1),
                &SorobanString::from_str(env, "bench"),
            );
        },
    ));

    rows.push(measure("governance", "vote", "happy_path".into(), |env| {
        env.mock_all_auths();
        let (client, token, target) = governance_client(env);
        let voter = Address::generate(env);
        StellarAssetClient::new(env, &token).mint(&voter, &200);
        let pid = client.propose(
            &Address::generate(env),
            &target,
            &symbol_short!("set_value"),
            &bytes(env, 16, 1),
            &SorobanString::from_str(env, "bench"),
        );
        client.vote(&voter, &pid, &true);
    }));

    rows.push(measure(
        "governance",
        "execute",
        "happy_path".into(),
        |env| {
            env.mock_all_auths();
            let (client, token, target) = governance_client(env);
            let voter = Address::generate(env);
            StellarAssetClient::new(env, &token).mint(&voter, &200);
            let pid = client.propose(
                &Address::generate(env),
                &target,
                &symbol_short!("set_value"),
                &bytes(env, 16, 1),
                &SorobanString::from_str(env, "bench"),
            );
            client.vote(&voter, &pid, &true);
            let proposal = client.get_proposal(&pid);
            env.ledger().with_mut(|li| {
                li.sequence_number = proposal.end_ledger + 20;
            });
            client.execute(&pid);
        },
    ));

    rows
}

pub fn print_markdown(rows: &[Row]) {
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

/// Serialize bench rows to a stable JSON document used for CI baselines.
///
/// Every row uses the same object shape (`contract`, `function`, `params`,
/// resource counters). New `collect_rows` cases are extra `results` entries;
/// existing keys and field names stay unchanged.
pub fn to_json(rows: &[Row], meta: &BenchMeta) -> std::string::String {
    let mut out = std::string::String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"generated_at\": \"{}\",\n",
        escape_json(&meta.generated_at)
    ));
    out.push_str(&format!(
        "  \"commit\": \"{}\",\n",
        escape_json(&meta.commit)
    ));
    out.push_str(&format!("  \"threshold_pct\": {},\n", meta.threshold_pct));
    out.push_str("  \"results\": [\n");
    for (i, row) in rows.iter().enumerate() {
        out.push_str("    {\n");
        out.push_str(&format!(
            "      \"contract\": \"{}\",\n",
            escape_json(row.contract)
        ));
        out.push_str(&format!(
            "      \"function\": \"{}\",\n",
            escape_json(row.function)
        ));
        out.push_str(&format!(
            "      \"params\": \"{}\",\n",
            escape_json(&row.params)
        ));
        out.push_str(&format!("      \"instructions\": {},\n", row.instructions));
        out.push_str(&format!("      \"mem_bytes\": {},\n", row.mem_bytes));
        out.push_str(&format!("      \"read_entries\": {},\n", row.read_entries));
        out.push_str(&format!(
            "      \"write_entries\": {},\n",
            row.write_entries
        ));
        out.push_str(&format!("      \"read_bytes\": {},\n", row.read_bytes));
        out.push_str(&format!("      \"write_bytes\": {},\n", row.write_bytes));
        out.push_str(&format!("      \"events_bytes\": {}\n", row.events_bytes));
        out.push_str("    }");
        if i + 1 < rows.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ]\n");
    out.push_str("}\n");
    out
}

/// Markdown table for PERF.md. One data row per collected case; column set is
/// fixed so new entrypoints do not change the existing table shape.
pub fn markdown_table(rows: &[Row]) -> std::string::String {
    let mut out = std::string::String::new();
    out.push_str("| Contract | Function | Parameters | Instructions | Mem bytes | Read entries | Write entries | Read bytes | Write bytes | Event bytes |\n");
    out.push_str("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for row in rows {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
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
        ));
    }
    out
}

#[derive(Clone, Debug)]
pub struct BenchMeta {
    pub generated_at: std::string::String,
    pub commit: std::string::String,
    pub threshold_pct: f64,
}

impl Default for BenchMeta {
    fn default() -> Self {
        Self {
            generated_at: utc_now(),
            commit: std::env::var("GITHUB_SHA")
                .or_else(|_| std::env::var("COMMIT_SHA"))
                .unwrap_or_else(|_| "unknown".into()),
            threshold_pct: 5.0,
        }
    }
}

fn utc_now() -> std::string::String {
    // Prefer a stable ISO-ish stamp from the environment when available.
    if let Ok(ts) = std::env::var("BENCH_GENERATED_AT") {
        return ts;
    }
    // Fallback: unix epoch seconds (deterministic enough for local runs).
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => format!("{}", d.as_secs()),
        Err(_) => "0".into(),
    }
}

fn escape_json(s: &str) -> std::string::String {
    let mut out = std::string::String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
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

/// Vault deposits must clear `unlock_ledger + grace_period`; these constants keep
/// every vault row on the same window so the numbers stay comparable.
const VAULT_UNLOCK_LEDGER: u32 = 100;
const VAULT_REFUND_AFTER: u32 = 2_000;

/// A registered, initialised vault plus a funded sender and a recipient.
fn vault_with_token(
    env: &Env,
    native: bool,
) -> (
    StealthVaultContractClient<'static>,
    Address,
    Address,
    Address,
) {
    let announcer_id = env.register(StealthAnnouncerContract, ());
    let vault_id = env.register(StealthVaultContract, ());
    let client = StealthVaultContractClient::new(env, &vault_id);
    client.init(&Address::generate(env), &announcer_id);
    let (token, sender) = funded_token(env, native);
    (client, token, sender, Address::generate(env))
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

fn numbered_names(env: &Env, n: u32) -> SorobanVec<SorobanString> {
    let mut names = vec![env];
    for i in 0..n {
        names.push_back(SorobanString::from_str(env, &format!("n{i:02}")));
    }
    names
}

fn numbered_metas(env: &Env, n: u32) -> SorobanVec<Bytes> {
    let mut metas = vec![env];
    for i in 0..n {
        metas.push_back(bytes(env, 64, i as u8));
    }
    metas
}

fn withdraw_entries(env: &Env, token: &Address, n: u32) -> SorobanVec<WithdrawalEntry> {
    let mut entries = vec![env];
    for _ in 0..n {
        entries.push_back(WithdrawalEntry {
            token: token.clone(),
            to: Address::generate(env),
            amount: 100,
        });
    }
    entries
}

fn splitter_client(env: &Env) -> StealthSplitterContractClient<'static> {
    let announcer_id = env.register(StealthAnnouncerContract, ());
    let splitter_id = env.register(StealthSplitterContract, ());
    let client = StealthSplitterContractClient::new(env, &splitter_id);
    client.init(&announcer_id);
    client
}

fn split_beneficiaries(env: &Env, n: u32) -> SorobanVec<Beneficiary> {
    let mut beneficiaries = vec![env];
    for i in 0..n {
        beneficiaries.push_back(Beneficiary {
            meta_address: bytes(env, 64, i as u8),
            weight: 1,
        });
    }
    beneficiaries
}

fn governance_client(env: &Env) -> (GovernanceContractClient<'static>, Address, Address) {
    let token = env
        .register_stellar_asset_contract_v2(Address::generate(env))
        .address();
    let gov_id = env.register(GovernanceContract, ());
    let client = GovernanceContractClient::new(env, &gov_id);
    client.init(&Address::generate(env), &token, &100i128, &50u32, &10u32);
    (client, token, env.register(BenchGovTarget, ()))
}

/// Minimal target so `governance::execute` has a real contract to invoke.
#[contract]
struct BenchGovTarget;

#[contractimpl]
impl BenchGovTarget {
    pub fn set_value(env: Env, value: Bytes) {
        env.storage()
            .instance()
            .set(&symbol_short!("value"), &value);
    }
}
