# Reference Indexer Schema & Migration Guide (v0 -> v1)

This directory provides the reference database schemas and SQL migration scripts for off-chain indexers, SDKs, and data partners monitoring Wraith Protocol smart contracts on Stellar.

## Overview

Cutting v1 contracts updates storage layouts, introduces contract pause circuit-breakers, upgrade timelock tracking, time-locked vault deposits, asset policy rules, and transitions `stealth-announcer` to the v2 event topic schema (`view_tag_bucket` indexing).

To prevent indexer downtime or data corruption, database schemas must be updated using zero-downtime migration scripts.

## Directory Structure

```
stellar/examples/indexer/
├── schema_v0.sql               # Base v0 indexer schema
├── schema_v1.sql               # Consolidated v1/v2 indexer target schema
├── migrations/
│   └── 001_v0_to_v1.sql        # Non-destructive SQL migration script (v0 -> v1)
├── validate_migration.py       # Automated SQL schema validation suite
└── README.md                   # Indexer documentation & instructions
```

## Schema Differences Summary

| Table | v0 Schema | v1 / v2 Additions | Purpose / Impact |
|---|---|---|---|
| `announcements` | `(id, contract_address, scheme_id, stealth_address, ephemeral_pub_key, metadata, caller, ledger, tx_hash)` | `view_tag_bucket` (INT), `metadata_kind` (VARCHAR), `scheme_version` (INT) | Enables server-side 256-bucket filtering (`getEvents` topic 2) for ~99.6% traffic reduction. |
| `registrations` | `(registrant, scheme_id, stealth_meta_address)` | `storage_type` (VARCHAR), `ttl_expiry_ledger` (INT), `is_active` (BOOL) | Tracks persistent storage migration and TTL expiration ledgers. |
| `name_registrations` | `(name, name_hash, owner, stealth_meta_address)` | `parent_hash` (VARCHAR), `guardians` (TEXT), `recovery_status` (VARCHAR), `is_paused` (BOOL) | Supports hierarchical subdomains (`sub.parent`), multisig recovery, and guardian configs. |
| `pausable_states` *(New)* | — | `(contract_address, is_paused, admin, last_changed_ledger)` | Tracks OpenZeppelin-style circuit breaker state changes across pausable contracts. |
| `timelock_proposals` *(New)* | — | `(proposal_id, contract_address, proposed_wasm_hash, timelock_end_ledger, status)` | Tracks governance contract upgrade proposals during the mandatory 7-day delay window. |
| `rate_limit_records` *(New)* | — | `(id, contract_address, sender, window_start_ledger, tx_count, window_limit)` | Observability for per-user rate-limiting windows. |
| `vault_deposits` *(New)* | — | `(deposit_id, sender, recipient, amount, asset, unlock_ledger, refund_after, status)` | Tracks time-locked payments in `stealth-vault`. |
| `asset_policy_rules` *(New)* | — | `(policy_address, asset_address, is_allowed)` | Tracks allowlisted assets for `stealth-sender`. |

## Validating the Migration

To run the automated SQL schema validation against SQLite / PostgreSQL definitions:

```bash
python3 stellar/examples/indexer/validate_migration.py
```

Expected output:

```
🚀 Starting Indexer Schema Migration Validation...
  [1/6] Loading schema_v0.sql...
  [2/6] Inserting v0 test data...
  [3/6] Executing migrations/001_v0_to_v1.sql...
  [4/6] Verifying data integrity and schema version...
  [5/6] Inserting v1/v2 event & state records...
  [6/6] Testing query filtering performance on indexed view_tag_bucket...
✅ Indexer Schema Migration Validation SUCCESSFUL! All checks passed.
```

## Running the Migration in Production

1. **Backup Database:** Create a full snapshot before executing migrations.
2. **Apply Migration Script:** Run `migrations/001_v0_to_v1.sql` within a database transaction.
3. **Deploy Updated Indexer Code:** Restart indexer service configured to parse both v1 historical events and v2 new topic schemas.
