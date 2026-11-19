#![no_main]

//! Fuzzes `CellDep` code/type hash resolution and canonical stealth
//! announcement args decoding, re-encoding, and output-index bounds.

use libfuzzer_sys::fuzz_target;
use wraith_ckb_fuzz::fuzz_entry_cell_deps_and_announcement;

fuzz_target!(|data: &[u8]| {
    fuzz_entry_cell_deps_and_announcement(data);
});
