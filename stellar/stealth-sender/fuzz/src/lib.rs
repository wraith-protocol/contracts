//! Pure-Rust model of the `stealth-sender` batch payload used by the fuzz
//! targets.
//!
//! The on-chain contract (`stellar/stealth-sender/src/lib.rs`) accepts a
//! variable-length batch of parallel vectors — `stealth_addresses`,
//! `ephemeral_pub_keys`, `metadatas`, `amounts` — and, for each index,
//! transfers tokens and emits one announcement. That variable-length,
//! caller-controlled batch is the attack surface we fuzz.
//!
//! Because Soroban contracts run in Wasm against host objects (not raw byte
//! buffers), this module reimplements the two byte-facing behaviours as a
//! self-contained model so `cargo fuzz` can drive them with arbitrary input:
//!
//! * [`decode`] / [`encode`] — a canonical wire codec for a batch payload,
//!   exercised for round-trip stability and panic-freedom (`batch_decode`).
//! * [`execute`] — the contract's batch loop, including the length-mismatch
//!   guard, with invariant checks asserting no event drift and no silent
//!   over-write of accumulated balances (`batch_execute`).

use arbitrary::Arbitrary;

/// Upper bound on entries decoded from a single buffer. Bounds memory so a
/// hostile declared count cannot cause an allocation blow-up.
pub const MAX_ENTRIES: usize = 4096;

/// Upper bound on a single metadata blob length (bytes).
pub const MAX_META: usize = 1024;

const ADDR_LEN: usize = 32;
const KEY_LEN: usize = 32;

/// One recipient in a batch: a stealth address, its ephemeral public key, the
/// amount to transfer, and opaque metadata (e.g. a view tag).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchEntry {
    pub stealth_address: [u8; ADDR_LEN],
    pub ephemeral_pub_key: [u8; KEY_LEN],
    /// Non-negative amount. Mirrors the valid domain of a token transfer
    /// (`i128` on-chain, but transfers of negative amounts revert).
    pub amount: u64,
    pub metadata: Vec<u8>,
}

/// A decoded batch payload: a scheme id plus its ordered entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BatchPayload {
    pub scheme_id: u32,
    pub entries: Vec<BatchEntry>,
}

/// Encode a payload into the canonical little-endian wire format.
///
/// Layout: `scheme_id: u32` `count: u32` then, per entry,
/// `stealth[32]` `ephemeral[32]` `amount: u64` `meta_len: u16` `meta[..]`.
pub fn encode(payload: &BatchPayload) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&payload.scheme_id.to_le_bytes());
    out.extend_from_slice(&(payload.entries.len() as u32).to_le_bytes());
    for e in &payload.entries {
        out.extend_from_slice(&e.stealth_address);
        out.extend_from_slice(&e.ephemeral_pub_key);
        out.extend_from_slice(&e.amount.to_le_bytes());
        let meta_len = e.metadata.len().min(MAX_META) as u16;
        out.extend_from_slice(&meta_len.to_le_bytes());
        out.extend_from_slice(&e.metadata[..meta_len as usize]);
    }
    out
}

/// A non-panicking cursor over a byte slice.
struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.remaining() < n {
            return None;
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Some(s)
    }

    fn u16(&mut self) -> Option<u16> {
        let s = self.take(2)?;
        Some(u16::from_le_bytes([s[0], s[1]]))
    }

    fn u32(&mut self) -> Option<u32> {
        let s = self.take(4)?;
        Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    fn u64(&mut self) -> Option<u64> {
        let s = self.take(8)?;
        let mut b = [0u8; 8];
        b.copy_from_slice(s);
        Some(u64::from_le_bytes(b))
    }

    fn array32(&mut self) -> Option<[u8; 32]> {
        let s = self.take(32)?;
        let mut b = [0u8; 32];
        b.copy_from_slice(s);
        Some(b)
    }
}

