#![no_main]

//! Fuzzes `StealthSenderContract::batch_send` end-to-end against a real
//! deployed announcer and SAC token, with independently-lengthed (so often
//! mismatched) input vectors and bounded-but-signed amounts.
//!
//! For every call, regardless of outcome, this asserts:
//! - the call never panics/traps the host (only `Ok`/`Err` outcomes reach us);
//! - on success, lengths matched, sender/recipient balances moved by exactly
//!   the batch amounts (no over-write when a recipient appears twice), and
//!   exactly one announcement event was emitted per batch item;
//! - on failure, the whole invocation left balances and events untouched
//!   (Soroban's per-invocation rollback held — no partial-batch drift).

use arbitrary::Unstructured;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{
    testutils::{Address as _, Events as _},
    token, Address, Bytes, BytesN, Env, Vec as SVec,
};
use stealth_announcer::StealthAnnouncerContract;
use stealth_sender::{StealthSenderContract, StealthSenderContractClient};

const MAX_BATCH_LEN: usize = 12;
const MAX_METADATA_LEN: usize = 64;
const RECIPIENT_POOL_SIZE: usize = 4;
const AMOUNT_BOUND: i128 = 1_000_000_000_000_000;
const MINT_BALANCE: i128 = 1_000_000_000_000_000_000;

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    let env = Env::default();
    env.mock_all_auths();

    let announcer_id = env.register(StealthAnnouncerContract, ());
    let sender_id = env.register(StealthSenderContract, ());
    let sender_client = StealthSenderContractClient::new(&env, &sender_id);
    sender_client.init(&announcer_id);

    let token_admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_admin);
    let token_address = sac.address();
    let token_client = token::Client::new(&env, &token_address);
    let asset_client = token::StellarAssetClient::new(&env, &token_address);

    let sender_addr = Address::generate(&env);
    asset_client.mint(&sender_addr, &MINT_BALANCE);

    let mut pool: std::vec::Vec<Address> = std::vec::Vec::with_capacity(RECIPIENT_POOL_SIZE);
    for _ in 0..RECIPIENT_POOL_SIZE {
        pool.push(Address::generate(&env));
    }

    let Ok(batch_len_addr) = u.int_in_range(0..=MAX_BATCH_LEN) else {
        return;
    };
    let Ok(batch_len_epk) = u.int_in_range(0..=MAX_BATCH_LEN) else {
        return;
    };
    let Ok(batch_len_meta) = u.int_in_range(0..=MAX_BATCH_LEN) else {
        return;
    };
    let Ok(batch_len_amt) = u.int_in_range(0..=MAX_BATCH_LEN) else {
        return;
    };

    let mut stealth_addresses: SVec<Address> = SVec::new(&env);
    for _ in 0..batch_len_addr {
        let Ok(idx) = u.int_in_range(0..=RECIPIENT_POOL_SIZE - 1) else {
            return;
        };
        stealth_addresses.push_back(pool[idx].clone());
    }

    let mut ephemeral_pub_keys: SVec<BytesN<32>> = SVec::new(&env);
    for _ in 0..batch_len_epk {
        let Ok(bytes) = u.arbitrary::<[u8; 32]>() else {
            return;
        };
        ephemeral_pub_keys.push_back(BytesN::from_array(&env, &bytes));
    }

    let mut metadatas: SVec<Bytes> = SVec::new(&env);
    for _ in 0..batch_len_meta {
        let Ok(meta_len) = u.int_in_range(0..=MAX_METADATA_LEN) else {
            return;
        };
        let Ok(bytes) = u.bytes(meta_len) else {
            return;
        };
        metadatas.push_back(Bytes::from_slice(&env, bytes));
    }

    let mut amounts: SVec<i128> = SVec::new(&env);
    for _ in 0..batch_len_amt {
        let Ok(amount) = u.int_in_range(-AMOUNT_BOUND..=AMOUNT_BOUND) else {
            return;
        };
        amounts.push_back(amount);
    }

    let Ok(scheme_id) = u.arbitrary::<u32>() else {
        return;
    };

    let len_addr = stealth_addresses.len();
    let lengths_match = ephemeral_pub_keys.len() == len_addr
        && metadatas.len() == len_addr
        && amounts.len() == len_addr;

    let sender_balance_before = token_client.balance(&sender_addr);
    let recipient_balances_before: std::vec::Vec<i128> =
        pool.iter().map(|a| token_client.balance(a)).collect();
    let events_before = env.events().all().len();

    let result = sender_client.try_batch_send(
        &sender_addr,
        &token_address,
        &scheme_id,
        &stealth_addresses,
        &ephemeral_pub_keys,
        &metadatas,
        &amounts,
    );

    let sender_balance_after = token_client.balance(&sender_addr);
    let recipient_balances_after: std::vec::Vec<i128> =
        pool.iter().map(|a| token_client.balance(a)).collect();
    let events_after = env.events().all().len();

    let succeeded = matches!(result, Ok(Ok(())));

    if succeeded {
        assert!(
            lengths_match,
            "batch_send reported success despite mismatched batch lengths"
        );

        let mut expected_deltas = [0i128; RECIPIENT_POOL_SIZE];
        let mut total_out: i128 = 0;
        for i in 0..len_addr {
            let addr = stealth_addresses.get(i).unwrap();
            let amount = amounts.get(i).unwrap();
            let idx = pool
                .iter()
                .position(|a| a == &addr)
                .expect("stealth address must come from the fixed recipient pool");
            let (Some(new_delta), Some(new_total)) = (
                expected_deltas[idx].checked_add(amount),
                total_out.checked_add(amount),
            ) else {
                // Harness-side accounting overflow, not a contract bug: bail
                // out of the drift assertions for this input.
                return;
            };
            expected_deltas[idx] = new_delta;
            total_out = new_total;
        }

        assert_eq!(
            sender_balance_after,
            sender_balance_before - total_out,
            "sender balance drifted from the sum of batch amounts"
        );
        for (idx, delta) in expected_deltas.iter().enumerate() {
            assert_eq!(
                recipient_balances_after[idx],
                recipient_balances_before[idx] + delta,
                "recipient balance drifted/was overwritten instead of accumulated"
            );
        }
        assert_eq!(
            events_after,
            events_before + len_addr,
            "announcement event count drifted from batch length"
        );
    } else {
        assert_eq!(
            sender_balance_after, sender_balance_before,
            "sender balance changed despite a failed batch_send"
        );
        assert_eq!(
            recipient_balances_after, recipient_balances_before,
            "recipient balances changed despite a failed batch_send"
        );
        assert_eq!(
            events_after, events_before,
            "events were emitted despite a failed batch_send"
        );
    }
});
