#![no_main]

//! Round-trip fuzzing of the batch payload wire codec.
//!
//! Asserts that decoding arbitrary bytes never panics or over-reads, and that
//! re-encoding a decoded payload and decoding again is a fixed point (canonical
//! form is stable).

use libfuzzer_sys::fuzz_target;
use stealth_sender_fuzz::{decode, encode};

fuzz_target!(|data: &[u8]| {
    if let Some(payload) = decode(data) {
        let bytes = encode(&payload);
        let reparsed = decode(&bytes).expect("canonical encoding must decode");
        assert_eq!(payload, reparsed, "decode/encode round-trip drifted");
        assert_eq!(
            encode(&reparsed),
            bytes,
            "canonical re-encoding is unstable"
        );
    }
});
