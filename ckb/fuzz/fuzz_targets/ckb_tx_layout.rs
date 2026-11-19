#![no_main]

//! Fuzzes multi-cell CKB transaction layouts: `u64` capacity conservation,
//! output index bounds, lock-group witnesses, singleton `.wraith` name cells,
//! and stealth announcement cross-checks.

use libfuzzer_sys::fuzz_target;
use wraith_ckb_fuzz::fuzz_entry_ckb_tx_layout;

fuzz_target!(|data: &[u8]| {
    fuzz_entry_ckb_tx_layout(data);
});
