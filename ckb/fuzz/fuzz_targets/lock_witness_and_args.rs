#![no_main]

//! Fuzzes Molecule `WitnessArgs` wire decoding/encoding and 53-byte
//! `wraith-stealth-lock` script args parsing.

use libfuzzer_sys::fuzz_target;
use wraith_ckb_fuzz::fuzz_entry_lock_witness_and_args;

fuzz_target!(|data: &[u8]| {
    fuzz_entry_lock_witness_and_args(data);
});
