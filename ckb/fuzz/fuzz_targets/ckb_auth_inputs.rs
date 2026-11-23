#![no_main]

//! Fuzzes `ckb-auth` IPC argv slices (`algorithm_id`, `signature`, `message`,
//! `pubkey_hash`), rejecting invalid algorithm IDs, NUL bytes, and bad recids.

use libfuzzer_sys::fuzz_target;
use wraith_ckb_fuzz::fuzz_entry_ckb_auth_inputs;

fuzz_target!(|data: &[u8]| {
    fuzz_entry_ckb_auth_inputs(data);
});
