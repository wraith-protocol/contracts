#![no_main]

//! Fuzzes `wraith-names-type` script args, 66-byte output data checks,
//! singleton cell uniqueness (duplicated inputs/outputs), and zero-match runs.

use libfuzzer_sys::fuzz_target;
use wraith_ckb_fuzz::fuzz_entry_names_type_script;

fuzz_target!(|data: &[u8]| {
    fuzz_entry_names_type_script(data);
});
