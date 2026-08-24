# Stealth Batch Sender Contract (`stealth-batch-sender`)

The `stealth-batch-sender` contract atomically sends tokens from a single sender to multiple pre-computed stealth addresses in a single transaction. This provides ~100x efficiency over N individual `stealth-sender::send` calls by requiring only one authorization and one ledger round-trip.

## Purpose

Batch stealth transfers for efficiency. Instead of calling `stealth-sender::send` N times (N transactions, N auth signatures, N ledger round-trips), a single `batch_send` handles all transfers atomically in one transaction.

## Entrypoints

| Function | Description | Authorization |
|----------|-------------|----------------|
| `batch_send(env, from, transfers, asset)` | Atomically send `asset` tokens to N stealth addresses | `from` must authorize |
| `max_batch_size(env)` | Query the maximum allowed batch size | None (read-only) |

### `batch_send`

Atomically send `asset` tokens from `from` to N pre-computed stealth addresses in a single transaction.

**All-or-nothing semantics:** Soroban's transaction model guarantees atomicity. If any individual transfer panics (e.g., insufficient balance mid-batch), the entire transaction is rolled back. No partial sends are possible.

**Parameters:**
- `from: Address` — Sender address (must authorize)
- `transfers: Vec<Transfer>` — Array of stealth transfers
- `asset: Address` — Token contract address

**Returns:** None

**Transfer struct:**
```rust
pub struct Transfer {
    pub stealth_address: Address,      // Pre-computed stealth address (recipient)
    pub ephemeral_pub_key: Bytes,      // Ephemeral public key for recipient scanning
    pub amount: i128,                  // Token amount (in asset's base unit)
}
```

**Validation:**
- Batch must contain at least 1 transfer
- Batch size cannot exceed `MAX_BATCH_SIZE` (100)
- Each transfer amount must be positive
- Each `ephemeral_pub_key` must not be empty

**Events emitted:**
- Per-transfer: `("ANNOUNCE",)` with `(stealth_address, ephemeral_pub_key, amount, asset)`
- Batch summary: `("BATCH",)` with `(from, count, asset)`
- Metric events: `batch_send_count`, `batch_send_volume`, `batch_size` (see [METRICS.md](../METRICS.md))

### `max_batch_size`

Query the maximum allowed batch size.

**Returns:** `u32` — Current `MAX_BATCH_SIZE` constant (100)

## Error Variants

The contract uses panics for validation (no custom error enum):

| Condition | Panic Reason |
|-----------|--------------|
| Empty `transfers` array | "batch must contain at least one transfer" |
| `transfers.len() > MAX_BATCH_SIZE` | "batch exceeds MAX_BATCH_SIZE" |
| `transfer.amount <= 0` | "transfer amount must be positive" |
| `transfer.ephemeral_pub_key.is_empty()` | "ephemeral_pub_key must not be empty" |

## Event Topics

| Topic | Data | Description |
|-------|------|-------------|
| `("ANNOUNCE",)` | `(stealth_address, ephemeral_pub_key, amount, asset)` | Per-transfer stealth payment announcement |
| `("BATCH",)` | `(from, count, asset)` | Batch-level summary event |
| `("metric", contract_id, metric_name)` | `(value, dimensions)` | Metric events (see [METRICS.md](../METRICS.md)) |

## Storage Layout

**None** — This contract is stateless and uses no persistent storage.

## Pause / Admin / Metrics Posture

| Feature | Status |
|---------|--------|
| Pausable | No — stateless, nothing to pause |
| Admin | No — no admin controls |
| Metrics | Yes — emits `batch_send_count`, `batch_send_volume`, `batch_size` metrics |

## Related Docs

- [PAUSE.md](../PAUSE.md) — Pause posture documentation
- [MULTISIG.md](../MULTISIG.md) — Multisig setup documentation
- [METRICS.md](../METRICS.md) — Metrics standard documentation

## Constants

- `MAX_BATCH_SIZE: u32 = 100` — Maximum transfers per batch, justified against Soroban's ~100M instruction budget. Each transfer costs ~500K instructions (token transfer + event emit). 100 transfers = ~50M instructions, leaving headroom for overhead.

## Resource Budget

- **Instruction usage:** ~500K instructions per transfer (token transfer + event emit)
- **Max batch:** 100 transfers = ~50M instructions (under Soroban's ~100M limit)
- **Efficiency gain:** ~100x vs N individual stealth-sender calls (1 auth vs N auths, 1 ledger round-trip vs N)

## Testing

```bash
cargo test -p stealth-batch-sender
```

Tests cover:
- Batch send with multiple transfers
- Empty batch rejection
- Batch size limit enforcement
- Positive amount validation
- Non-empty ephemeral_pub_key validation
- Metric event emission
