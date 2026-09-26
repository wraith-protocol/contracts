//! `cargo bench -p wraith-stellar-bench-crossover --bench crossover`

#[path = "../src/crossover.rs"]
mod crossover;

fn main() {
    let _ = crossover::run_and_report(true);
}
