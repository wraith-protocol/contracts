#![no_main]

//! Fuzzes the XDR round trip of `batch_send`'s argument shapes: an arbitrary
//! (but independently-lengthed, so possibly mismatched) batch is built, each
//! container is serialized to XDR bytes exactly as a contract invocation
//! argument would be, deserialized back, and compared to the original.
//!
//! This does not call into the contract at all — it isolates the decode path
//! from execution so failures here point at (de)serialization, not at
//! `batch_send`'s own logic (covered by `batch_execute`).

use arbitrary::Unstructured;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{
    testutils::Address as _,
    xdr::{FromXdr, ToXdr},
    Address, Bytes, BytesN, Env, Vec as SVec,
};

const MAX_BATCH_LEN: usize = 32;
const MAX_METADATA_LEN: usize = 128;
const ADDRESS_POOL_SIZE: usize = 6;

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    let env = Env::default();

    let mut pool = std::vec::Vec::with_capacity(ADDRESS_POOL_SIZE);
    for _ in 0..ADDRESS_POOL_SIZE {
        pool.push(Address::generate(&env));
    }

    let Ok(len_addr) = u.int_in_range(0..=MAX_BATCH_LEN) else {
        return;
    };
    let Ok(len_epk) = u.int_in_range(0..=MAX_BATCH_LEN) else {
        return;
    };
    let Ok(len_meta) = u.int_in_range(0..=MAX_BATCH_LEN) else {
        return;
    };
    let Ok(len_amt) = u.int_in_range(0..=MAX_BATCH_LEN) else {
        return;
    };

    let mut stealth_addresses: SVec<Address> = SVec::new(&env);
    for _ in 0..len_addr {
        let Ok(idx) = u.int_in_range(0..=ADDRESS_POOL_SIZE - 1) else {
            return;
        };
        stealth_addresses.push_back(pool[idx].clone());
    }

    let mut ephemeral_pub_keys: SVec<BytesN<32>> = SVec::new(&env);
    for _ in 0..len_epk {
        let Ok(bytes) = u.arbitrary::<[u8; 32]>() else {
            return;
        };
        ephemeral_pub_keys.push_back(BytesN::from_array(&env, &bytes));
    }

    let mut metadatas: SVec<Bytes> = SVec::new(&env);
    for _ in 0..len_meta {
        let Ok(meta_len) = u.int_in_range(0..=MAX_METADATA_LEN) else {
            return;
        };
        let Ok(bytes) = u.bytes(meta_len) else {
            return;
        };
        metadatas.push_back(Bytes::from_slice(&env, bytes));
    }

    let mut amounts: SVec<i128> = SVec::new(&env);
    for _ in 0..len_amt {
        let Ok(amount) = u.arbitrary::<i128>() else {
            return;
        };
        amounts.push_back(amount);
    }

    assert_round_trip(&env, &stealth_addresses);
    assert_round_trip(&env, &ephemeral_pub_keys);
    assert_round_trip(&env, &metadatas);
    assert_round_trip(&env, &amounts);
});

fn assert_round_trip<T>(env: &Env, value: &T)
where
    T: Clone + PartialEq + core::fmt::Debug + ToXdr + FromXdr,
    T::Error: core::fmt::Debug,
{
    let xdr = value.clone().to_xdr(env);
    let decoded = T::from_xdr(env, &xdr)
        .expect("valid batch element failed to decode from its own XDR encoding");
    assert_eq!(
        &decoded, value,
        "decoded batch element drifted from the original"
    );
}