/// Decode a batch payload from arbitrary bytes.
///
/// Lenient and total: any input either yields a `BatchPayload` or `None`, never
/// a panic and never an over-read. A declared count/length that exceeds the
/// remaining buffer truncates the batch rather than allocating on the caller's
/// word. Decoding is capped by [`MAX_ENTRIES`] and [`MAX_META`].
pub fn decode(data: &[u8]) -> Option<BatchPayload> {
    let mut r = Reader::new(data);
    let scheme_id = r.u32()?;
    let declared = r.u32()? as usize;

    let mut entries = Vec::new();
    let want = declared.min(MAX_ENTRIES);
    for _ in 0..want {
        let stealth_address = match r.array32() {
            Some(v) => v,
            None => break,
        };
        let ephemeral_pub_key = match r.array32() {
            Some(v) => v,
            None => break,
        };
        let amount = match r.u64() {
            Some(v) => v,
            None => break,
        };
        let meta_len = match r.u16() {
            Some(v) => v as usize,
            None => break,
        };
        let take = meta_len.min(MAX_META).min(r.remaining());
        let metadata = match r.take(take) {
            Some(v) => v.to_vec(),
            None => break,
        };
        // A metadata length that outran the buffer means a truncated final
        // entry; drop it so the payload stays canonical and round-trippable.
        if take < meta_len {
            break;
        }
        entries.push(BatchEntry {
            stealth_address,
            ephemeral_pub_key,
            amount,
            metadata,
        });
    }

    Some(BatchPayload { scheme_id, entries })
}

/// Errors mirroring the on-chain contract's batch guard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchError {
    /// The parallel input vectors have mismatched lengths.
    LengthMismatch,
}

/// One announcement the contract would emit, in batch order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Announcement {
    pub scheme_id: u32,
    pub stealth_address: [u8; ADDR_LEN],
    pub ephemeral_pub_key: [u8; KEY_LEN],
    pub metadata: Vec<u8>,
}

/// Result of running the batch: the announcements emitted and the per-address
/// credited balances.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Executed {
    pub announcements: Vec<Announcement>,
    pub balances: std::collections::BTreeMap<[u8; ADDR_LEN], i128>,
    pub total: i128,
}

/// Independent parallel vectors, as an untrusted caller supplies them to
/// `batch_send`. Kept separate (rather than aligned `BatchEntry`s) so the
/// fuzzer can drive the length-mismatch path and any index drift.
#[derive(Clone, Debug, Arbitrary)]
pub struct BatchInput {
    pub scheme_id: u32,
    pub stealth_addresses: Vec<[u8; ADDR_LEN]>,
    pub ephemeral_pub_keys: Vec<[u8; KEY_LEN]>,
    pub metadatas: Vec<Vec<u8>>,
    pub amounts: Vec<u64>,
}

/// Run the contract's batch loop over `input`.
///
/// Faithfully mirrors `StealthSenderContract::batch_send`: it rejects
/// mismatched vector lengths up front, then for each index credits the transfer
/// and records one announcement, in order.
pub fn execute(input: &BatchInput) -> Result<Executed, BatchError> {
    let len = input.stealth_addresses.len();
    if input.ephemeral_pub_keys.len() != len
        || input.metadatas.len() != len
        || input.amounts.len() != len
    {
        return Err(BatchError::LengthMismatch);
    }

    let mut announcements = Vec::with_capacity(len);
    let mut balances: std::collections::BTreeMap<[u8; ADDR_LEN], i128> =
        std::collections::BTreeMap::new();
    let mut total: i128 = 0;

    for i in 0..len {
        let stealth_address = input.stealth_addresses[i];
        let ephemeral_pub_key = input.ephemeral_pub_keys[i];
        let metadata = input.metadatas[i].clone();
        let amount = input.amounts[i] as i128;

        // Accumulate — never overwrite — a repeated recipient's balance.
        let entry = balances.entry(stealth_address).or_insert(0);
        *entry = entry
            .checked_add(amount)
            .expect("balance accumulation overflowed i128");
        total = total
            .checked_add(amount)
            .expect("running total overflowed i128");

        announcements.push(Announcement {
            scheme_id: input.scheme_id,
            stealth_address,
            ephemeral_pub_key,
            metadata,
        });
    }

    Ok(Executed {
        announcements,
        balances,
        total,
    })
}

