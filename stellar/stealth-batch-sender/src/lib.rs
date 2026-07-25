#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token::Client as TokenClient, Address, Env,
    IntoVal, Vec,
};
use wraith_metrics::{contract_ids, dimension_names, emit_metric, metric_names};

/// Maximum transfers per batch — justified against Soroban's ~100M instruction
/// budget. Each transfer costs ~500K instructions (token transfer + event emit).
/// 100 transfers = ~50M instructions, leaving headroom for overhead.
pub const MAX_BATCH_SIZE: u32 = 100;

/// A single stealth transfer within a batch.
/// Mirrors the EVM WraithSender batchSendETH/batchSendERC20 structure.
#[contracttype]
#[derive(Clone)]
pub struct Transfer {
    /// Pre-computed stealth address (recipient)
    pub stealth_address: Address,
    /// Ephemeral public key for the recipient to scan with
    pub ephemeral_pub_key: soroban_sdk::Bytes,
    /// Token amount (in the asset's base unit)
    pub amount: i128,
}

#[contract]
pub struct StealthBatchSender;

#[contractimpl]
impl StealthBatchSender {
    /// Atomically send `asset` tokens from `from` to N pre-computed stealth
    /// addresses in a single transaction.
    ///
    /// # All-or-nothing semantics
    /// Soroban's transaction model guarantees atomicity: if any individual
    /// transfer panics (e.g. insufficient balance mid-batch), the entire
    /// transaction is rolled back. No partial sends are possible.
    ///
    /// # Resource budget
    /// Capped at MAX_BATCH_SIZE (100) transfers. This keeps instruction usage
    /// well under Soroban's per-transaction limit while still being ~100x more
    /// efficient than N individual stealth-sender::send calls (one auth, one
    /// ledger round-trip vs N).
    pub fn batch_send(env: Env, from: Address, transfers: Vec<Transfer>, asset: Address) {
        // Auth: sender must sign once for the entire batch
        from.require_auth();

        // Validate batch size
        let count = transfers.len();
        if count == 0 {
            panic!("batch must contain at least one transfer");
        }
        if count > MAX_BATCH_SIZE {
            panic!("batch exceeds MAX_BATCH_SIZE");
        }

        let token = TokenClient::new(&env, &asset);

        let mut total_amount: i128 = 0;

        for transfer in transfers.iter() {
            // Validate individual transfer
            if transfer.amount <= 0 {
                panic!("transfer amount must be positive");
            }
            if transfer.ephemeral_pub_key.is_empty() {
                panic!("ephemeral_pub_key must not be empty");
            }

            total_amount += transfer.amount;

            // Execute transfer — any failure here aborts the whole tx (atomicity)
            token.transfer(&from, &transfer.stealth_address, &transfer.amount);

            // Per-transfer announcement (mirrors stealth-sender pattern)
            env.events().publish(
                (symbol_short!("ANNOUNCE"),),
                (
                    transfer.stealth_address.clone(),
                    transfer.ephemeral_pub_key.clone(),
                    transfer.amount,
                    asset.clone(),
                ),
            );
        }

        // Batch-level summary event
        env.events()
            .publish((symbol_short!("BATCH"),), (from, count, asset.clone()));

        // Emit metric events.
        emit_metric(
            &env,
            contract_ids::STEALTH_BATCH_SENDER,
            metric_names::BATCH_SEND_COUNT,
            1,
            soroban_sdk::vec![&env, (dimension_names::ASSET_ADDRESS, asset.into_val(&env))],
        );
        emit_metric(
            &env,
            contract_ids::STEALTH_BATCH_SENDER,
            metric_names::BATCH_SEND_VOLUME,
            total_amount,
            soroban_sdk::vec![&env, (dimension_names::ASSET_ADDRESS, asset.into_val(&env))],
        );
        emit_metric(
            &env,
            contract_ids::STEALTH_BATCH_SENDER,
            metric_names::BATCH_SIZE,
            count as i128,
            soroban_sdk::vec![&env, (dimension_names::ASSET_ADDRESS, asset.into_val(&env))],
        );
    }

    /// Query the maximum allowed batch size.
    pub fn max_batch_size(_env: Env) -> u32 {
        MAX_BATCH_SIZE
    }
}
