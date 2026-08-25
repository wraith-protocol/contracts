#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token::Client as TokenClient, Address,
    Bytes, BytesN, Env, IntoVal, Vec,
};
use wraith_metrics::{contract_ids, dimension_names, emit_metric, metric_names};

#[cfg(test)]
mod test;

/// Maximum transfers per batch — justified against Soroban's ~100M instruction
/// budget. Each transfer costs ~500K instructions (token transfer + event emit).
/// 100 transfers = ~50M instructions, leaving headroom for overhead.
pub const MAX_BATCH_SIZE: u32 = 100;

/// Lightweight client wrapper to invoke the StealthAnnouncer contract.
/// Same pattern as `stealth-splitter` / `stealth-sender`: announcements go
/// out through the announcer so indexers see the canonical v2 4-topic layout
/// `("announce", scheme_id, view_tag_bucket, metadata_kind)`.
mod announcer_client {
    use soroban_sdk::{Address, Bytes, BytesN, Env, IntoVal};

    pub fn announce(
        env: &Env,
        announcer: &Address,
        scheme_id: u32,
        stealth_address: &Address,
        ephemeral_pub_key: &BytesN<32>,
        metadata: &Bytes,
    ) {
        let _: () = env.invoke_contract(
            announcer,
            &soroban_sdk::symbol_short!("announce"),
            soroban_sdk::vec![
                env,
                scheme_id.into_val(env),
                stealth_address.into_val(env),
                ephemeral_pub_key.into_val(env),
                metadata.into_val(env),
            ],
        );
    }
}

/// A single stealth transfer within a batch.
/// Mirrors the EVM WraithSender batchSendETH/batchSendERC20 structure.
#[contracttype]
#[derive(Clone)]
pub struct Transfer {
    /// Pre-computed stealth address (recipient)
    pub stealth_address: Address,
    /// Ephemeral public key for the recipient to scan with
    pub ephemeral_pub_key: BytesN<32>,
    /// Token amount (in the asset's base unit)
    pub amount: i128,
    /// Announcement metadata whose first byte is the view tag (v2 schema).
    pub metadata: Bytes,
}

#[contract]
pub struct StealthBatchSender;

#[contractimpl]
impl StealthBatchSender {
    /// Atomically send `asset` tokens from `from` to N pre-computed stealth
    /// addresses in a single transaction.
    ///
    /// Each transfer is announced via `announcer` using the v2 4-topic layout
    /// (`"announce"`, `scheme_id`, `view_tag_bucket = metadata[0] as u32`,
    /// `metadata_kind`). This is the same routing `stealth-splitter` uses, so
    /// a single `getEvents` topic-3 (view-tag) filter covers announcer,
    /// splitter, and batch-sender output.
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
    pub fn batch_send(
        env: Env,
        from: Address,
        transfers: Vec<Transfer>,
        asset: Address,
        announcer: Address,
        scheme_id: u32,
    ) {
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
            if transfer.metadata.is_empty() {
                panic!("metadata must include view tag");
            }

            total_amount += transfer.amount;

            // Execute transfer — any failure here aborts the whole tx (atomicity)
            token.transfer(&from, &transfer.stealth_address, &transfer.amount);

            // Per-transfer announcement via the announcer contract (v2 layout).
            announcer_client::announce(
                &env,
                &announcer,
                scheme_id,
                &transfer.stealth_address,
                &transfer.ephemeral_pub_key,
                &transfer.metadata,
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
