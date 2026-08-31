//! Batch vs individual send crossover measurements.

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::{vec, Address, Bytes, BytesN, Env, Vec as SorobanVec};

use stealth_announcer::StealthAnnouncerContract;
use stealth_batch_sender::{StealthBatchSender, StealthBatchSenderClient, Transfer};
use stealth_sender::{StealthSenderContract, StealthSenderContractClient};

pub const CROSSOVER_SIZES: [u32; 6] = [1, 2, 5, 10, 15, 20];

#[derive(Clone, Debug)]
pub struct CrossoverRow {
    pub n: u32,
    pub individual_instructions: i64,
    pub individual_wall_ns: u128,
    pub individual_instr_per_entry: i64,
    pub individual_wall_ns_per_entry: u128,
    pub batch_instructions: i64,
    pub batch_wall_ns: u128,
    pub batch_instr_per_entry: i64,
    pub batch_wall_ns_per_entry: u128,
}

struct Measured {
    instructions: i64,
    wall_ns: u128,
}

pub fn run_and_report(write_data: bool) -> (std::vec::Vec<CrossoverRow>, Option<u32>) {
    let mut rows = std::vec::Vec::with_capacity(CROSSOVER_SIZES.len());

    for &n in &CROSSOVER_SIZES {
        let individual = measure_individual(n);
        let batch = measure_batch(n);
        rows.push(CrossoverRow {
            n,
            individual_instructions: individual.instructions,
            individual_wall_ns: individual.wall_ns,
            individual_instr_per_entry: individual.instructions / i64::from(n),
            individual_wall_ns_per_entry: individual.wall_ns / u128::from(n),
            batch_instructions: batch.instructions,
            batch_wall_ns: batch.wall_ns,
            batch_instr_per_entry: batch.instructions / i64::from(n),
            batch_wall_ns_per_entry: batch.wall_ns / u128::from(n),
        });
    }

    let crossover = rows
        .iter()
        .find(|r| r.batch_instructions < r.individual_instructions)
        .map(|r| r.n);

    print_crossover_markdown(&rows, crossover);
    if write_data {
        write_chart_data(&rows, crossover);
    }
    (rows, crossover)
}

fn measure_individual(n: u32) -> Measured {
    let mut total_instructions = 0i64;
    let mut total_wall_ns = 0u128;
    for i in 0..n {
        let env = Env::default();
        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();

        let sender_contract_id = env.register(StealthSenderContract, ());
        let announcer_id = env.register(StealthAnnouncerContract, ());
        let client = StealthSenderContractClient::new(&env, &sender_contract_id);
        let admin = Address::generate(&env);
        client.init(&announcer_id, &None, &None, &0, &admin);
        let (token, sender) = funded_token(&env);

        env.cost_estimate().budget().reset_unlimited();
        let start = Instant::now();
        client.send(
            &sender,
            &token,
            &100,
            &2,
            &Address::generate(&env),
            &BytesN::from_array(&env, &[(i as u8).wrapping_add(1); 32]),
            &bytes(&env, 32, (i as u8).wrapping_add(1)),
        );
        total_wall_ns += start.elapsed().as_nanos();
        total_instructions += env.cost_estimate().resources().instructions;
    }
    Measured {
        instructions: total_instructions,
        wall_ns: total_wall_ns,
    }
}

fn measure_batch(n: u32) -> Measured {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    let contract_id = env.register(StealthBatchSender, ());
    let client = StealthBatchSenderClient::new(&env, &contract_id);
    // batch_send now requires init(admin, announcer, asset_policy) (issue #155)
    // and routes announcements through the real announcer (issue #63).
    let admin = Address::generate(&env);
    let announcer = env.register(StealthAnnouncerContract, ());
    client.init(&admin, &announcer, &None);
    let (token, sender) = funded_token(&env);

    let mut transfers: SorobanVec<Transfer> = vec![&env];
    for i in 0..n {
        transfers.push_back(Transfer {
            stealth_address: Address::generate(&env),
            ephemeral_pub_key: bytes(&env, 32, 0x02u8.wrapping_add(i as u8)),
            amount: 100,
            metadata: bytes(&env, 1, (i as u8).wrapping_add(1)),
        });
    }

    env.cost_estimate().budget().reset_unlimited();
    let start = Instant::now();
    client.batch_send(&sender, &transfers, &token);
    Measured {
        instructions: env.cost_estimate().resources().instructions,
        wall_ns: start.elapsed().as_nanos(),
    }
}