/// Assert the post-conditions the contract must uphold for a successful batch.
///
/// Panics (a fuzz finding) if any invariant drifts.
pub fn check_execute_invariants(input: &BatchInput, out: &Executed) {
    let len = input.stealth_addresses.len();

    // No event drift: exactly one announcement per input index, in order.
    assert_eq!(
        out.announcements.len(),
        len,
        "announcement count drifted from batch length",
    );

    // No index drift: announcement i must reflect input i.
    let mut expected_total: i128 = 0;
    for i in 0..len {
        let a = &out.announcements[i];
        assert_eq!(a.scheme_id, input.scheme_id, "scheme_id drift at {i}");
        assert_eq!(
            a.stealth_address, input.stealth_addresses[i],
            "stealth address drift at {i}",
        );
        assert_eq!(
            a.ephemeral_pub_key, input.ephemeral_pub_keys[i],
            "ephemeral key drift at {i}",
        );
        assert_eq!(a.metadata, input.metadatas[i], "metadata drift at {i}");
        expected_total += input.amounts[i] as i128;
    }

    // No value drift: credited total equals the sum of amounts.
    assert_eq!(out.total, expected_total, "credited total drifted");

    // No over-write: each recipient's balance is the sum of every amount sent
    // to it, so repeated addresses accumulate rather than clobber.
    let mut expected: std::collections::BTreeMap<[u8; ADDR_LEN], i128> =
        std::collections::BTreeMap::new();
    for i in 0..len {
        *expected.entry(input.stealth_addresses[i]).or_insert(0) += input.amounts[i] as i128;
    }
    assert_eq!(out.balances, expected, "balance map drifted / over-wrote");

    let sum: i128 = out.balances.values().sum();
    assert_eq!(
        sum, out.total,
        "balance map total disagrees with running total"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> BatchPayload {
        BatchPayload {
            scheme_id: 7,
            entries: vec![
                BatchEntry {
                    stealth_address: [1u8; 32],
                    ephemeral_pub_key: [2u8; 32],
                    amount: 100,
                    metadata: vec![9, 9, 9],
                },
                BatchEntry {
                    stealth_address: [3u8; 32],
                    ephemeral_pub_key: [4u8; 32],
                    amount: 250,
                    metadata: vec![],
                },
            ],
        }
    }

    #[test]
    fn round_trip_canonical() {
        let p = sample();
        let bytes = encode(&p);
        let decoded = decode(&bytes).expect("canonical decodes");
        assert_eq!(decoded, p);
        assert_eq!(encode(&decoded), bytes);
    }

    #[test]
    fn decode_never_over_reads() {
        // Declares 1000 entries but supplies almost nothing.
        let mut bytes = 5u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&1000u32.to_le_bytes());
        let p = decode(&bytes).expect("header decodes");
        assert_eq!(p.scheme_id, 5);
        assert!(p.entries.is_empty());
    }

    #[test]
    fn decode_of_encode_is_stable_for_any_first_decode() {
        // Arbitrary garbage: first decode is lenient; re-encoding then decoding
        // must be a fixed point.
        for seed in 0u8..64 {
            let data: Vec<u8> = (0..seed).map(|i| i.wrapping_mul(seed)).collect();
            if let Some(p) = decode(&data) {
                let b = encode(&p);
                let p2 = decode(&b).expect("canonical must decode");
                assert_eq!(p, p2);
                assert_eq!(encode(&p2), b);
            }
        }
    }

    #[test]
    fn execute_accumulates_duplicate_recipients() {
        let addr = [8u8; 32];
        let input = BatchInput {
            scheme_id: 1,
            stealth_addresses: vec![addr, addr],
            ephemeral_pub_keys: vec![[0u8; 32], [1u8; 32]],
            metadatas: vec![vec![], vec![1]],
            amounts: vec![10, 32],
        };
        let out = execute(&input).expect("aligned lengths");
        check_execute_invariants(&input, &out);
        assert_eq!(out.balances.get(&addr), Some(&42));
        assert_eq!(out.total, 42);
        assert_eq!(out.announcements.len(), 2);
    }

    #[test]
    fn execute_rejects_length_mismatch() {
        let input = BatchInput {
            scheme_id: 1,
            stealth_addresses: vec![[0u8; 32]],
            ephemeral_pub_keys: vec![],
            metadatas: vec![vec![]],
            amounts: vec![1],
        };
        assert_eq!(execute(&input), Err(BatchError::LengthMismatch));
    }
}
