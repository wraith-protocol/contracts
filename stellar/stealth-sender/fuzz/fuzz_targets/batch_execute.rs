#![no_main]

//! State-consistency fuzzing of the batch execute loop.
//!
//! Feeds arbitrary parallel batch vectors (possibly of mismatched length)
//! through the contract's `batch_send` model and asserts its invariants: no
//! event drift, no index drift, and no silent over-write of accumulated
//! recipient balances.

use libfuzzer_sys::fuzz_target;
use stealth_sender_fuzz::{check_execute_invariants, execute, BatchInput};

fuzz_target!(|input: BatchInput| {
    match execute(&input) {
        Ok(out) => check_execute_invariants(&input, &out),
        Err(_) => {
            // Mismatched lengths are rejected by the guard; nothing to check.
        }
    }
});