fn funded_token(env: &Env) -> (Address, Address) {
    let admin = Address::generate(env);
    let sender = Address::generate(env);
    let token = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    StellarAssetClient::new(env, &token).mint(&sender, &1_000_000);
    (token, sender)
}

fn bytes(env: &Env, len: u32, fill: u8) -> Bytes {
    let mut out = Bytes::new(env);
    for _ in 0..len {
        out.push_back(fill);
    }
    out
}

fn print_crossover_markdown(rows: &[CrossoverRow], crossover: Option<u32>) {
    println!(
        "| N | Individual instr | Batch instr | Indiv /entry | Batch /entry | Individual ns | Batch ns | Indiv ns/entry | Batch ns/entry | Winner |"
    );
    println!("|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|");
    for row in rows {
        let winner = if row.batch_instructions < row.individual_instructions {
            "batch"
        } else if row.batch_instructions > row.individual_instructions {
            "individual"
        } else {
            "tie"
        };
        println!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            row.n,
            row.individual_instructions,
            row.batch_instructions,
            row.individual_instr_per_entry,
            row.batch_instr_per_entry,
            row.individual_wall_ns,
            row.batch_wall_ns,
            row.individual_wall_ns_per_entry,
            row.batch_wall_ns_per_entry,
            winner,
        );
    }
    println!();
    match crossover {
        Some(n) => println!(
            "Instruction crossover: `stealth-batch-sender` becomes cheaper at **N = {n}**."
        ),
        None => println!(
            "Instruction crossover: `stealth-batch-sender` did not undercut N individual `stealth-sender::send` calls in the measured range (1..=20)."
        ),
    }
}

fn data_dir() -> PathBuf {
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        return PathBuf::from(manifest).join("../bench/data");
    }
    PathBuf::from("bench/data")
}

fn write_chart_data(rows: &[CrossoverRow], crossover: Option<u32>) {
    let dir = data_dir();
    let _ = fs::create_dir_all(&dir);

    let csv_path = dir.join("crossover.csv");
    let mut csv = String::from(
        "n,individual_instructions,batch_instructions,individual_instr_per_entry,batch_instr_per_entry,individual_wall_ns,batch_wall_ns,individual_wall_ns_per_entry,batch_wall_ns_per_entry,winner\n",
    );
    for row in rows {
        let winner = if row.batch_instructions < row.individual_instructions {
            "batch"
        } else if row.batch_instructions > row.individual_instructions {
            "individual"
        } else {
            "tie"
        };
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{}\n",
            row.n,
            row.individual_instructions,
            row.batch_instructions,
            row.individual_instr_per_entry,
            row.batch_instr_per_entry,
            row.individual_wall_ns,
            row.batch_wall_ns,
            row.individual_wall_ns_per_entry,
            row.batch_wall_ns_per_entry,
            winner,
        ));
    }
    fs::write(&csv_path, &csv).expect("write crossover.csv");

    let indiv = rows
        .iter()
        .map(|r| r.individual_instructions.to_string())
        .collect::<std::vec::Vec<_>>()
        .join(", ");
    let batch = rows
        .iter()
        .map(|r| r.batch_instructions.to_string())
        .collect::<std::vec::Vec<_>>()
        .join(", ");
    let header = match crossover {
        Some(n) => format!("# Crossover chart data\n\nInstruction crossover at N = {n}.\n\n"),
        None => "# Crossover chart data\n\nNo crossover in measured range.\n\n".to_string(),
    };
    let mermaid = format!(
        "```mermaid\nxychart-beta\n    title \"Instructions: individual send vs batch send\"\n    x-axis [1, 2, 5, 10, 15, 20]\n    y-axis \"Instructions\"\n    line \"individual (N x send)\" [{indiv}]\n    line \"batch (batch_send)\" [{batch}]\n```\n"
    );
    let chart_path = dir.join("crossover-chart.md");
    fs::write(&chart_path, format!("{header}{mermaid}")).expect("write crossover-chart.md");
    eprintln!(
        "Wrote chart data to {} and {}",
        csv_path.display(),
        chart_path.display()
    );
}
